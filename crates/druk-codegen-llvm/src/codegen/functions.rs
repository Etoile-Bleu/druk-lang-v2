use inkwell::values::FunctionValue;

use druk_diagnostics::{ice, Diagnostic};
use druk_ir::IrStmt;
use druk_runtime::TAG_NIL;

use super::{unwrap_build, CodeGen};

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn compile_function(
        &mut self,
        function_value: FunctionValue<'ctx>,
        arity: usize,
        locals_count: u32,
        capture_slots: &[u32],
        body: &[IrStmt],
        is_main: bool,
    ) -> Result<(), Diagnostic> {
        let entry = self.context.append_basic_block(function_value, "entry");
        self.builder.position_at_end(entry);

        let locals_ty = self.value_ty.array_type(locals_count);
        let locals_ptr = unwrap_build(self.builder.build_alloca(locals_ty, "locals"));
        self.locals_ptr = Some(locals_ptr);
        self.locals_ty = Some(locals_ty);

        // Every non-main function carries a leading `self: ptr` parameter
        // (the callee's own `RtValue`, see `calls::compile_call`), unused by
        // a non-capturing function but always present so codegen never
        // branches on call shape. `main` keeps its fixed zero-arg native
        // entry signature instead.
        let param_offset = if is_main { 0 } else { 1 };
        for index in 0..arity {
            let param = function_value
                .get_nth_param((index + param_offset) as u32)
                .unwrap_or_else(|| ice!("function is missing declared parameter {index}"));
            let ptr = self.local_ptr(index as u32);
            unwrap_build(self.builder.build_store(ptr, param));
        }

        // Captures are unpacked from the environment exactly once, here,
        // before `push_frame` and before any user code runs, straight into
        // this function's own (about-to-be-rooted) locals array. Nothing
        // can allocate between function entry and this point, so `self`
        // never needs to be an independent GC root.
        if !is_main && !capture_slots.is_empty() {
            let self_ptr = function_value
                .get_nth_param(0)
                .unwrap_or_else(|| ice!("closure-capturing function is missing its self parameter"))
                .into_pointer_value();
            for (closure_index, &local_slot) in capture_slots.iter().enumerate() {
                let index_value = self
                    .context
                    .i64_type()
                    .const_int(closure_index as u64, false);
                let capture = self.call_with_out_param(
                    self.runtime.closure_get_capture,
                    &[self_ptr.into(), index_value.into()],
                    "call_closure_get_capture",
                );
                let ptr = self.local_ptr(local_slot);
                unwrap_build(self.builder.build_store(ptr, capture));
            }
        }

        self.push_frame(locals_count);

        for stmt in body {
            self.compile_stmt(stmt, function_value)?;
        }

        if !self.is_terminated() {
            self.pop_frame();
            if is_main {
                unwrap_build(self.builder.build_return(None));
            } else {
                let nil = self.const_value(TAG_NIL, 0);
                unwrap_build(self.builder.build_return(Some(&nil)));
            }
        }

        Ok(())
    }
}
