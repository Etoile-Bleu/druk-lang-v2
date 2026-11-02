use inkwell::basic_block::BasicBlock;
use inkwell::values::FunctionValue;

use druk_diagnostics::{ice, Diagnostic};
use druk_ir::{IrExpr, IrStmt};

use super::{unwrap_build, CodeGen};

impl<'ctx> CodeGen<'ctx> {
    pub(crate) fn compile_stmt(
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
                let ptr = self.value_ptr(function, value);
                unwrap_build(self.builder.build_call(
                    self.runtime.print,
                    &[ptr.into()],
                    "call_print",
                ));
            }
            IrStmt::Return(value) => {
                let result = match value {
                    Some(expr) => self.compile_expr(expr, function)?,
                    None => self.const_value(druk_runtime::TAG_NIL, 0),
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
            IrStmt::Break => {
                let (_, break_target) = self.current_loop_targets("break");
                unwrap_build(self.builder.build_unconditional_branch(break_target));
            }
            IrStmt::Continue => {
                let (continue_target, _) = self.current_loop_targets("continue");
                unwrap_build(self.builder.build_unconditional_branch(continue_target));
            }
        }
        Ok(())
    }

    /// The innermost enclosing loop's `(continue_target, break_target)`
    /// pair. HIR rejects a `break`/`continue` outside of a loop before
    /// codegen ever runs (see `druk-hir`'s `loop_depth` check), so an empty
    /// `loop_stack` here means an earlier pass has a bug, not that the
    /// user's program is invalid.
    fn current_loop_targets(&self, what: &str) -> (BasicBlock<'ctx>, BasicBlock<'ctx>) {
        *self
            .loop_stack
            .last()
            .unwrap_or_else(|| ice!("codegen reached a '{what}' outside of a loop"))
    }

    fn compile_if(
        &mut self,
        cond: &IrExpr,
        then_branch: &[IrStmt],
        else_branch: &[IrStmt],
        function: FunctionValue<'ctx>,
    ) -> Result<(), Diagnostic> {
        let cond_value = self.compile_expr(cond, function)?;
        let cond_bool = self.is_truthy(function, cond_value);

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
        let cond_bool = self.is_truthy(function, cond_value);
        unwrap_build(
            self.builder
                .build_conditional_branch(cond_bool, body_bb, end_bb),
        );

        self.builder.position_at_end(body_bb);
        self.loop_stack.push((cond_bb, end_bb));
        for stmt in body {
            if let Err(diagnostic) = self.compile_stmt(stmt, function) {
                self.loop_stack.pop();
                return Err(diagnostic);
            }
        }
        self.loop_stack.pop();
        if !self.is_terminated() {
            unwrap_build(self.builder.build_unconditional_branch(cond_bb));
        }

        self.builder.position_at_end(end_bb);
        Ok(())
    }
}
