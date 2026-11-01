use inkwell::values::FunctionValue;

use druk_diagnostics::{ice, Diagnostic};
use druk_ir::IrStmt;
use druk_runtime::TAG_NIL;

use super::{unwrap_build, CodeGen, MainKind};

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn compile_function(
        &mut self,
        function_value: FunctionValue<'ctx>,
        arity: usize,
        locals_count: u32,
        capture_slots: &[u32],
        body: &[IrStmt],
        main_kind: MainKind,
    ) -> Result<(), Diagnostic> {
        let entry = self.context.append_basic_block(function_value, "entry");
        self.builder.position_at_end(entry);

        let locals_ty = self.value_ty.array_type(locals_count);
        let locals_ptr = unwrap_build(self.builder.build_alloca(locals_ty, "locals"));
        self.locals_ptr = Some(locals_ptr);
        self.locals_ty = Some(locals_ty);

        let is_main = main_kind != MainKind::NotMain;

        // Every non-main function carries a leading `self: ptr` parameter
        // (the callee's own `RtValue`, see `calls::compile_call`), unused by
        // a non-capturing function but always present so codegen never
        // branches on call shape. Either kind of `main` keeps its own fixed
        // entry signature instead (see `MainKind`).
        let param_offset = if is_main { 0 } else { 1 };
        for index in 0..arity {
            let param = function_value
                .get_nth_param((index + param_offset) as u32)
                .unwrap_or_else(|| ice!("function is missing declared parameter {index}"));
            let ptr = self.local_ptr(index as u32);
            unwrap_build(self.builder.build_store(ptr, param));
        }

        // A natively-compiled `main` is the process's real OS entry point,
        // so it's the only place `argv`/`argc` can come from; convert them
        // into the same internal state `druk_rt_argv`/`argc` read before
        // anything else runs. The JIT path has no equivalent step here: its
        // `main` takes no parameters, since `run()` sets that state ahead
        // of time instead.
        if main_kind == MainKind::Native {
            let argc = function_value
                .get_nth_param(0)
                .unwrap_or_else(|| ice!("native main is missing its argc parameter"));
            let argv = function_value
                .get_nth_param(1)
                .unwrap_or_else(|| ice!("native main is missing its argv parameter"));
            unwrap_build(self.builder.build_call(
                self.runtime.init_args,
                &[argc.into(), argv.into()],
                "call_init_args",
            ));
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
                    function_value,
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
            match main_kind {
                MainKind::Jit => {
                    unwrap_build(self.builder.build_return(None));
                }
                MainKind::Native => {
                    let zero = self.context.i32_type().const_zero();
                    unwrap_build(self.builder.build_return(Some(&zero)));
                }
                MainKind::NotMain => {
                    let nil = self.const_value(TAG_NIL, 0);
                    unwrap_build(self.builder.build_return(Some(&nil)));
                }
            }
        }

        Ok(())
    }
}
