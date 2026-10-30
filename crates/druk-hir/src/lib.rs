//! Name resolution and type checking: `check` walks a parsed [`Ast`] once,
//! resolving every identifier against a lexical scope stack and inferring a
//! [`Type`] for every expression, collecting a [`Diagnostic`] for anything
//! that cannot be resolved or does not type-check instead of stopping at the
//! first problem.

use std::collections::HashMap;

use druk_ast::{
    Ast, BaseType, BinOp, Expr, ExprId, Literal, Pattern, Stmt, StmtId, TypeAnnotation, UnOp,
};
use druk_diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    Str,
    Bool,
    Nil,
    Array(Box<Type>),
    Struct,
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    /// A type that could not be determined (an untyped lambda parameter, the
    /// result of a call through an untyped binding, ...). Comparisons
    /// against `Unknown` never produce a diagnostic. It exists so one
    /// unresolved type does not cascade into unrelated false positives.
    Unknown,
}

impl Type {
    fn from_annotation(ty: TypeAnnotation) -> Self {
        let base = match ty.base {
            BaseType::Number => Type::Number,
            BaseType::Str => Type::Str,
            BaseType::Bool => Type::Bool,
        };
        if ty.is_array {
            Type::Array(Box::new(base))
        } else {
            base
        }
    }
}

/// The result of a successful check: the original AST plus the inferred
/// type of every expression in it, indexed by `ExprId::index()`.
#[derive(Debug)]
pub struct Hir {
    pub ast: Ast,
    pub expr_types: Vec<Type>,
}

pub fn check(ast: Ast) -> Result<Hir, Vec<Diagnostic>> {
    let top_level = ast.top_level.clone();
    let (expr_types, diagnostics) = {
        let mut checker = Checker {
            ast: &ast,
            scopes: vec![HashMap::new()],
            expr_types: vec![Type::Unknown; ast.expr_count()],
            diagnostics: Vec::new(),
            in_function: false,
        };
        checker.check_block(&top_level);
        (checker.expr_types, checker.diagnostics)
    };
    if diagnostics.is_empty() {
        Ok(Hir { ast, expr_types })
    } else {
        Err(diagnostics)
    }
}

struct Checker<'a> {
    ast: &'a Ast,
    scopes: Vec<HashMap<String, Type>>,
    expr_types: Vec<Type>,
    diagnostics: Vec<Diagnostic>,
    in_function: bool,
}

fn builtin_type(name: &str) -> Option<Type> {
    let unknown = || Type::Unknown;
    Some(match name {
        "len" => Type::Function {
            params: vec![unknown()],
            ret: Box::new(Type::Number),
        },
        "push" => Type::Function {
            params: vec![unknown(), unknown()],
            ret: Box::new(Type::Nil),
        },
        "pop" => Type::Function {
            params: vec![unknown()],
            ret: Box::new(Type::Unknown),
        },
        "typeof" => Type::Function {
            params: vec![unknown()],
            ret: Box::new(Type::Str),
        },
        "keys" | "values" => Type::Function {
            params: vec![unknown()],
            ret: Box::new(Type::Array(Box::new(Type::Unknown))),
        },
        "contains" => Type::Function {
            params: vec![unknown(), unknown()],
            ret: Box::new(Type::Bool),
        },
        "input" => Type::Function {
            params: vec![],
            ret: Box::new(Type::Str),
        },
        "argv" => Type::Array(Box::new(Type::Str)),
        "argc" => Type::Number,
        _ => return None,
    })
}

