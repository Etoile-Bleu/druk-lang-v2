use druk_ast::{Expr, ExprId};
use druk_diagnostics::Diagnostic;

use crate::{lower_literal, BuiltinOp, Function, FunctionId, IrExpr, Lowerer};

impl<'a> Lowerer<'a> {
    pub(crate) fn lower_expr(&mut self, id: ExprId) -> IrExpr {
        let ast = self.ast;
        match ast.expr(id) {
            Expr::Literal(literal) => lower_literal(literal),
            Expr::Ident(name) => {
                if let Some(expr) = self.try_resolve_name(name) {
                    expr
                } else if let Some(op) = builtin_global_op(name) {
                    IrExpr::Builtin(op, Vec::new())
                } else {
                    self.diagnostics.push(Diagnostic::error(format!(
                        "Druk doesn't know what '{name}' is"
                    )));
                    IrExpr::ConstNil
                }
            }
            Expr::Unary { op, operand } => {
                let operand_ir = self.lower_expr(*operand);
                IrExpr::Unary(*op, Box::new(operand_ir))
            }
            Expr::Binary { op, lhs, rhs } => {
                let lhs_ir = self.lower_expr(*lhs);
                let rhs_ir = self.lower_expr(*rhs);
                IrExpr::Binary(*op, Box::new(lhs_ir), Box::new(rhs_ir))
            }
            Expr::Assign { target, value } => {
                let value_ir = self.lower_expr(*value);
                let target_id = *target;
                self.lower_assign(target_id, value_ir)
            }
            Expr::Call { callee, args } => self.lower_call(*callee, args),
            Expr::Index { object, index } => {
                let object_ir = self.lower_expr(*object);
                let index_ir = self.lower_expr(*index);
                IrExpr::ArrayGet(Box::new(object_ir), Box::new(index_ir))
            }
            Expr::FieldAccess { object, field } => {
                let object_ir = self.lower_expr(*object);
                IrExpr::FieldGet(Box::new(object_ir), field.clone())
            }
            Expr::ArrayLiteral(elements) => {
                let elements_ir = elements
                    .iter()
                    .map(|&element| self.lower_expr(element))
                    .collect();
                IrExpr::ArrayLiteral(elements_ir)
            }
            Expr::StructLiteral(fields) => {
                let fields_ir = fields
                    .iter()
                    .map(|(name, value)| (name.clone(), self.lower_expr(*value)))
                    .collect();
                IrExpr::StructLiteral(fields_ir)
            }
            Expr::Lambda { params, body } => {
                let params = params.clone();
                let body_id = *body;
                self.lower_lambda(&params, body_id)
            }
        }
    }

    /// A call's callee gets one extra resolution step beyond an ordinary
    /// `Expr::Ident`: a name that isn't a local, a capture, or a top-level
    /// function might still be a Dzongkha builtin (`len`, `push`, ...),
    /// which has no `FunctionId` of its own and lowers straight to
    /// [`IrExpr::Builtin`] instead of the generic indirect-call shape.
    /// `try_resolve_name` is called at most once per callee reference so a
    /// capture it resolves is never declared twice.
    fn lower_call(&mut self, callee: ExprId, args: &[ExprId]) -> IrExpr {
        if let Expr::Ident(name) = self.ast.expr(callee) {
            let name = name.clone();
            if let Some(callee_ir) = self.try_resolve_name(&name) {
                let args_ir = args.iter().map(|&arg| self.lower_expr(arg)).collect();
                return IrExpr::Call(Box::new(callee_ir), args_ir);
            }
            if let Some(op) = builtin_fn_op(&name) {
                let args_ir = args.iter().map(|&arg| self.lower_expr(arg)).collect();
                return IrExpr::Builtin(op, args_ir);
            }
            self.diagnostics.push(Diagnostic::error(format!(
                "Druk doesn't know what '{name}' is"
            )));
            return IrExpr::ConstNil;
        }
        let callee_ir = self.lower_expr(callee);
        let args_ir = args.iter().map(|&arg| self.lower_expr(arg)).collect();
        IrExpr::Call(Box::new(callee_ir), args_ir)
    }

    fn lower_assign(&mut self, target: ExprId, value_ir: IrExpr) -> IrExpr {
        let ast = self.ast;
        match ast.expr(target) {
            Expr::Ident(name) => {
                if let Some(slot) = self.resolve_local(name) {
                    IrExpr::SetLocal(slot, Box::new(value_ir))
                } else {
                    self.diagnostics.push(Diagnostic::error(format!(
                        "can't assign to '{name}': there is no variable with that name"
                    )));
                    IrExpr::ConstNil
                }
            }
            Expr::Index { object, index } => {
                let object_ir = self.lower_expr(*object);
                let index_ir = self.lower_expr(*index);
                IrExpr::ArraySet(Box::new(object_ir), Box::new(index_ir), Box::new(value_ir))
            }
            Expr::FieldAccess { object, field } => {
                let object_ir = self.lower_expr(*object);
                IrExpr::FieldSet(Box::new(object_ir), field.clone(), Box::new(value_ir))
            }
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "the left side of '=' must be a variable, a list item (like x[0]), or a struct field (like x.y)",
                ));
                IrExpr::ConstNil
            }
        }
    }

    fn lower_lambda(&mut self, params: &[String], body: ExprId) -> IrExpr {
        let fid = FunctionId(self.functions.len() as u32);
        self.functions.push(Function {
            name: format!("lambda${}", fid.0),
            arity: params.len(),
            locals: 0,
            body: Vec::new(),
            capture_slots: Vec::new(),
        });

        self.push_function_scope(true);
        for param in params {
            self.declare_slot(param);
        }
        let body_ir = self.lower_expr(body);
        let (locals, captures) = self.pop_function_scope();

        let function = &mut self.functions[fid.0 as usize];
        function.locals = locals;
        function.body = vec![crate::IrStmt::Return(Some(body_ir))];
        function.capture_slots = captures.iter().map(|(_, _, slot)| *slot).collect();

        if captures.is_empty() {
            IrExpr::FunctionRef(fid)
        } else {
            let capture_values = captures
                .into_iter()
                .map(|(_, enclosing_slot, _)| IrExpr::GetLocal(enclosing_slot))
                .collect();
            IrExpr::MakeClosure(fid, capture_values)
        }
    }
}

/// The Dzongkha builtin *functions* (callable with `name(...)`), as opposed
/// to the two bare-identifier globals `builtin_global_op` resolves.
/// `keys`/`values`/`contains` are deliberately absent, see [`BuiltinOp`]'s
/// doc comment.
fn builtin_fn_op(name: &str) -> Option<BuiltinOp> {
    Some(match name {
        "len" => BuiltinOp::Len,
        "push" => BuiltinOp::Push,
        "pop" => BuiltinOp::Pop,
        "typeof" => BuiltinOp::TypeOf,
        "input" => BuiltinOp::Input,
        _ => return None,
    })
}

/// `argv`/`argc` are read as plain identifiers, never called, so they
/// resolve through `Expr::Ident` rather than `Expr::Call`.
fn builtin_global_op(name: &str) -> Option<BuiltinOp> {
    Some(match name {
        "argv" => BuiltinOp::Argv,
        "argc" => BuiltinOp::Argc,
        _ => return None,
    })
}
