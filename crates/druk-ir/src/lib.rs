//! Lowers a checked [`Hir`] into a small, backend-agnostic intermediate
//! representation: structured control flow (no basic blocks), locals
//! addressed by flat per-function slot indices, and every runtime value
//! represented uniformly so a backend does not need type-directed codegen
//! for the handful of Druk types.
//!
//! Scope for this first lowering: the ten bundled example programs plus
//! structs and single-level lambda capture. Builtins (`len`, `push`, ...)
//! are not supported yet and are reported as diagnostics rather than
//! silently miscompiled, since none of the bundled examples need them. A
//! lambda can capture a variable from its immediately enclosing scope
//! (one level up); a lambda nested inside another lambda trying to reach
//! two levels up is also reported as a diagnostic rather than silently
//! miscompiled, since no bundled example needs that either.

use std::collections::HashMap;

use druk_ast::{Ast, Expr, ExprId, Literal, MatchArm, Pattern, Stmt, StmtId};
use druk_diagnostics::{ice, Diagnostic};
use druk_hir::Hir;

pub use druk_ast::{BinOp, UnOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    pub locals: u32,
    pub body: Vec<IrStmt>,
    pub capture_slots: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub functions: Vec<Function>,
    pub main: Function,
}

#[derive(Debug, Clone)]
pub enum IrStmt {
    Expr(IrExpr),
    Print(IrExpr),
    If {
        cond: IrExpr,
        then_branch: Vec<IrStmt>,
        else_branch: Vec<IrStmt>,
    },
    While {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    Return(Option<IrExpr>),
    Block(Vec<IrStmt>),
}

#[derive(Debug, Clone)]
pub enum IrExpr {
    ConstInt(i64),
    ConstStr(String),
    ConstBool(bool),
    ConstNil,
    FunctionRef(FunctionId),
    GetLocal(u32),
    SetLocal(u32, Box<IrExpr>),
    Unary(UnOp, Box<IrExpr>),
    Binary(BinOp, Box<IrExpr>, Box<IrExpr>),
    Call(Box<IrExpr>, Vec<IrExpr>),
    ArrayLiteral(Vec<IrExpr>),
    ArrayGet(Box<IrExpr>, Box<IrExpr>),
    ArraySet(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>),
    StructLiteral(Vec<(String, IrExpr)>),
    FieldGet(Box<IrExpr>, String),
    FieldSet(Box<IrExpr>, String, Box<IrExpr>),
    MakeClosure(FunctionId, Vec<IrExpr>),
}

pub fn lower(hir: &Hir) -> Result<Module, Vec<Diagnostic>> {
    let ast = &hir.ast;

    let mut function_ids = HashMap::new();
    let mut functions = Vec::new();
    for &id in &ast.top_level {
        if let Stmt::FunctionDecl { name, params, .. } = ast.stmt(id) {
            let fid = FunctionId(functions.len() as u32);
            function_ids.insert(name.clone(), fid);
            functions.push(Function {
                name: name.clone(),
                arity: params.len(),
                locals: 0,
                body: Vec::new(),
                capture_slots: Vec::new(),
            });
        }
    }

    let mut lowerer = Lowerer {
        ast,
        function_ids,
        functions,
        diagnostics: Vec::new(),
        scope_stack: Vec::new(),
        next_slot_stack: Vec::new(),
        captures_stack: Vec::new(),
        is_lambda_stack: Vec::new(),
    };

    let mut main_body = Vec::new();
    lowerer.push_function_scope(false);
    for &id in &ast.top_level {
        match ast.stmt(id) {
            Stmt::FunctionDecl { name, params, body } => {
                let fid = lowerer.function_ids[name];
                lowerer.push_function_scope(false);
                for param in params {
                    lowerer.declare_slot(&param.name);
                }
                let lowered_body = lowerer.lower_block(body);
                let (locals, _) = lowerer.pop_function_scope();
                lowerer.functions[fid.0 as usize].locals = locals;
                lowerer.functions[fid.0 as usize].body = lowered_body;
            }
            _ => {
                let stmt_ir = lowerer.lower_stmt(id);
                main_body.push(stmt_ir);
            }
        }
    }
    let (main_locals, _) = lowerer.pop_function_scope();

    if lowerer.diagnostics.is_empty() {
        Ok(Module {
            functions: lowerer.functions,
            main: Function {
                name: "main".to_string(),
                arity: 0,
                locals: main_locals,
                body: main_body,
                capture_slots: Vec::new(),
            },
        })
    } else {
        Err(lowerer.diagnostics)
    }
}

struct Lowerer<'a> {
    ast: &'a Ast,
    function_ids: HashMap<String, FunctionId>,
    functions: Vec<Function>,
    diagnostics: Vec<Diagnostic>,
    scope_stack: Vec<Vec<HashMap<String, u32>>>,
    next_slot_stack: Vec<u32>,
    captures_stack: Vec<Vec<(String, u32, u32)>>,
    is_lambda_stack: Vec<bool>,
}

