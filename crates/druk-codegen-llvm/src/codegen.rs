use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::types::{ArrayType, StructType};
use inkwell::values::{
    BasicMetadataValueEnum, CallSiteValue, FunctionValue, IntValue, PointerValue, StructValue,
    ValueKind,
};
use inkwell::{AddressSpace, IntPredicate};

use druk_diagnostics::{ice, Diagnostic};
use druk_ir::{BinOp, FunctionId, IrExpr, IrStmt, UnOp};
use druk_runtime::{TAG_BOOL, TAG_FUNCTION, TAG_NIL, TAG_NUMBER};

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
        exprs: &[&IrExpr],
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
        // (the callee's own `RtValue`, see `compile_call`), unused by a
        // non-capturing function but always present so codegen never
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

    fn compile_stmt(
        &mut self,
        stmt: &IrStmt,
        function: FunctionValue<'ctx>,
    ) -> Result<(), Diagnostic> {
        if self.is_terminated() {
            return Ok(());
        }
        match stmt {
            IrStmt::Expr(expr) => {
                self.compile_expr(expr, function)?;
            }
            IrStmt::Print(expr) => {
                let value = self.compile_expr(expr, function)?;
                let ptr = self.value_ptr(value);
                unwrap_build(self.builder.build_call(
                    self.runtime.print,
                    &[ptr.into()],
                    "call_print",
                ));
            }
            IrStmt::Return(value) => {
                let result = match value {
                    Some(expr) => self.compile_expr(expr, function)?,
                    None => self.const_value(TAG_NIL, 0),
                };
                self.pop_frame();
                unwrap_build(self.builder.build_return(Some(&result)));
            }
            IrStmt::Block(stmts) => {
                for inner in stmts {
                    self.compile_stmt(inner, function)?;
                }
            }
            IrStmt::If {
                cond,
                then_branch,
                else_branch,
            } => self.compile_if(cond, then_branch, else_branch, function)?,
            IrStmt::While { cond, body } => self.compile_while(cond, body, function)?,
        }
        Ok(())
    }

    fn compile_if(
        &mut self,
        cond: &IrExpr,
        then_branch: &[IrStmt],
        else_branch: &[IrStmt],
        function: FunctionValue<'ctx>,
    ) -> Result<(), Diagnostic> {
        let cond_value = self.compile_expr(cond, function)?;
        let cond_bool = self.is_truthy(cond_value);

        let then_bb = self.context.append_basic_block(function, "if.then");
        let else_bb = self.context.append_basic_block(function, "if.else");
        let merge_bb = self.context.append_basic_block(function, "if.merge");

        unwrap_build(
            self.builder
                .build_conditional_branch(cond_bool, then_bb, else_bb),
        );

        self.builder.position_at_end(then_bb);
        for stmt in then_branch {
            self.compile_stmt(stmt, function)?;
        }
        if !self.is_terminated() {
            unwrap_build(self.builder.build_unconditional_branch(merge_bb));
        }

        self.builder.position_at_end(else_bb);
        for stmt in else_branch {
            self.compile_stmt(stmt, function)?;
        }
        if !self.is_terminated() {
            unwrap_build(self.builder.build_unconditional_branch(merge_bb));
        }

        self.builder.position_at_end(merge_bb);
        Ok(())
    }

    fn compile_while(
        &mut self,
        cond: &IrExpr,
        body: &[IrStmt],
        function: FunctionValue<'ctx>,
    ) -> Result<(), Diagnostic> {
        let cond_bb = self.context.append_basic_block(function, "while.cond");
        let body_bb = self.context.append_basic_block(function, "while.body");
        let end_bb = self.context.append_basic_block(function, "while.end");

        unwrap_build(self.builder.build_unconditional_branch(cond_bb));

        self.builder.position_at_end(cond_bb);
        let cond_value = self.compile_expr(cond, function)?;
        let cond_bool = self.is_truthy(cond_value);
        unwrap_build(
            self.builder
                .build_conditional_branch(cond_bool, body_bb, end_bb),
        );

        self.builder.position_at_end(body_bb);
        for stmt in body {
            self.compile_stmt(stmt, function)?;
        }
        if !self.is_terminated() {
            unwrap_build(self.builder.build_unconditional_branch(cond_bb));
        }

        self.builder.position_at_end(end_bb);
        Ok(())
    }

    fn compile_expr(
        &mut self,
        expr: &IrExpr,
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        Ok(match expr {
            IrExpr::ConstInt(value) => self.const_value(TAG_NUMBER, *value),
            IrExpr::ConstBool(value) => self.const_value(TAG_BOOL, *value as i64),
            IrExpr::ConstNil => self.const_value(TAG_NIL, 0),
            IrExpr::ConstStr(value) => self.compile_string_literal(value),
            IrExpr::FunctionRef(fid) => self.compile_function_ref(*fid),
            IrExpr::GetLocal(slot) => {
                let ptr = self.local_ptr(*slot);
                unwrap_build(self.builder.build_load(self.value_ty, ptr, "load_local"))
                    .into_struct_value()
            }
            IrExpr::SetLocal(slot, value_expr) => {
                let value = self.compile_expr(value_expr, function)?;
                let ptr = self.local_ptr(*slot);
                unwrap_build(self.builder.build_store(ptr, value));
                value
            }
            IrExpr::Unary(op, operand) => self.compile_unary(*op, operand, function)?,
            IrExpr::Binary(op, lhs, rhs) => self.compile_binary(*op, lhs, rhs, function)?,
            IrExpr::Call(callee, args) => self.compile_call(callee, args, function)?,
            IrExpr::ArrayLiteral(elements) => self.compile_array_literal(elements, function)?,
            IrExpr::ArrayGet(object, index) => {
                let operands =
                    self.compile_rooted_siblings(&[object.as_ref(), index.as_ref()], function)?;
                let object_ptr = self.value_ptr(operands[0]);
                let index_ptr = self.value_ptr(operands[1]);
                self.call_with_out_param(
                    self.runtime.array_get,
                    &[object_ptr.into(), index_ptr.into()],
                    "call_array_get",
                )
            }
            IrExpr::ArraySet(object, index, value) => {
                let operands = self.compile_rooted_siblings(
                    &[object.as_ref(), index.as_ref(), value.as_ref()],
                    function,
                )?;
                let object_ptr = self.value_ptr(operands[0]);
                let index_ptr = self.value_ptr(operands[1]);
                let value_ptr = self.value_ptr(operands[2]);
                self.call_with_out_param(
                    self.runtime.array_set,
                    &[object_ptr.into(), index_ptr.into(), value_ptr.into()],
                    "call_array_set",
                )
            }
            IrExpr::StructLiteral(fields) => self.compile_struct_literal(fields, function)?,
            IrExpr::FieldGet(object, field) => {
                let object_value = self.compile_expr(object, function)?;
                let object_ptr = self.value_ptr(object_value);
                let (name_ptr, name_len) = self.compile_field_name(field);
                self.call_with_out_param(
                    self.runtime.struct_get,
                    &[object_ptr.into(), name_ptr.into(), name_len.into()],
                    "call_struct_get",
                )
            }
            IrExpr::MakeClosure(fid, captures) => {
                self.compile_make_closure(*fid, captures, function)?
            }
            IrExpr::FieldSet(object, field, value) => {
                let operands =
                    self.compile_rooted_siblings(&[object.as_ref(), value.as_ref()], function)?;
                let object_ptr = self.value_ptr(operands[0]);
                let value_ptr = self.value_ptr(operands[1]);
                let (name_ptr, name_len) = self.compile_field_name(field);
                self.call_with_out_param(
                    self.runtime.struct_set,
                    &[
                        object_ptr.into(),
                        name_ptr.into(),
                        name_len.into(),
                        value_ptr.into(),
                    ],
                    "call_struct_set",
                )
            }
        })
    }

    /// Embeds `name` as a compile-time constant and returns a pointer to its
    /// bytes plus its length, the shape `druk_rt_struct_get`/
    /// `druk_rt_struct_set` expect for a field name. A struct field's name
    /// is always a literal identifier in the source, never a dynamically
    /// computed value, so unlike an array index it never needs rooting.
    fn compile_field_name(&self, name: &str) -> (PointerValue<'ctx>, IntValue<'ctx>) {
        let global = unwrap_build(self.builder.build_global_string_ptr(name, "field_name"));
        let ptr = global.as_pointer_value();
        let len = self.context.i64_type().const_int(name.len() as u64, false);
        (ptr, len)
    }

    fn compile_string_literal(&self, value: &str) -> StructValue<'ctx> {
        let global = unwrap_build(self.builder.build_global_string_ptr(value, "str_lit"));
        let ptr = global.as_pointer_value();
        let len = self.context.i64_type().const_int(value.len() as u64, false);
        self.call_with_out_param(
            self.runtime.make_string,
            &[ptr.into(), len.into()],
            "call_make_string",
        )
    }

    fn compile_function_ref(&self, fid: FunctionId) -> StructValue<'ctx> {
        let function_value = self.functions[fid.0 as usize];
        let ptr = function_value.as_global_value().as_pointer_value();
        let int_ty = self.context.i64_type();
        let as_int = unwrap_build(self.builder.build_ptr_to_int(ptr, int_ty, "fn_ptr_as_int"));
        self.make_value_from_int(TAG_FUNCTION, as_int)
    }

    fn compile_unary(
        &mut self,
        op: UnOp,
        operand: &IrExpr,
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let value = self.compile_expr(operand, function)?;
        Ok(match op {
            UnOp::Negate => {
                let payload = self.extract_payload(value);
                let zero = self.context.i64_type().const_zero();
                let negated = unwrap_build(self.builder.build_int_sub(zero, payload, "neg"));
                self.make_value_from_int(TAG_NUMBER, negated)
            }
            UnOp::Not => {
                let truthy = self.is_truthy(value);
                let inverted = unwrap_build(self.builder.build_not(truthy, "not"));
                self.bool_to_tagged(inverted)
            }
        })
    }

    fn compile_binary(
        &mut self,
        op: BinOp,
        lhs: &IrExpr,
        rhs: &IrExpr,
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let operands = self.compile_rooted_siblings(&[lhs, rhs], function)?;
        let lhs_value = operands[0];
        let rhs_value = operands[1];

        Ok(match op {
            BinOp::Eq => {
                let lhs_ptr = self.value_ptr(lhs_value);
                let rhs_ptr = self.value_ptr(rhs_value);
                self.call_with_out_param(
                    self.runtime.values_equal,
                    &[lhs_ptr.into(), rhs_ptr.into()],
                    "call_values_equal",
                )
            }
            BinOp::NotEq => {
                let lhs_ptr = self.value_ptr(lhs_value);
                let rhs_ptr = self.value_ptr(rhs_value);
                let result = self.call_with_out_param(
                    self.runtime.values_equal,
                    &[lhs_ptr.into(), rhs_ptr.into()],
                    "call_values_equal",
                );
                let equal = self.extract_payload(result);
                let zero = self.context.i64_type().const_zero();
                let not_equal = unwrap_build(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    equal,
                    zero,
                    "not_eq",
                ));
                self.bool_to_tagged(not_equal)
            }
            BinOp::Add => {
                let result = unwrap_build(self.builder.build_int_add(
                    self.extract_payload(lhs_value),
                    self.extract_payload(rhs_value),
                    "add",
                ));
                self.make_value_from_int(TAG_NUMBER, result)
            }
            BinOp::Sub => {
                let result = unwrap_build(self.builder.build_int_sub(
                    self.extract_payload(lhs_value),
                    self.extract_payload(rhs_value),
                    "sub",
                ));
                self.make_value_from_int(TAG_NUMBER, result)
            }
            BinOp::Mul => {
                let result = unwrap_build(self.builder.build_int_mul(
                    self.extract_payload(lhs_value),
                    self.extract_payload(rhs_value),
                    "mul",
                ));
                self.make_value_from_int(TAG_NUMBER, result)
            }
            BinOp::Div => {
                let result = unwrap_build(self.builder.build_int_signed_div(
                    self.extract_payload(lhs_value),
                    self.extract_payload(rhs_value),
                    "div",
                ));
                self.make_value_from_int(TAG_NUMBER, result)
            }
            BinOp::Lt => self.compile_comparison(IntPredicate::SLT, lhs_value, rhs_value),
            BinOp::LtEq => self.compile_comparison(IntPredicate::SLE, lhs_value, rhs_value),
            BinOp::Gt => self.compile_comparison(IntPredicate::SGT, lhs_value, rhs_value),
            BinOp::GtEq => self.compile_comparison(IntPredicate::SGE, lhs_value, rhs_value),
            BinOp::And => {
                let combined = unwrap_build(self.builder.build_and(
                    self.is_truthy(lhs_value),
                    self.is_truthy(rhs_value),
                    "and",
                ));
                self.bool_to_tagged(combined)
            }
            BinOp::Or => {
                let combined = unwrap_build(self.builder.build_or(
                    self.is_truthy(lhs_value),
                    self.is_truthy(rhs_value),
                    "or",
                ));
                self.bool_to_tagged(combined)
            }
        })
    }

    fn compile_comparison(
        &self,
        predicate: IntPredicate,
        lhs: StructValue<'ctx>,
        rhs: StructValue<'ctx>,
    ) -> StructValue<'ctx> {
        let cmp = unwrap_build(self.builder.build_int_compare(
            predicate,
            self.extract_payload(lhs),
            self.extract_payload(rhs),
            "cmp",
        ));
        self.bool_to_tagged(cmp)
    }

    fn compile_call(
        &mut self,
        callee: &IrExpr,
        args: &[IrExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        // `callee` and `args` are evaluated and rooted together: a closure
        // built fresh as the call target (never stored in a local) must
        // stay reachable while its arguments are evaluated, exactly like
        // any other set of sibling expressions.
        let mut exprs: Vec<&IrExpr> = Vec::with_capacity(args.len() + 1);
        exprs.push(callee);
        exprs.extend(args.iter());
        let values = self.compile_rooted_siblings(&exprs, function)?;
        let callee_value = values[0];
        let arg_values = &values[1..];

        let self_ptr = self.value_ptr(callee_value);
        let fn_ptr_int = self.closure_function_ptr(self_ptr);
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let fn_ptr = unwrap_build(
            self.builder
                .build_int_to_ptr(fn_ptr_int, ptr_ty, "callee_ptr"),
        );

        let mut param_types: Vec<inkwell::types::BasicMetadataTypeEnum> = vec![ptr_ty.into()];
        for _ in 0..args.len() {
            param_types.push(self.value_ty.into());
        }
        let fn_type = self.value_ty.fn_type(&param_types, false);

        let mut call_args: Vec<BasicMetadataValueEnum> = vec![self_ptr.into()];
        for value in arg_values {
            call_args.push((*value).into());
        }

        let call = unwrap_build(self.builder.build_indirect_call(
            fn_type,
            fn_ptr,
            &call_args,
            "call_indirect",
        ));
        Ok(call_result(call))
    }

    fn compile_make_closure(
        &mut self,
        fid: FunctionId,
        captures: &[IrExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let capture_refs: Vec<&IrExpr> = captures.iter().collect();
        let values = self.compile_rooted_siblings(&capture_refs, function)?;

        let len = values.len() as u32;
        let buffer_ty = self.value_ty.array_type(len);
        let buffer = unwrap_build(self.builder.build_alloca(buffer_ty, "closure_captures"));
        for (index, value) in values.into_iter().enumerate() {
            let slot_ptr = self.array_slot_ptr(buffer_ty, buffer, index as u32);
            unwrap_build(self.builder.build_store(slot_ptr, value));
        }

        let target_function = self.functions[fid.0 as usize];
        let target_ptr = target_function.as_global_value().as_pointer_value();
        let int_ty = self.context.i64_type();
        let function_ptr_int = unwrap_build(self.builder.build_ptr_to_int(
            target_ptr,
            int_ty,
            "closure_fn_ptr_as_int",
        ));

        let len_value = self.context.i64_type().const_int(len as u64, false);
        Ok(self.call_with_out_param(
            self.runtime.make_closure,
            &[function_ptr_int.into(), buffer.into(), len_value.into()],
            "call_make_closure",
        ))
    }

    fn compile_array_literal(
        &mut self,
        elements: &[IrExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let element_refs: Vec<&IrExpr> = elements.iter().collect();
        let values = self.compile_rooted_siblings(&element_refs, function)?;

        let len = values.len() as u32;
        let array_ty = self.value_ty.array_type(len);
        let buffer = unwrap_build(self.builder.build_alloca(array_ty, "array_buffer"));
        for (index, value) in values.into_iter().enumerate() {
            let elem_ptr = self.array_slot_ptr(array_ty, buffer, index as u32);
            unwrap_build(self.builder.build_store(elem_ptr, value));
        }

        let len_value = self.context.i64_type().const_int(len as u64, false);
        Ok(self.call_with_out_param(
            self.runtime.make_array,
            &[buffer.into(), len_value.into()],
            "call_make_array",
        ))
    }

    /// The `RtField { name_ptr: ptr, name_len: i64, value: RtValue }` type
    /// `compile_struct_literal` builds a buffer of, matching `druk-runtime`'s
    /// `#[repr(C)]` struct field for field.
    fn field_ty(&self) -> StructType<'ctx> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i64_t = self.context.i64_type();
        self.context
            .struct_type(&[ptr_t.into(), i64_t.into(), self.value_ty.into()], false)
    }

    fn compile_struct_literal(
        &mut self,
        fields: &[(String, IrExpr)],
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let value_exprs: Vec<&IrExpr> = fields.iter().map(|(_, value)| value).collect();
        let values = self.compile_rooted_siblings(&value_exprs, function)?;

        let field_ty = self.field_ty();
        let len = values.len() as u32;
        let buffer_ty = field_ty.array_type(len);
        let buffer = unwrap_build(self.builder.build_alloca(buffer_ty, "struct_fields"));
        let i32_ty = self.context.i32_type();

        for (index, ((name, _), value)) in fields.iter().zip(values).enumerate() {
            let (name_ptr, name_len) = self.compile_field_name(name);
            // SAFETY: `index` ranges over `0..len`, the exact bound
            // `buffer_ty` was created with, and `buffer` was just allocated
            // by this same builder in this same function, so each GEP stays
            // in bounds. The third index selects one of `field_ty`'s three
            // fixed fields, always in range.
            let name_ptr_slot = unsafe {
                unwrap_build(self.builder.build_gep(
                    buffer_ty,
                    buffer,
                    &[
                        i32_ty.const_zero(),
                        i32_ty.const_int(index as u64, false),
                        i32_ty.const_zero(),
                    ],
                    "field_name_ptr_slot",
                ))
            };
            unwrap_build(self.builder.build_store(name_ptr_slot, name_ptr));

            let name_len_slot = unsafe {
                unwrap_build(self.builder.build_gep(
                    buffer_ty,
                    buffer,
                    &[
                        i32_ty.const_zero(),
                        i32_ty.const_int(index as u64, false),
                        i32_ty.const_int(1, false),
                    ],
                    "field_name_len_slot",
                ))
            };
            unwrap_build(self.builder.build_store(name_len_slot, name_len));

            let value_slot = unsafe {
                unwrap_build(self.builder.build_gep(
                    buffer_ty,
                    buffer,
                    &[
                        i32_ty.const_zero(),
                        i32_ty.const_int(index as u64, false),
                        i32_ty.const_int(2, false),
                    ],
                    "field_value_slot",
                ))
            };
            unwrap_build(self.builder.build_store(value_slot, value));
        }

        let len_value = self.context.i64_type().const_int(len as u64, false);
        Ok(self.call_with_out_param(
            self.runtime.make_struct,
            &[buffer.into(), len_value.into()],
            "call_make_struct",
        ))
    }
}
