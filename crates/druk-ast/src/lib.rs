//! Arena-backed syntax tree: nodes live in flat `Vec`s owned by `Ast` and
//! reference each other by small `Copy` IDs instead of `Box<Node>`, so the
//! tree has no recursive ownership or lifetime shape at all.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(u32);

impl ExprId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StmtId(u32);

impl StmtId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Str(String),
    Bool(bool),
    Nil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseType {
    Number,
    Str,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeAnnotation {
    pub base: BaseType,
    pub is_array: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Literal(Literal),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Ident(String),
    Unary { op: UnOp, operand: ExprId },
    Binary { op: BinOp, lhs: ExprId, rhs: ExprId },
    Assign { target: ExprId, value: ExprId },
    Call { callee: ExprId, args: Vec<ExprId> },
    Index { object: ExprId, index: ExprId },
    FieldAccess { object: ExprId, field: String },
    ArrayLiteral(Vec<ExprId>),
    StructLiteral(Vec<(String, ExprId)>),
    Lambda { params: Vec<String>, body: ExprId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeAnnotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: StmtId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        ty: TypeAnnotation,
        name: String,
        value: Option<ExprId>,
    },
    Expr(ExprId),
    Print(ExprId),
    If {
        condition: ExprId,
        then_branch: Vec<StmtId>,
        else_branch: Option<Vec<StmtId>>,
    },
    While {
        condition: ExprId,
        body: Vec<StmtId>,
    },
    For {
        init: StmtId,
        condition: ExprId,
        post: ExprId,
        body: Vec<StmtId>,
    },
    Break,
    Continue,
    Return(Option<ExprId>),
    FunctionDecl {
        name: String,
        params: Vec<Param>,
        body: Vec<StmtId>,
    },
    Match {
        subject: ExprId,
        arms: Vec<MatchArm>,
    },
}

/// A parsed program: its top-level statements, plus the arena backing every
/// node reachable from them.
#[derive(Debug, Default)]
pub struct Ast {
    exprs: Vec<Expr>,
    stmts: Vec<Stmt>,
    pub top_level: Vec<StmtId>,
}

impl Ast {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    pub fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        let id = StmtId(self.stmts.len() as u32);
        self.stmts.push(stmt);
        id
    }

    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0 as usize]
    }

    pub fn stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id.0 as usize]
    }

    pub fn expr_count(&self) -> usize {
        self.exprs.len()
    }

    pub fn stmt_count(&self) -> usize {
        self.stmts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_small_tree_by_id_without_recursive_ownership() {
        let mut ast = Ast::new();
        let one = ast.alloc_expr(Expr::Literal(Literal::Integer(1)));
        let two = ast.alloc_expr(Expr::Literal(Literal::Integer(2)));
        let sum = ast.alloc_expr(Expr::Binary {
            op: BinOp::Add,
            lhs: one,
            rhs: two,
        });
        let x = ast.alloc_expr(Expr::Ident("x".to_string()));
        let assign = ast.alloc_expr(Expr::Assign {
            target: x,
            value: sum,
        });
        let stmt = ast.alloc_stmt(Stmt::Expr(assign));
        ast.top_level.push(stmt);

        match ast.stmt(stmt) {
            Stmt::Expr(expr_id) => match ast.expr(*expr_id) {
                Expr::Assign { target, value } => {
                    assert_eq!(ast.expr(*target), &Expr::Ident("x".to_string()));
                    match ast.expr(*value) {
                        Expr::Binary { op, lhs, rhs } => {
                            assert_eq!(*op, BinOp::Add);
                            assert_eq!(ast.expr(*lhs), &Expr::Literal(Literal::Integer(1)));
                            assert_eq!(ast.expr(*rhs), &Expr::Literal(Literal::Integer(2)));
                        }
                        other => panic!("expected Binary, got {other:?}"),
                    }
                }
                other => panic!("expected Assign, got {other:?}"),
            },
            other => panic!("expected Expr statement, got {other:?}"),
        }
    }
}
