use druk_ast::{ExprId, MatchArm, Pattern, Stmt, StmtId};
use druk_diagnostics::Diagnostic;

use crate::{lower_literal, BinOp, BuiltinOp, IrExpr, IrStmt, Lowerer};

impl<'a> Lowerer<'a> {
    pub(crate) fn lower_block(&mut self, stmts: &[StmtId]) -> Vec<IrStmt> {
        stmts.iter().map(|&id| self.lower_stmt(id)).collect()
    }

    pub(crate) fn lower_stmt(&mut self, id: StmtId) -> IrStmt {
        let ast = self.ast;
        match ast.stmt(id) {
            Stmt::Let { name, value, .. } => {
                let value_ir = match value {
                    Some(value_id) => self.lower_expr(*value_id),
                    None => IrExpr::ConstNil,
                };
                let slot = self.declare_slot(name);
                IrStmt::Expr(IrExpr::SetLocal(slot, Box::new(value_ir)))
            }
            Stmt::Expr(expr) => IrStmt::Expr(self.lower_expr(*expr)),
            Stmt::Print(expr) => IrStmt::Print(self.lower_expr(*expr)),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(*condition);
                self.push_block_scope();
                let then_ir = self.lower_block(then_branch);
                self.pop_block_scope();
                let else_ir = match else_branch {
                    Some(branch) => {
                        self.push_block_scope();
                        let lowered = self.lower_block(branch);
                        self.pop_block_scope();
                        lowered
                    }
                    None => Vec::new(),
                };
                IrStmt::If {
                    cond,
                    then_branch: then_ir,
                    else_branch: else_ir,
                }
            }
            Stmt::While { condition, body } => {
                let cond = self.lower_expr(*condition);
                self.push_block_scope();
                let body_ir = self.lower_block(body);
                self.pop_block_scope();
                IrStmt::While {
                    cond,
                    body: body_ir,
                }
            }
            Stmt::For {
                init,
                condition,
                post,
                body,
            } => {
                self.push_block_scope();
                let init_ir = Box::new(self.lower_stmt(*init));
                let cond = self.lower_expr(*condition);
                self.push_block_scope();
                let body_ir = self.lower_block(body);
                self.pop_block_scope();
                let post_ir = self.lower_expr(*post);
                self.pop_block_scope();
                IrStmt::For {
                    init: init_ir,
                    cond,
                    post: post_ir,
                    body: body_ir,
                }
            }
            Stmt::ForEach {
                var,
                iterable,
                body,
            } => {
                let iterable_ir = self.lower_expr(*iterable);
                self.push_block_scope();
                let arr_slot = self.declare_slot("$foreach_arr");
                let idx_slot = self.declare_slot("$foreach_idx");
                let var_slot = self.declare_slot(var);

                // A plain `While` has no separate post-body step, so a
                // `continue` inside `body` would skip straight back to the
                // condition and never advance `idx_slot`, looping forever
                // on the same element. `IrStmt::For` exists exactly to give
                // `continue` a `post` step to land on instead; see
                // `compile_for`'s own doc comment in druk-codegen-llvm.
                let init = Box::new(IrStmt::Block(vec![
                    IrStmt::Expr(IrExpr::SetLocal(arr_slot, Box::new(iterable_ir))),
                    IrStmt::Expr(IrExpr::SetLocal(idx_slot, Box::new(IrExpr::ConstInt(0)))),
                ]));
                let cond = IrExpr::Binary(
                    BinOp::Lt,
                    Box::new(IrExpr::GetLocal(idx_slot)),
                    Box::new(IrExpr::Builtin(
                        BuiltinOp::Len,
                        vec![IrExpr::GetLocal(arr_slot)],
                    )),
                );
                let post = IrExpr::SetLocal(
                    idx_slot,
                    Box::new(IrExpr::Binary(
                        BinOp::Add,
                        Box::new(IrExpr::GetLocal(idx_slot)),
                        Box::new(IrExpr::ConstInt(1)),
                    )),
                );

                let mut body_ir = vec![IrStmt::Expr(IrExpr::SetLocal(
                    var_slot,
                    Box::new(IrExpr::ArrayGet(
                        Box::new(IrExpr::GetLocal(arr_slot)),
                        Box::new(IrExpr::GetLocal(idx_slot)),
                    )),
                ))];
                body_ir.extend(self.lower_block(body));
                self.pop_block_scope();

                IrStmt::For {
                    init,
                    cond,
                    post,
                    body: body_ir,
                }
            }
            Stmt::Break => IrStmt::Break,
            Stmt::Continue => IrStmt::Continue,
            Stmt::Return(value) => IrStmt::Return(value.map(|value_id| self.lower_expr(value_id))),
            Stmt::FunctionDecl { .. } => {
                self.diagnostics.push(Diagnostic::error(
                    "nested function declarations are not yet supported",
                ));
                IrStmt::Expr(IrExpr::ConstNil)
            }
            Stmt::Match { subject, arms } => {
                let subject_id = *subject;
                let arms = arms.clone();
                self.lower_match(subject_id, &arms)
            }
        }
    }

    fn lower_match(&mut self, subject: ExprId, arms: &[MatchArm]) -> IrStmt {
        let subject_ir = self.lower_expr(subject);
        let slot = self.declare_slot("$match");
        let set_stmt = IrStmt::Expr(IrExpr::SetLocal(slot, Box::new(subject_ir)));

        let mut chain: Vec<IrStmt> = Vec::new();
        for arm in arms.iter().rev() {
            match &arm.pattern {
                Pattern::Wildcard => {
                    chain = vec![self.lower_stmt(arm.body)];
                }
                Pattern::Literal(literal) => {
                    let literal_ir = lower_literal(literal);
                    let cond = IrExpr::Binary(
                        BinOp::Eq,
                        Box::new(IrExpr::GetLocal(slot)),
                        Box::new(literal_ir),
                    );
                    let then_branch = vec![self.lower_stmt(arm.body)];
                    chain = vec![IrStmt::If {
                        cond,
                        then_branch,
                        else_branch: chain,
                    }];
                }
            }
        }

        IrStmt::Block(vec![set_stmt, IrStmt::Block(chain)])
    }
}
