//! Best-effort inference of an untyped function parameter's type from how
//! it's used inside that function's own body: a single, local evidence
//! pass, not a unifier, and never reasoning across function boundaries
//! except to borrow an already-known concrete parameter type from a call
//! target (another already-hoisted top-level function, or a builtin).
//!
//! Scope, matching issue #87: this looks at the parameter's own direct
//! occurrences in arithmetic, comparisons, logical operators, indexing,
//! field access, and call arguments. It does not track reassignment,
//! does not descend into a nested lambda or function declaration's own
//! body (either could shadow the parameter's name with a binding of
//! their own), and stops as soon as two occurrences imply conflicting
//! types rather than trying to reconcile them.

use druk_ast::{Ast, BinOp, Expr, ExprId, Stmt, StmtId, UnOp};

use crate::{Checker, Type};

impl<'a> Checker<'a> {
    /// `None` means the evidence was either absent (the parameter is
    /// unused, or only ever passed somewhere that doesn't pin a type
    /// down) or contradictory (used as both a number and a boolean,
    /// say); the caller reports a diagnostic asking for an explicit type
    /// keyword in either case, rather than guessing.
    pub(crate) fn infer_param_type(&self, body: &[StmtId], param_name: &str) -> Option<Type> {
        let mut found = None;
        let mut conflict = false;
        for &stmt in body {
            self.collect_evidence_stmt(stmt, param_name, &mut found, &mut conflict);
        }
        if conflict {
            None
        } else {
            found
        }
    }

    fn record_evidence(found: &mut Option<Type>, conflict: &mut bool, candidate: Type) {
        if *conflict {
            return;
        }
        match found {
            Some(existing) if *existing != candidate => *conflict = true,
            Some(_) => {}
            None => *found = Some(candidate),
        }
    }

    fn collect_evidence_stmt(
        &self,
        id: StmtId,
        name: &str,
        found: &mut Option<Type>,
        conflict: &mut bool,
    ) {
        if *conflict {
            return;
        }
        let ast = self.ast;
        match ast.stmt(id) {
            Stmt::Let { value, .. } => {
                if let Some(value_id) = value {
                    self.collect_evidence_expr(*value_id, name, found, conflict);
                }
            }
            Stmt::Expr(expr) | Stmt::Print(expr) => {
                self.collect_evidence_expr(*expr, name, found, conflict);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.note_bool_condition(*condition, name, found, conflict);
                self.collect_evidence_expr(*condition, name, found, conflict);
                self.collect_evidence_stmts(then_branch, name, found, conflict);
                if let Some(branch) = else_branch {
                    self.collect_evidence_stmts(branch, name, found, conflict);
                }
            }
            Stmt::While { condition, body } => {
                self.note_bool_condition(*condition, name, found, conflict);
                self.collect_evidence_expr(*condition, name, found, conflict);
                self.collect_evidence_stmts(body, name, found, conflict);
            }
            Stmt::For {
                init,
                condition,
                post,
                body,
            } => {
                self.collect_evidence_stmt(*init, name, found, conflict);
                self.note_bool_condition(*condition, name, found, conflict);
                self.collect_evidence_expr(*condition, name, found, conflict);
                self.collect_evidence_expr(*post, name, found, conflict);
                self.collect_evidence_stmts(body, name, found, conflict);
            }
            Stmt::Return(value) => {
                if let Some(value_id) = value {
                    self.collect_evidence_expr(*value_id, name, found, conflict);
                }
            }
            Stmt::Match { subject, arms } => {
                self.collect_evidence_expr(*subject, name, found, conflict);
                for arm in arms {
                    self.collect_evidence_stmt(arm.body, name, found, conflict);
                }
            }
            Stmt::Break | Stmt::Continue => {}
            // A nested function declaration is its own scope; a binding
            // inside it that shares this name refers to that scope, not
            // this parameter, so it's never evidence for it.
            Stmt::FunctionDecl { .. } => {}
        }
    }

    fn collect_evidence_stmts(
        &self,
        stmts: &[StmtId],
        name: &str,
        found: &mut Option<Type>,
        conflict: &mut bool,
    ) {
        for &stmt in stmts {
            self.collect_evidence_stmt(stmt, name, found, conflict);
        }
    }

    fn note_bool_condition(
        &self,
        id: ExprId,
        name: &str,
        found: &mut Option<Type>,
        conflict: &mut bool,
    ) {
        if Self::is_ident(self.ast, id, name) {
            Self::record_evidence(found, conflict, Type::Bool);
        }
    }