impl<'a> Checker<'a> {
    fn declare(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn lookup(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        builtin_type(name)
    }

    fn expect_type(&mut self, actual: &Type, expected: &Type, context: &str) {
        let unknown = Type::Unknown;
        if *actual == unknown || *expected == unknown || actual == expected {
            return;
        }
        self.diagnostics.push(Diagnostic::error(format!(
            "type mismatch in {context}: expected {expected:?}, found {actual:?}"
        )));
    }

    fn hoist_functions(&mut self, stmts: &[StmtId]) {
        let ast = self.ast;
        for &id in stmts {
            if let Stmt::FunctionDecl { name, params, .. } = ast.stmt(id) {
                let param_types = params
                    .iter()
                    .map(|p| p.ty.map(Type::from_annotation).unwrap_or(Type::Unknown))
                    .collect();
                self.declare(
                    name.clone(),
                    Type::Function {
                        params: param_types,
                        ret: Box::new(Type::Unknown),
                    },
                );
            }
        }
    }

    fn check_block(&mut self, stmts: &[StmtId]) {
        self.hoist_functions(stmts);
        for &id in stmts {
            self.check_stmt(id);
        }
    }

    fn check_stmt(&mut self, id: StmtId) {
        let ast = self.ast;
        match ast.stmt(id) {
            Stmt::Let { ty, name, value } => {
                let declared = Type::from_annotation(*ty);
                let binding_type = match value {
                    Some(value_id) => self.check_expr(*value_id),
                    None => declared,
                };
                self.declare(name.clone(), binding_type);
            }
            Stmt::Expr(expr) => {
                self.check_expr(*expr);
            }
            Stmt::Print(expr) => {
                self.check_expr(*expr);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_type = self.check_expr(*condition);
                self.expect_type(&condition_type, &Type::Bool, "if condition");
                self.scopes.push(HashMap::new());
                self.check_block(then_branch);
                self.scopes.pop();
                if let Some(branch) = else_branch {
                    self.scopes.push(HashMap::new());
                    self.check_block(branch);
                    self.scopes.pop();
                }
            }
            Stmt::While { condition, body } => {
                let condition_type = self.check_expr(*condition);
                self.expect_type(&condition_type, &Type::Bool, "while condition");
                self.scopes.push(HashMap::new());
                self.check_block(body);
                self.scopes.pop();
            }
            Stmt::Return(value) => {
                if !self.in_function {
                    self.diagnostics
                        .push(Diagnostic::error("return outside of a function"));
                }
                if let Some(value_id) = value {
                    self.check_expr(*value_id);
                }
            }
            Stmt::FunctionDecl {
                name: _,
                params,
                body,
            } => {
                self.scopes.push(HashMap::new());
                let was_in_function = self.in_function;
                self.in_function = true;
                for param in params {
                    let ty = param.ty.map(Type::from_annotation).unwrap_or(Type::Unknown);
                    self.declare(param.name.clone(), ty);
                }
                self.check_block(body);
                self.in_function = was_in_function;
                self.scopes.pop();
            }
            Stmt::Match { subject, arms } => {
                let subject_type = self.check_expr(*subject);
                for arm in arms {
                    if let Pattern::Literal(literal) = &arm.pattern {
                        let pattern_type = literal_type(literal);
                        self.expect_type(&pattern_type, &subject_type, "match pattern");
                    }
                    self.check_stmt(arm.body);
                }
            }
        }
    }

    fn check_expr(&mut self, id: ExprId) -> Type {
        let ast = self.ast;
        let ty = match ast.expr(id) {
            Expr::Literal(literal) => literal_type(literal),
            Expr::Ident(name) => self.lookup(name).unwrap_or_else(|| {
                self.diagnostics
                    .push(Diagnostic::error(format!("undefined variable '{name}'")));
                Type::Unknown
            }),
            Expr::Unary { op, operand } => {
                let operand_type = self.check_expr(*operand);
                match op {
                    UnOp::Negate => {
                        self.expect_type(&operand_type, &Type::Number, "unary '-'");
                        Type::Number
                    }
                    UnOp::Not => {
                        self.expect_type(&operand_type, &Type::Bool, "unary '!'");
                        Type::Bool
                    }
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let lhs_type = self.check_expr(*lhs);
                let rhs_type = self.check_expr(*rhs);
                self.check_binary(*op, &lhs_type, &rhs_type)
            }
            Expr::Assign { target, value } => {
                let value_type = self.check_expr(*value);
                let target_type = self.check_expr(*target);
                self.expect_type(&value_type, &target_type, "assignment");
                target_type
            }
            Expr::Call { callee, args } => {
                let callee_type = self.check_expr(*callee);
                let arg_types: Vec<Type> = args.iter().map(|arg| self.check_expr(*arg)).collect();
                match callee_type {
                    Type::Function { params, ret } => {
                        if params.len() != arg_types.len() {
                            self.diagnostics.push(Diagnostic::error(format!(
                                "expected {} argument(s), found {}",
                                params.len(),
                                arg_types.len()
                            )));
                        }
                        *ret
                    }
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.diagnostics.push(Diagnostic::error(format!(
                            "cannot call a value of type {other:?}"
                        )));
                        Type::Unknown
                    }
                }
            }
            Expr::Index { object, index } => {
                let object_type = self.check_expr(*object);
                let index_type = self.check_expr(*index);
                self.expect_type(&index_type, &Type::Number, "array index");
                match object_type {
                    Type::Array(element) => *element,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.diagnostics.push(Diagnostic::error(format!(
                            "cannot index a value of type {other:?}"
                        )));
                        Type::Unknown
                    }
                }
            }
            Expr::FieldAccess { object, field: _ } => {
                self.check_expr(*object);
                Type::Unknown
            }
            Expr::ArrayLiteral(elements) => {
                let mut element_type = Type::Unknown;
                for &element in elements {
                    let this_type = self.check_expr(element);
                    if element_type == Type::Unknown {
                        element_type = this_type;
                    }
                }
                Type::Array(Box::new(element_type))
            }
            Expr::StructLiteral(fields) => {
                for (_, value) in fields {
                    self.check_expr(*value);
                }
                Type::Struct
            }
            Expr::Lambda { params, body } => {
                self.scopes.push(HashMap::new());
                for param in params {
                    self.declare(param.clone(), Type::Unknown);
                }
                let body_type = self.check_expr(*body);
                self.scopes.pop();
                Type::Function {
                    params: vec![Type::Unknown; params.len()],
                    ret: Box::new(body_type),
                }
            }
        };
        self.expr_types[id.index()] = ty.clone();
        ty
    }

