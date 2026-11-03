use inkwell::values::{FunctionValue, StructValue};
use inkwell::IntPredicate;

use druk_diagnostics::Diagnostic;
use druk_ir::{BinOp, FunctionId, IrExpr, UnOp};
use druk_runtime::{TAG_BOOL, TAG_FUNCTION, TAG_NIL, TAG_NUMBER};

use super::{unwrap_build, CodeGen};

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn compile_expr(
        &mut self,
        expr: &IrExpr,
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        Ok(match expr {
            IrExpr::ConstInt(value) => self.const_value(TAG_NUMBER, *value),
            IrExpr::ConstBool(value) => self.const_value(TAG_BOOL, *value as i64),
            IrExpr::ConstNil => self.const_value(TAG_NIL, 0),
            IrExpr::ConstStr(value) => self.compile_string_literal(function, value),
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
                let object_ptr = self.value_ptr(function, operands[0]);
                let index_ptr = self.value_ptr(function, operands[1]);
                self.call_with_out_param(
                    function,
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
                let object_ptr = self.value_ptr(function, operands[0]);
                let index_ptr = self.value_ptr(function, operands[1]);
                let value_ptr = self.value_ptr(function, operands[2]);
                self.call_with_out_param(
                    function,
                    self.runtime.array_set,
                    &[object_ptr.into(), index_ptr.into(), value_ptr.into()],
                    "call_array_set",
                )
            }
            IrExpr::StructLiteral(fields) => self.compile_struct_literal(fields, function)?,
            IrExpr::FieldGet(object, field) => {
                let object_value = self.compile_expr(object, function)?;
                let object_ptr = self.value_ptr(function, object_value);
                let (name_ptr, name_len) = self.compile_field_name(field);
                self.call_with_out_param(
                    function,
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
                let object_ptr = self.value_ptr(function, operands[0]);
                let value_ptr = self.value_ptr(function, operands[1]);
                let (name_ptr, name_len) = self.compile_field_name(field);
                self.call_with_out_param(
                    function,
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
            IrExpr::Builtin(op, args) => self.compile_builtin(*op, args, function)?,
        })
    }

    fn compile_string_literal(
        &self,
        function: FunctionValue<'ctx>,
        value: &str,
    ) -> StructValue<'ctx> {
        let global = unwrap_build(self.builder.build_global_string_ptr(value, "str_lit"));
        let ptr = global.as_pointer_value();
        let len = self.context.i64_type().const_int(value.len() as u64, false);
        self.call_with_out_param(
            function,
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
                let truthy = self.is_truthy(function, value);
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
                let lhs_ptr = self.value_ptr(function, lhs_value);
                let rhs_ptr = self.value_ptr(function, rhs_value);
                self.call_with_out_param(
                    function,
                    self.runtime.values_equal,
                    &[lhs_ptr.into(), rhs_ptr.into()],
                    "call_values_equal",
                )
            }
            BinOp::NotEq => {
                let lhs_ptr = self.value_ptr(function, lhs_value);
                let rhs_ptr = self.value_ptr(function, rhs_value);
                let result = self.call_with_out_param(
                    function,
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
            BinOp::Rem => {
                let result = unwrap_build(self.builder.build_int_signed_rem(
                    self.extract_payload(lhs_value),
                    self.extract_payload(rhs_value),
                    "rem",
                ));
                self.make_value_from_int(TAG_NUMBER, result)
            }
            BinOp::Lt => self.compile_comparison(IntPredicate::SLT, lhs_value, rhs_value),
            BinOp::LtEq => self.compile_comparison(IntPredicate::SLE, lhs_value, rhs_value),
            BinOp::Gt => self.compile_comparison(IntPredicate::SGT, lhs_value, rhs_value),
            BinOp::GtEq => self.compile_comparison(IntPredicate::SGE, lhs_value, rhs_value),
            BinOp::And => {
                let combined = unwrap_build(self.builder.build_and(
                    self.is_truthy(function, lhs_value),
                    self.is_truthy(function, rhs_value),
                    "and",
                ));
                self.bool_to_tagged(combined)
            }
            BinOp::Or => {
                let combined = unwrap_build(self.builder.build_or(
                    self.is_truthy(function, lhs_value),
                    self.is_truthy(function, rhs_value),
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
}