    fn collect_evidence_expr(
        &self,
        id: ExprId,
        name: &str,
        found: &mut Option<Type>,
        conflict: &mut bool,
    ) {
        if *conflict {
            return;
        }
        let ast = self.ast;
        match ast.expr(id) {
            Expr::Literal(_) | Expr::Ident(_) => {}
            Expr::Unary { op, operand } => {
                if Self::is_ident(ast, *operand, name) {
                    let candidate = match op {
                        UnOp::Negate => Type::Number,
                        UnOp::Not => Type::Bool,
                    };
                    Self::record_evidence(found, conflict, candidate);
                }
                self.collect_evidence_expr(*operand, name, found, conflict);
            }
            Expr::Binary { op, lhs, rhs } => {
                if let Some(candidate) = binary_operand_evidence(*op) {
                    if Self::is_ident(ast, *lhs, name) || Self::is_ident(ast, *rhs, name) {
                        Self::record_evidence(found, conflict, candidate);
                    }
                }
                self.collect_evidence_expr(*lhs, name, found, conflict);
                self.collect_evidence_expr(*rhs, name, found, conflict);
            }
            Expr::Assign { target, value } => {
                self.collect_evidence_expr(*target, name, found, conflict);
                self.collect_evidence_expr(*value, name, found, conflict);
            }
            Expr::Call { callee, args } => {
                if Self::is_ident(ast, *callee, name) {
                    // `name` is itself the thing being called, e.g.
                    // `op(a, b)` where `op` is an untyped parameter: the
                    // exact signature isn't known, but the call site at
                    // least pins down how many arguments it's called
                    // with.
                    Self::record_evidence(
                        found,
                        conflict,
                        Type::Function {
                            params: vec![Type::Unknown; args.len()],
                            ret: Box::new(Type::Unknown),
                        },
                    );
                }
                if let Some(param_types) = self.resolve_call_param_types(*callee) {
                    for (arg, expected) in args.iter().zip(param_types.iter()) {
                        if *expected != Type::Unknown && Self::is_ident(ast, *arg, name) {
                            Self::record_evidence(found, conflict, expected.clone());
                        }
                    }
                }
                self.collect_evidence_expr(*callee, name, found, conflict);
                for &arg in args {
                    self.collect_evidence_expr(arg, name, found, conflict);
                }
            }
            Expr::Index { object, index } => {
                if Self::is_ident(ast, *index, name) {
                    Self::record_evidence(found, conflict, Type::Number);
                }
                if Self::is_ident(ast, *object, name) {
                    Self::record_evidence(found, conflict, Type::Array(Box::new(Type::Unknown)));
                }
                self.collect_evidence_expr(*object, name, found, conflict);
                self.collect_evidence_expr(*index, name, found, conflict);
            }
            Expr::FieldAccess { object, field: _ } => {
                if Self::is_ident(ast, *object, name) {
                    Self::record_evidence(found, conflict, Type::Struct);
                }
                self.collect_evidence_expr(*object, name, found, conflict);
            }
            Expr::ArrayLiteral(elements) => {
                for &element in elements {
                    self.collect_evidence_expr(element, name, found, conflict);
                }
            }
            Expr::StructLiteral(fields) => {
                for (_, value) in fields {
                    self.collect_evidence_expr(*value, name, found, conflict);
                }
            }
            // A lambda is its own scope, and one of its own parameters
            // could reuse this name, so its body is treated as opaque
            // rather than risking evidence drawn from the wrong binding.
            Expr::Lambda { .. } => {}
        }
    }

    fn is_ident(ast: &Ast, id: ExprId, name: &str) -> bool {
        matches!(ast.expr(id), Expr::Ident(ident) if ident == name)
    }

    /// The parameter types of whatever `callee` resolves to right now
    /// (an already-hoisted top-level function, or a builtin), used only
    /// to borrow one already-known concrete parameter type. `None` when
    /// `callee` isn't a bare identifier, or doesn't resolve to a
    /// function at all. An untyped callee's own parameters are still
    /// `Unknown` at this point (hoisting runs before any body, including
    /// this inference, is checked), so this never reasons through a
    /// chain of untyped parameters, only off ones already pinned down by
    /// an explicit type keyword or a builtin's own fixed signature.
    fn resolve_call_param_types(&self, callee: ExprId) -> Option<Vec<Type>> {
        let Expr::Ident(name) = self.ast.expr(callee) else {
            return None;
        };
        match self.lookup(name)? {
            Type::Function { params, .. } => Some(params),
            _ => None,
        }
    }
}

fn binary_operand_evidence(op: BinOp) -> Option<Type> {
    match op {
        BinOp::Add
        | BinOp::Sub
        | BinOp::Mul
        | BinOp::Div
        | BinOp::Lt
        | BinOp::LtEq
        | BinOp::Gt
        | BinOp::GtEq => Some(Type::Number),
        BinOp::And | BinOp::Or => Some(Type::Bool),
        BinOp::Eq | BinOp::NotEq => None,
    }
}
