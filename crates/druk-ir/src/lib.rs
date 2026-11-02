//! Lowers a checked [`Hir`] into a small, backend-agnostic intermediate
//! representation: structured control flow (no basic blocks), locals
//! addressed by flat per-function slot indices, and every runtime value
//! represented uniformly so a backend does not need type-directed codegen
//! for the handful of Druk types.
//!
//! Scope for this first lowering: the ten bundled example programs plus
//! structs, single-level lambda capture, and the Dzongkha builtins (`len`,
//! `push`, `pop`, `typeof`, `input`, `argv`, `argc`). `keys`/`values`/
//! `contains` depend on a dictionary type that doesn't exist yet and are
//! reported as diagnostics rather than silently miscompiled, since no
//! bundled example needs them. A lambda can capture a variable from its
//! immediately enclosing scope (one level up); a lambda nested inside
//! another lambda trying to reach two levels up is also reported as a
//! diagnostic rather than silently miscompiled, since no bundled example
//! needs that either.
//!
//! Split by responsibility into sibling files: `statements` (control flow)
//! and `expressions` (everything else, including builtin-call resolution).
//! Scope and capture bookkeeping (`push_function_scope`, `resolve_local`,
//! `resolve_or_add_capture`, ...) stays here in `lib.rs` alongside the
//! [`Lowerer`] struct itself, the same way `druk-parser`'s token-stream
//! helpers stay alongside its `Parser` struct.

mod expressions;
mod statements;

use std::collections::HashMap;

use druk_ast::{Ast, Literal, Stmt};
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
    /// Kept as its own IR shape rather than desugared to `While` plus a
    /// trailing `post` statement appended to `body`: `continue` inside a
    /// `for` loop must still run `post` before the next `cond` check, which
    /// needs its own codegen target distinct from `While`'s (whose
    /// `continue` branches straight to `cond`). See `druk-codegen-llvm`'s
    /// `compile_for`.
    For {
        init: Box<IrStmt>,
        cond: IrExpr,
        post: IrExpr,
        body: Vec<IrStmt>,
    },
    Break,
    Continue,
    Return(Option<IrExpr>),
    Block(Vec<IrStmt>),
}