    fn check_binary(&mut self, op: BinOp, lhs: &Type, rhs: &Type) -> Type {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                self.expect_type(lhs, &Type::Number, "arithmetic");
                self.expect_type(rhs, &Type::Number, "arithmetic");
                Type::Number
            }
            BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                self.expect_type(lhs, &Type::Number, "comparison");
                self.expect_type(rhs, &Type::Number, "comparison");
                Type::Bool
            }
            BinOp::Eq | BinOp::NotEq => Type::Bool,
            BinOp::And | BinOp::Or => {
                self.expect_type(lhs, &Type::Bool, "logical operator");
                self.expect_type(rhs, &Type::Bool, "logical operator");
                Type::Bool
            }
        }
    }
}

fn literal_type(literal: &Literal) -> Type {
    match literal {
        Literal::Integer(_) => Type::Number,
        Literal::Str(_) => Type::Str,
        Literal::Bool(_) => Type::Bool,
        Literal::Nil => Type::Nil,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_source(source: &str) -> Result<Hir, Vec<Diagnostic>> {
        let (ast, parse_diagnostics) = druk_parser::parse(source);
        assert!(
            parse_diagnostics.is_empty(),
            "unexpected parse diagnostics: {parse_diagnostics:?}"
        );
        check(ast)
    }

    #[test]
    fn checks_every_bundled_example_without_diagnostics() {
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let entries = std::fs::read_dir(examples_dir).expect("examples dir exists");
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
                let source = std::fs::read_to_string(&path).expect("example file is readable");
                let result = check_source(&source);
                assert!(
                    result.is_ok(),
                    "unexpected diagnostics checking {path:?}: {result:?}"
                );
            }
        }
    }

    #[test]
    fn reports_an_undefined_variable() {
        let result = check_source("བཀོད་ missing;");
        let diagnostics = result.expect_err("expected a diagnostic");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("undefined variable")));
    }

    #[test]
    fn reports_a_type_mismatch_in_arithmetic() {
        let result = check_source("བདེན་རྫུན་ flag = བདེན་པ་; བཀོད་ flag + ༡;");
        let diagnostics = result.expect_err("expected a diagnostic");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("type mismatch")));
    }

    #[test]
    fn reports_return_outside_function() {
        let result = check_source("སླར་ལོག་ ༡;");
        let diagnostics = result.expect_err("expected a diagnostic");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("return outside")));
    }

    #[test]
    fn allows_recursive_function_calls() {
        let result = check_source(
            "ལས་འགན་ fib(གྲངས་ n) { གལ་སྲིད་ (n <= ༡) སླར་ལོག་ n; སླར་ལོག་ fib(n - ༡) + fib(n - ༢); } བཀོད་ fib(༥);",
        );
        assert!(result.is_ok(), "unexpected diagnostics: {result:?}");
    }

    #[test]
    fn reports_wrong_argument_count() {
        let result = check_source("ལས་འགན་ square(གྲངས་ n) { སླར་ལོག་ n * n; } བཀོད་ square(༡, ༢);");
        let diagnostics = result.expect_err("expected a diagnostic");
        assert!(diagnostics.iter().any(|d| d.message.contains("argument")));
    }
}
