//! The `CodeGen` struct and the primitives every other file in this module
//! builds on: the tagged-value helpers (`const_value`, `make_value_from_int`,
//! `extract_payload`), the GC-rooting helpers (`compile_rooted_siblings`,
//! `push_frame`/`pop_frame`), and the runtime-call helpers
//! (`call_with_out_param`, `value_ptr`). Split by responsibility into sibling
//! files: `functions` (function prologue/epilogue), `statements` (control
//! flow), `expressions` (the `compile_expr` dispatcher and arithmetic),
//! `calls` (indirect calls and closure creation), `collections` (array/struct
//! literals), and `builtins` (the Dzongkha builtin functions).

mod builtins;
mod calls;
mod collections;
mod expressions;
mod functions;
mod statements;

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::types::ArrayType;
use inkwell::types::StructType;
use inkwell::values::{
    BasicMetadataValueEnum, CallSiteValue, FunctionValue, IntValue, PointerValue, StructValue,
    ValueKind,
};
use inkwell::IntPredicate;

use druk_diagnostics::{ice, Diagnostic};
use druk_runtime::{TAG_BOOL, TAG_NIL};

use crate::runtime_fns::RuntimeFns;

fn unwrap_build<T>(result: Result<T, inkwell::builder::BuilderError>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => ice!("LLVM builder call failed: {err}"),
    }
}

fn call_result(call: CallSiteValue<'_>) -> StructValue<'_> {
    match call.try_as_basic_value() {
        ValueKind::Basic(value) => value.into_struct_value(),
        ValueKind::Instruction(_) => {
            ice!("a runtime call that should return a value returned void")
        }
    }
}

pub(crate) struct CodeGen<'ctx> {
    pub(crate) context: &'ctx Context,
    pub(crate) builder: &'ctx Builder<'ctx>,
    pub(crate) value_ty: StructType<'ctx>,
    pub(crate) runtime: RuntimeFns<'ctx>,
    pub(crate) functions: Vec<FunctionValue<'ctx>>,
    /// The current function's locals, as one contiguous `alloca` array
    /// rather than a separate `alloca` per slot, so its pointer and length
    /// can be handed to `druk_rt_push_frame` as the GC's root set for this
    /// frame. `None` outside of `compile_function` (there is no "current
    /// function" between top-level calls).
    pub(crate) locals_ptr: Option<PointerValue<'ctx>>,
    pub(crate) locals_ty: Option<ArrayType<'ctx>>,
}

impl<'ctx> CodeGen<'ctx> {
    fn is_terminated(&self) -> bool {
        self.builder
            .get_insert_block()
            .and_then(BasicBlock::get_terminator)
            .is_some()
    }