impl<'a> Lowerer<'a> {
    fn push_function_scope(&mut self, is_lambda: bool) {
        self.scope_stack.push(vec![HashMap::new()]);
        self.next_slot_stack.push(0);
        self.captures_stack.push(Vec::new());
        self.is_lambda_stack.push(is_lambda);
    }

    fn pop_function_scope(&mut self) -> (u32, Vec<(String, u32, u32)>) {
        self.scope_stack.pop();
        self.is_lambda_stack.pop();
        let locals = self.next_slot_stack.pop().unwrap_or(0);
        let captures = self.captures_stack.pop().unwrap_or_default();
        (locals, captures)
    }

    fn resolve_or_add_capture(&mut self, name: &str) -> Option<u32> {
        if !*self.is_lambda_stack.last()? {
            return None;
        }
        if let Some((_, _, slot)) = self
            .captures_stack
            .last()?
            .iter()
            .find(|(capture_name, _, _)| capture_name == name)
        {
            return Some(*slot);
        }

        let enclosing_index = self.scope_stack.len().checked_sub(2)?;
        let enclosing_slot = {
            let scopes = &self.scope_stack[enclosing_index];
            scopes
                .iter()
                .rev()
                .find_map(|scope| scope.get(name).copied())
        }?;

        let local_slot = self.declare_slot(name);
        match self.captures_stack.last_mut() {
            Some(captures) => captures.push((name.to_string(), enclosing_slot, local_slot)),
            None => ice!("captures_stack was just confirmed non-empty above"),
        }
        Some(local_slot)
    }

    fn push_block_scope(&mut self) {
        if let Some(scopes) = self.scope_stack.last_mut() {
            scopes.push(HashMap::new());
        }
    }

    fn pop_block_scope(&mut self) {
        if let Some(scopes) = self.scope_stack.last_mut() {
            scopes.pop();
        }
    }

    fn declare_slot(&mut self, name: &str) -> u32 {
        let slot = self.next_slot_stack.last().copied().unwrap_or(0);
        if let Some(top) = self.next_slot_stack.last_mut() {
            *top += 1;
        }
        if let Some(scopes) = self.scope_stack.last_mut() {
            if let Some(scope) = scopes.last_mut() {
                scope.insert(name.to_string(), slot);
            }
        }
        slot
    }

    fn resolve_local(&self, name: &str) -> Option<u32> {
        let scopes = self.scope_stack.last()?;
        for scope in scopes.iter().rev() {
            if let Some(&slot) = scope.get(name) {
                return Some(slot);
            }
        }
        None
    }

    fn lower_block(&mut self, stmts: &[StmtId]) -> Vec<IrStmt> {
        stmts.iter().map(|&id| self.lower_stmt(id)).collect()
    }