/// A Dzongkha builtin function, lowered to its own IR node (rather than an
/// ordinary [`IrExpr::Call`] through a [`FunctionId`]) since none of them are
/// Druk-defined functions with a compiled body: codegen dispatches each one
/// straight to a fixed `druk-runtime` entry point. `keys`/`values`/
/// `contains` are deliberately absent: they depend on a dictionary type this
/// compiler doesn't have yet, so a call to one of them still falls through
/// to the "not supported by the compiler yet" diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinOp {
    Len,
    Push,
    Pop,
    TypeOf,
    Input,
    Argv,
    Argc,
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
    Builtin(BuiltinOp, Vec<IrExpr>),
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
    /// Per enclosing-function-scope: `(name, enclosing_slot, local_slot)`
    /// for every variable a lambda directly inside it has captured so far.
    captures_stack: Vec<Vec<(String, u32, u32)>>,
    /// Parallel to `scope_stack`/`next_slot_stack`: whether the function
    /// scope at that depth is a lambda (capture-eligible) or a named
    /// top-level function (never capture-eligible, even though it may sit
    /// at the same nesting depth as a lambda would).
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

    /// Resolves `name` against the current lambda's immediately enclosing
    /// scope (one level up), declaring it as a new capture the first time
    /// it's seen and reusing the same local slot on every later reference
    /// within the same lambda. Returns `None` (without declaring anything)
    /// when the current scope isn't a lambda, or `name` isn't found one
    /// level up either.
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

    /// Resolves `name` against a local, a captured enclosing variable, or a
    /// top-level function, in that order, without touching `diagnostics` or
    /// falling back to a builtin. `None` means `name` is none of those,
    /// which the caller (an `Expr::Ident` or an `Expr::Call`'s callee) is
    /// responsible for either resolving as a builtin or reporting.
    fn try_resolve_name(&mut self, name: &str) -> Option<IrExpr> {
        if let Some(slot) = self.resolve_local(name) {
            Some(IrExpr::GetLocal(slot))
        } else if let Some(slot) = self.resolve_or_add_capture(name) {
            Some(IrExpr::GetLocal(slot))
        } else if let Some(&fid) = self.function_ids.get(name) {
            Some(IrExpr::FunctionRef(fid))
        } else {
            None
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
    fn lowers_a_for_loop_to_its_own_init_cond_post_and_body() {
        let module =
            lower_source("རེ་རེར་ (གྲངས་ i = ༠; i < ༥; i = i + ༡) { བཀོད་ i; }").expect("should lower");
        match &module.main.body[0] {
            IrStmt::For {
                init,
                cond,
                post,
                body,
            } => {
                assert!(matches!(
                    init.as_ref(),
                    IrStmt::Expr(IrExpr::SetLocal(0, _))
                ));
                assert!(matches!(cond, IrExpr::Binary(BinOp::Lt, _, _)));
                assert!(matches!(post, IrExpr::SetLocal(0, _)));
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected For statement, got {other:?}"),
        }
    }

    #[test]
    fn lowers_break_and_continue_to_their_own_ir_statements() {
        let module = lower_source("ཡང་བསྐྱར་ (བདེན་པ་) { ཆད་; མུ་མཐུད་; }").expect("should lower");
        match &module.main.body[0] {
            IrStmt::While { body, .. } => {
                assert_eq!(body.len(), 2);
                assert!(matches!(body[0], IrStmt::Break));
                assert!(matches!(body[1], IrStmt::Continue));
            }
            other => panic!("expected While statement, got {other:?}"),
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

    #[test]
    fn lowers_len_push_pop_typeof_and_input_calls_to_builtin_nodes() {
        let module = lower_source(
            "གྲངས་[] xs = [༡, ༢]; བཀོད་ རིང་ཚད(xs); སྣོན་(xs, ༣); བཏོན་(xs); བཀོད་ རིགས་(xs); བཀོད་ ནང་འཇུག་();",
        )
        .expect("should lower");

        match &module.main.body[1] {
            IrStmt::Print(IrExpr::Builtin(BuiltinOp::Len, args)) => assert_eq!(args.len(), 1),
            other => panic!("expected Print(Builtin(Len)), got {other:?}"),
        }
        match &module.main.body[2] {
            IrStmt::Expr(IrExpr::Builtin(BuiltinOp::Push, args)) => assert_eq!(args.len(), 2),
            other => panic!("expected Expr(Builtin(Push)), got {other:?}"),
        }
        match &module.main.body[3] {
            IrStmt::Expr(IrExpr::Builtin(BuiltinOp::Pop, args)) => assert_eq!(args.len(), 1),
            other => panic!("expected Expr(Builtin(Pop)), got {other:?}"),
        }
        match &module.main.body[4] {
            IrStmt::Print(IrExpr::Builtin(BuiltinOp::TypeOf, args)) => assert_eq!(args.len(), 1),
            other => panic!("expected Print(Builtin(TypeOf)), got {other:?}"),
        }
        match &module.main.body[5] {
            IrStmt::Print(IrExpr::Builtin(BuiltinOp::Input, args)) => assert!(args.is_empty()),
            other => panic!("expected Print(Builtin(Input)), got {other:?}"),
        }
    }

    #[test]
    fn lowers_argv_and_argc_globals_to_builtin_nodes() {
        let module = lower_source("བཀོད་ ནང་འཇུག་ཐོ་; བཀོད་ ནང་འཇུག་གྲངས་;").expect("should lower");
        assert!(matches!(
            &module.main.body[0],
            IrStmt::Print(IrExpr::Builtin(BuiltinOp::Argv, _))
        ));
        assert!(matches!(
            &module.main.body[1],
            IrStmt::Print(IrExpr::Builtin(BuiltinOp::Argc, _))
        ));
    }

    #[test]
    fn a_local_named_like_a_builtin_shadows_it() {
        let module = lower_source("གྲངས་ len = ༥; བཀོད་ len;").expect("should lower");
        match &module.main.body[1] {
            IrStmt::Print(IrExpr::GetLocal(_)) => {}
            other => panic!("expected Print(GetLocal), got {other:?}"),
        }
    }
}