    fn const_value(&self, tag: i64, payload: i64) -> StructValue<'ctx> {
        let i64_t = self.context.i64_type();
        self.value_ty.const_named_struct(&[
            i64_t.const_int(tag as u64, true).into(),
            i64_t.const_int(payload as u64, true).into(),
        ])
    }

    fn make_value_from_int(&self, tag: i64, payload: IntValue<'ctx>) -> StructValue<'ctx> {
        let tag_value = self.context.i64_type().const_int(tag as u64, true);
        let undef = self.value_ty.get_undef();
        let with_tag = unwrap_build(
            self.builder
                .build_insert_value(undef, tag_value, 0, "with_tag"),
        );
        let with_payload =
            unwrap_build(
                self.builder
                    .build_insert_value(with_tag, payload, 1, "with_payload"),
            );
        with_payload.into_struct_value()
    }

    fn extract_payload(&self, value: StructValue<'ctx>) -> IntValue<'ctx> {
        unwrap_build(self.builder.build_extract_value(value, 1, "payload")).into_int_value()
    }

    /// Points at slot `index` of an array `alloca` of type `ty` at `ptr`.
    /// Shared by every place that stores into a fixed-size stack array of
    /// `RtValue`s: locals, array literals, and `compile_rooted_siblings`'s
    /// pending-values buffer.
    fn array_slot_ptr(
        &self,
        ty: ArrayType<'ctx>,
        ptr: PointerValue<'ctx>,
        index: u32,
    ) -> PointerValue<'ctx> {
        let i32_ty = self.context.i32_type();
        // SAFETY: every caller passes an `index` below the exact length
        // `ty` was created with, and a `ptr` that was itself just allocated
        // with that same `ty`, so the GEP stays in bounds.
        unsafe {
            unwrap_build(self.builder.build_gep(
                ty,
                ptr,
                &[i32_ty.const_zero(), i32_ty.const_int(index as u64, false)],
                "array_slot",
            ))
        }
    }

    /// Points at local slot `slot` inside the current function's locals
    /// array. Only valid between `compile_function` allocating that array
    /// and returning from it.
    fn local_ptr(&self, slot: u32) -> PointerValue<'ctx> {
        let ptr = self
            .locals_ptr
            .unwrap_or_else(|| ice!("local slot accessed outside of compile_function"));
        let ty = self
            .locals_ty
            .unwrap_or_else(|| ice!("local slot accessed outside of compile_function"));
        self.array_slot_ptr(ty, ptr, slot)
    }

    /// Registers `ptr` (an array of `count` `RtValue`s) as a temporary GC
    /// root for the duration of the block emitted between this call and the
    /// matching [`CodeGen::pop_temp_frame`].
    fn push_temp_frame(&self, ptr: PointerValue<'ctx>, count: u32) {
        let count_value = self.context.i64_type().const_int(count as u64, false);
        unwrap_build(self.builder.build_call(
            self.runtime.push_frame,
            &[ptr.into(), count_value.into()],
            "call_push_temp_frame",
        ));
    }

    fn pop_temp_frame(&self) {
        unwrap_build(
            self.builder
                .build_call(self.runtime.pop_frame, &[], "call_pop_temp_frame"),
        );
    }

    /// Evaluates every expression in `exprs`, left to right, keeping each
    /// already-computed sibling rooted in a temporary GC frame for the rest
    /// of the evaluation.
    ///
    /// Without this, a later sibling's own allocation could collect an
    /// earlier sibling's fresh heap value before it's stored anywhere: a
    /// named local stays rooted for as long as its function's frame is
    /// registered (see `push_frame`), but a value that has only just been
    /// computed (the first two elements of an array literal, while a third
    /// element's construction calls `druk_rt_make_string`) isn't reachable
    /// from any frame until something stores it into one. This closes that
    /// gap for every place `compile_expr` is called more than once before
    /// all the results are consumed: array literals, call arguments, binary
    /// operands, and array index/assignment operands.
    fn compile_rooted_siblings(
        &mut self,
        exprs: &[&druk_ir::IrExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<Vec<StructValue<'ctx>>, Diagnostic> {
        if exprs.is_empty() {
            return Ok(Vec::new());
        }

        let len = exprs.len() as u32;
        let buffer_ty = self.value_ty.array_type(len);
        let buffer = unwrap_build(self.builder.build_alloca(buffer_ty, "pending_siblings"));

        // Every slot starts out as a safe sentinel (tag = nil), so a
        // collection that fires before a later slot is written never traces
        // uninitialized bytes as if they were a handle.
        let nil = self.const_value(TAG_NIL, 0);
        for index in 0..len {
            let slot_ptr = self.array_slot_ptr(buffer_ty, buffer, index);
            unwrap_build(self.builder.build_store(slot_ptr, nil));
        }

        self.push_temp_frame(buffer, len);

        let mut values = Vec::with_capacity(exprs.len());
        for (index, expr) in exprs.iter().enumerate() {
            let value = self.compile_expr(expr, function)?;
            let slot_ptr = self.array_slot_ptr(buffer_ty, buffer, index as u32);
            unwrap_build(self.builder.build_store(slot_ptr, value));
            values.push(value);
        }

        self.pop_temp_frame();
        Ok(values)
    }

    /// Registers the current function's locals array as a GC root set for
    /// the duration of this call. See `druk_rt_push_frame`'s doc comment in
    /// `druk-runtime` for why this, rather than scanning the native stack,
    /// is how Druk's collector finds live locals across (including
    /// recursive) calls.
    fn push_frame(&self, locals_count: u32) {
        let ptr = self
            .locals_ptr
            .unwrap_or_else(|| ice!("push_frame called outside of compile_function"));
        let count = self
            .context
            .i64_type()
            .const_int(locals_count as u64, false);
        unwrap_build(self.builder.build_call(
            self.runtime.push_frame,
            &[ptr.into(), count.into()],
            "call_push_frame",
        ));
    }

    /// Unregisters the current function's locals, emitted right before
    /// every return point so a popped frame never outlives its call.
    fn pop_frame(&self) {
        unwrap_build(
            self.builder
                .build_call(self.runtime.pop_frame, &[], "call_pop_frame"),
        );
    }

    /// Writes `value` into a fresh stack slot and returns a pointer to it,
    /// for passing a runtime function an `RtValue` argument. See the
    /// `druk_runtime` module doc comment for why every `RtValue` crosses
    /// this FFI boundary through a pointer rather than by value.
    fn value_ptr(&self, value: StructValue<'ctx>) -> PointerValue<'ctx> {
        let slot = unwrap_build(self.builder.build_alloca(self.value_ty, "value_tmp"));
        unwrap_build(self.builder.build_store(slot, value));
        slot
    }

    fn is_truthy(&self, value: StructValue<'ctx>) -> IntValue<'ctx> {
        let ptr = self.value_ptr(value);
        let call = unwrap_build(self.builder.build_call(
            self.runtime.is_truthy,
            &[ptr.into()],
            "call_is_truthy",
        ));
        let raw = match call.try_as_basic_value() {
            ValueKind::Basic(value) => value.into_int_value(),
            ValueKind::Instruction(_) => ice!("druk_rt_is_truthy unexpectedly returned void"),
        };
        let zero = self.context.i64_type().const_zero();
        unwrap_build(
            self.builder
                .build_int_compare(IntPredicate::NE, raw, zero, "truthy"),
        )
    }

    /// Resolves the code pointer to jump to for a call target, whether it's
    /// a plain function value (`TAG_FUNCTION`, payload is already the raw
    /// pointer) or a closure (`TAG_CLOSURE`, payload is a heap handle):
    /// `druk_rt_closure_function_ptr` is the single dispatch point so
    /// codegen itself never branches on which tag `self_ptr` holds.
    fn closure_function_ptr(&self, self_ptr: PointerValue<'ctx>) -> IntValue<'ctx> {
        let call = unwrap_build(self.builder.build_call(
            self.runtime.closure_function_ptr,
            &[self_ptr.into()],
            "call_closure_function_ptr",
        ));
        match call.try_as_basic_value() {
            ValueKind::Basic(value) => value.into_int_value(),
            ValueKind::Instruction(_) => {
                ice!("druk_rt_closure_function_ptr unexpectedly returned void")
            }
        }
    }

    fn bool_to_tagged(&self, bit: IntValue<'ctx>) -> StructValue<'ctx> {
        let extended = unwrap_build(self.builder.build_int_z_extend(
            bit,
            self.context.i64_type(),
            "bool_ext",
        ));
        self.make_value_from_int(TAG_BOOL, extended)
    }

    /// Embeds `name` as a compile-time constant and returns a pointer to its
    /// bytes plus its length, the shape `druk_rt_struct_get`/
    /// `druk_rt_struct_set` expect for a field name. A struct field's name
    /// is always a literal identifier in the source, never a dynamically
    /// computed value, so unlike an array index it never needs rooting.
    /// Shared by `expressions` (field read/write) and `collections` (struct
    /// literals).
    fn compile_field_name(&self, name: &str) -> (PointerValue<'ctx>, IntValue<'ctx>) {
        let global = unwrap_build(self.builder.build_global_string_ptr(name, "field_name"));
        let ptr = global.as_pointer_value();
        let len = self.context.i64_type().const_int(name.len() as u64, false);
        (ptr, len)
    }

    /// Calls an `RtValue`-producing runtime function: allocate a slot for
    /// the callee to write its result into, pass its pointer as the leading
    /// argument, then load the result back out. `args` are the rest of the
    /// callee's parameters, already assembled by the caller (themselves
    /// plain integers or pointers from `value_ptr`, never a struct passed
    /// by value, see the `druk_runtime` module doc comment).
    fn call_with_out_param(
        &self,
        function: FunctionValue<'ctx>,
        args: &[BasicMetadataValueEnum<'ctx>],
        name: &str,
    ) -> StructValue<'ctx> {
        let out = unwrap_build(self.builder.build_alloca(self.value_ty, "out"));
        let mut full_args: Vec<BasicMetadataValueEnum<'ctx>> = vec![out.into()];
        full_args.extend_from_slice(args);
        unwrap_build(self.builder.build_call(function, &full_args, name));
        unwrap_build(self.builder.build_load(self.value_ty, out, "out_load")).into_struct_value()
    }
}