    fn lower_stmt(&mut self, id: StmtId) -> IrStmt {
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

    fn lower_expr(&mut self, id: ExprId) -> IrExpr {
        let ast = self.ast;
        match ast.expr(id) {
            Expr::Literal(literal) => lower_literal(literal),
            Expr::Ident(name) => {
                if let Some(slot) = self.resolve_local(name) {
                    IrExpr::GetLocal(slot)
                } else if let Some(slot) = self.resolve_or_add_capture(name) {
                    IrExpr::GetLocal(slot)
                } else if let Some(&fid) = self.function_ids.get(name) {
                    IrExpr::FunctionRef(fid)
                } else {
                    self.diagnostics.push(Diagnostic::error(format!(
                        "'{name}' is not supported by the compiler yet"
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
            Expr::Call { callee, args } => {
                let callee_ir = self.lower_expr(*callee);
                let args_ir = args.iter().map(|&arg| self.lower_expr(arg)).collect();
                IrExpr::Call(Box::new(callee_ir), args_ir)
            }
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

    fn lower_assign(&mut self, target: ExprId, value_ir: IrExpr) -> IrExpr {
        let ast = self.ast;
        match ast.expr(target) {
            Expr::Ident(name) => {
                if let Some(slot) = self.resolve_local(name) {
                    IrExpr::SetLocal(slot, Box::new(value_ir))
                } else {
                    self.diagnostics
                        .push(Diagnostic::error(format!("cannot assign to '{name}'")));
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
                self.diagnostics
                    .push(Diagnostic::error("invalid assignment target"));
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
        function.body = vec![IrStmt::Return(Some(body_ir))];
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

fn lower_literal(literal: &Literal) -> IrExpr {
    match literal {
        Literal::Integer(value) => IrExpr::ConstInt(*value),
        Literal::Str(value) => IrExpr::ConstStr(value.clone()),
        Literal::Bool(value) => IrExpr::ConstBool(*value),
        Literal::Nil => IrExpr::ConstNil,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower_source(source: &str) -> Result<Module, Vec<Diagnostic>> {
        let (ast, parse_diagnostics) = druk_parser::parse(source);
        assert!(
            parse_diagnostics.is_empty(),
            "unexpected parse diagnostics: {parse_diagnostics:?}"
        );
        let hir = druk_hir::check(ast).expect("source should type-check");
        lower(&hir)
    }

    #[test]
    fn lowers_every_bundled_example_without_diagnostics() {
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let entries = std::fs::read_dir(examples_dir).expect("examples dir exists");
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
                let source = std::fs::read_to_string(&path).expect("example file is readable");
                let result = lower_source(&source);
                assert!(
                    result.is_ok(),
                    "unexpected diagnostics lowering {path:?}: {result:?}"
                );
            }
        }
    }

    #[test]
    fn lowers_a_recursive_function_with_its_own_slot() {
        let module = lower_source(
            "ལས་འགན་ fib(གྲངས་ n) { གལ་སྲིད་ (n <= ༡) སླར་ལོག་ n; སླར་ལོག་ fib(n - ༡) + fib(n - ༢); } བཀོད་ fib(༥);",
        )
        .expect("should lower");
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].name, "fib");
        assert_eq!(module.functions[0].arity, 1);
        assert_eq!(module.main.body.len(), 1);
    }

    #[test]
    fn lowers_struct_literal_field_access_and_field_assignment() {
        let module = lower_source("གྲངས་ p = {x: ༡, y: ༢}; བཀོད་ p.x; p.y = ༩; བཀོད་ p.y;")
            .expect("should lower");
        assert_eq!(module.main.body.len(), 4);

        match &module.main.body[0] {
            IrStmt::Expr(IrExpr::SetLocal(_, value)) => match value.as_ref() {
                IrExpr::StructLiteral(fields) => {
                    assert_eq!(fields.len(), 2);
                    assert_eq!(fields[0].0, "x");
                    assert_eq!(fields[1].0, "y");
                }
                other => panic!("expected StructLiteral, got {other:?}"),
            },
            other => panic!("expected a SetLocal statement, got {other:?}"),
        }

        match &module.main.body[1] {
            IrStmt::Print(IrExpr::FieldGet(_, field)) => assert_eq!(field, "x"),
            other => panic!("expected Print(FieldGet), got {other:?}"),
        }

        match &module.main.body[2] {
            IrStmt::Expr(IrExpr::FieldSet(_, field, _)) => assert_eq!(field, "y"),
            other => panic!("expected Expr(FieldSet), got {other:?}"),
        }
    }

    #[test]
    fn lowers_a_lambda_into_a_separate_function() {
        let module =
            lower_source("གྲངས་ double = n -> n * ༢; བཀོད་ double(༣);").expect("should lower");
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].arity, 1);
        assert!(module.functions[0].capture_slots.is_empty());
        match &module.main.body[0] {
            IrStmt::Expr(IrExpr::SetLocal(_, value)) => {
                assert!(matches!(value.as_ref(), IrExpr::FunctionRef(_)));
            }
            other => panic!("expected a SetLocal statement, got {other:?}"),
        }
    }

    #[test]
    fn lowers_a_lambda_capturing_an_enclosing_variable() {
        let module = lower_source("གྲངས་ n = ༣; གྲངས་ add_n = m -> m + n; བཀོད་ add_n(༢);")
            .expect("should lower");
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].capture_slots.len(), 1);

        match &module.main.body[1] {
            IrStmt::Expr(IrExpr::SetLocal(_, value)) => match value.as_ref() {
                IrExpr::MakeClosure(_, captures) => {
                    assert_eq!(captures.len(), 1);
                    assert!(matches!(captures[0], IrExpr::GetLocal(0)));
                }
                other => panic!("expected MakeClosure, got {other:?}"),
            },
            other => panic!("expected a SetLocal statement, got {other:?}"),
        }
    }
}
