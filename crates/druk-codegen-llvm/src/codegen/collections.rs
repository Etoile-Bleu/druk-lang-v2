use inkwell::types::StructType;
use inkwell::values::{FunctionValue, StructValue};
use inkwell::AddressSpace;

use druk_diagnostics::Diagnostic;
use druk_ir::IrExpr;

use super::{unwrap_build, CodeGen};

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn compile_array_literal(
        &mut self,
        elements: &[IrExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let element_refs: Vec<&IrExpr> = elements.iter().collect();
        let values = self.compile_rooted_siblings(&element_refs, function)?;

        let len = values.len() as u32;
        let array_ty = self.value_ty.array_type(len);
        let buffer = self.build_entry_alloca(function, array_ty, "array_buffer");
        for (index, value) in values.into_iter().enumerate() {
            let elem_ptr = self.array_slot_ptr(array_ty, buffer, index as u32);
            unwrap_build(self.builder.build_store(elem_ptr, value));
        }

        let len_value = self.context.i64_type().const_int(len as u64, false);
        Ok(self.call_with_out_param(
            function,
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

    pub(crate) fn compile_struct_literal(
        &mut self,
        fields: &[(String, IrExpr)],
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let value_exprs: Vec<&IrExpr> = fields.iter().map(|(_, value)| value).collect();
        let values = self.compile_rooted_siblings(&value_exprs, function)?;

        let field_ty = self.field_ty();
        let len = values.len() as u32;
        let buffer_ty = field_ty.array_type(len);
        let buffer = self.build_entry_alloca(function, buffer_ty, "struct_fields");
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
            function,
            self.runtime.make_struct,
            &[buffer.into(), len_value.into()],
            "call_make_struct",
        ))
    }
}
