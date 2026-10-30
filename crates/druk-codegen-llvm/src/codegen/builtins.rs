use inkwell::values::{FunctionValue, StructValue};

use druk_diagnostics::Diagnostic;
use druk_ir::{BuiltinOp, IrExpr};

use super::CodeGen;

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn compile_builtin(
        &mut self,
        op: BuiltinOp,
        args: &[IrExpr],
        function: FunctionValue<'ctx>,
    ) -> Result<StructValue<'ctx>, Diagnostic> {
        let arg_refs: Vec<&IrExpr> = args.iter().collect();
        let values = self.compile_rooted_siblings(&arg_refs, function)?;
        let arg_ptrs: Vec<_> = values.iter().map(|value| self.value_ptr(*value)).collect();

        let runtime_fn = match op {
            BuiltinOp::Len => self.runtime.len,
            BuiltinOp::Push => self.runtime.push,
            BuiltinOp::Pop => self.runtime.pop,
            BuiltinOp::TypeOf => self.runtime.typeof_,
            BuiltinOp::Input => self.runtime.input,
            BuiltinOp::Argv => self.runtime.argv,
            BuiltinOp::Argc => self.runtime.argc,
        };
        let call_args: Vec<_> = arg_ptrs.iter().map(|ptr| (*ptr).into()).collect();
        Ok(self.call_with_out_param(runtime_fn, &call_args, "call_builtin"))
    }
}
