use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::{BasicMetadataValueEnum, FunctionValue, StructValue};
use inkwell::AddressSpace;

use druk_diagnostics::Diagnostic;
use druk_ir::{FunctionId, IrExpr};

use super::{call_result, unwrap_build, CodeGen};

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn compile_call(
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

        let self_ptr = self.value_ptr(function, callee_value);
        let fn_ptr_int = self.closure_function_ptr(self_ptr);
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let fn_ptr = unwrap_build(
            self.builder
                .build_int_to_ptr(fn_ptr_int, ptr_ty, "callee_ptr"),
        );

        let mut param_types: Vec<BasicMetadataTypeEnum> = vec![ptr_ty.into()];
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

    pub(crate) fn compile_make_closure(
        &mut self,
        fid: FunctionId,
        captures: &[IrExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let capture_refs: Vec<&IrExpr> = captures.iter().collect();
        let values = self.compile_rooted_siblings(&capture_refs, function)?;

        let len = values.len() as u32;
        let buffer_ty = self.value_ty.array_type(len);
        let buffer = self.build_entry_alloca(function, buffer_ty, "closure_captures");
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
            function,
            self.runtime.make_closure,
            &[function_ptr_int.into(), buffer.into(), len_value.into()],
            "call_make_closure",
        ))
    }
}
