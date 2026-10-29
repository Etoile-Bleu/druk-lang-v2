//! Hand-written recursive-descent parser with Pratt-style precedence climbing
//! for expressions. Parse errors are collected as diagnostics rather than
//! aborting the pass — a single `parse` call reports every statement it
//! could not make sense of, recovering at the next statement boundary.

use druk_ast::{
    Ast, BaseType, BinOp, Expr, ExprId, Literal, MatchArm, Param, Pattern, Stmt, StmtId,
    TypeAnnotation, UnOp,
};
use druk_diagnostics::Diagnostic;
use druk_lexer::{tokenize, SpannedToken, TokenKind};

pub fn parse(source: &str) -> (Ast, Vec<Diagnostic>) {
    let (tokens, mut diagnostics) = tokenize(source);
    let mut parser = Parser {
        tokens,
        pos: 0,
        ast: Ast::new(),
        diagnostics: Vec::new(),
    };
    parser.run();
    diagnostics.append(&mut parser.diagnostics);
    (parser.ast, diagnostics)
}

struct Parser<'src> {
    tokens: Vec<SpannedToken<'src>>,
    pos: usize,
    ast: Ast,
    diagnostics: Vec<Diagnostic>,
}

impl<'src> Parser<'src> {
    fn run(&mut self) {
        while !self.at_end() {
            let before = self.pos;
            let stmt = self.parse_statement();
            self.ast.top_level.push(stmt);
            if self.pos == before {
                self.synchronize();
            }
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos).map(|t| t.kind)
    }

    fn peek_at(&self, offset: usize) -> Option<TokenKind> {
        self.tokens.get(self.pos + offset).map(|t| t.kind)
    }

    fn current_span(&self) -> druk_diagnostics::Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span.clone())
            .unwrap_or_else(|| {
                let end = self.tokens.last().map(|t| t.span.end).unwrap_or(0);
                end..end
            })
    }

    fn advance(&mut self) -> Option<SpannedToken<'src>> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    /// Advances past the current token and returns its text, or `""` if
    /// there was no token to advance past. Used where a caller already
    /// matched on `peek()`/`check()` and only needs the consumed text.
    fn bump(&mut self) -> &'src str {
        match self.tokens.get(self.pos) {
            Some(token) => {
                let text = token.text;
                self.pos += 1;
                text
            }
            None => "",
        }
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek() == Some(kind)
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> bool {
        if self.eat(kind) {
            true
        } else {
            let span = self.current_span();
            self.diagnostics.push(
                Diagnostic::error(format!("expected {what}"))
                    .with_label(span, "found here instead"),
            );
            false
        }
    }

    fn expect_ident(&mut self) -> String {
        if self.check(TokenKind::Ident) {
            self.bump().to_string()
        } else {
            let span = self.current_span();
            self.diagnostics.push(
                Diagnostic::error("expected an identifier").with_label(span, "found here instead"),
            );
            String::new()
        }
    }

    /// Skips tokens until a likely statement boundary, so one malformed
    /// statement doesn't cascade into spurious errors for everything after it.
    fn synchronize(&mut self) {
        while let Some(kind) = self.peek() {
            if matches!(kind, TokenKind::Semi | TokenKind::RBrace) {
                self.advance();
                return;
            }
            if matches!(
                kind,
                TokenKind::KwFunction
                    | TokenKind::KwNumber
                    | TokenKind::KwString
                    | TokenKind::KwBool
                    | TokenKind::KwIf
                    | TokenKind::KwWhile
                    | TokenKind::KwReturn
                    | TokenKind::KwPrint
                    | TokenKind::KwMatch
            ) {
                return;
            }
            self.advance();
        }
    }

    // ---- statements ----

    fn parse_statement(&mut self) -> StmtId {
        match self.peek() {
            Some(TokenKind::KwNumber | TokenKind::KwString | TokenKind::KwBool) => {
                self.parse_let_stmt()
            }
            Some(TokenKind::KwPrint) => self.parse_print_stmt(),
            Some(TokenKind::KwIf) => self.parse_if_stmt(),
            Some(TokenKind::KwWhile) => self.parse_while_stmt(),
            Some(TokenKind::KwReturn) => self.parse_return_stmt(),
            Some(TokenKind::KwFunction) => self.parse_function_decl(),
            Some(TokenKind::KwMatch) => self.parse_match_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_block(&mut self) -> Vec<StmtId> {
        self.expect(TokenKind::LBrace, "'{'");
        let mut stmts = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.at_end() {
            let before = self.pos;
            stmts.push(self.parse_statement());
            if self.pos == before {
                self.synchronize();
            }
        }
        self.expect(TokenKind::RBrace, "'}'");
        stmts
    }

    fn parse_stmt_or_block(&mut self) -> Vec<StmtId> {
        if self.check(TokenKind::LBrace) {
            self.parse_block()
        } else {
            vec![self.parse_statement()]
        }
    }

    fn parse_type_annotation(&mut self) -> TypeAnnotation {
        let base = match self.peek() {
            Some(TokenKind::KwNumber) => BaseType::Number,
            Some(TokenKind::KwString) => BaseType::Str,
            Some(TokenKind::KwBool) => BaseType::Bool,
            _ => {
                let span = self.current_span();
                self.diagnostics.push(
                    Diagnostic::error("expected a type")
                        .with_label(span, "expected one of the type keywords here"),
                );
                BaseType::Number
            }
        };
        self.advance();
        let is_array = if self.eat(TokenKind::LBracket) {
            self.expect(TokenKind::RBracket, "']'");
            true
        } else {
            false
        };
        TypeAnnotation { base, is_array }
    }

    fn parse_let_stmt(&mut self) -> StmtId {
        let ty = self.parse_type_annotation();
        let name = self.expect_ident();
        let value = if self.eat(TokenKind::Eq) {
            Some(self.parse_expression())
        } else {
            None
        };
        self.expect(TokenKind::Semi, "';'");
        self.ast.alloc_stmt(Stmt::Let { ty, name, value })
    }

    fn parse_print_stmt(&mut self) -> StmtId {
        self.advance();
        let value = self.parse_expression();
        self.expect(TokenKind::Semi, "';'");
        self.ast.alloc_stmt(Stmt::Print(value))
    }

    fn parse_if_stmt(&mut self) -> StmtId {
        self.advance();
        self.expect(TokenKind::LParen, "'('");
        let condition = self.parse_expression();
        self.expect(TokenKind::RParen, "')'");
        let then_branch = self.parse_stmt_or_block();
        let else_branch = if self.eat(TokenKind::KwElse) {
            if self.check(TokenKind::KwIf) {
                Some(vec![self.parse_if_stmt()])
            } else {
                Some(self.parse_stmt_or_block())
            }
        } else {
            None
        };
        self.ast.alloc_stmt(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while_stmt(&mut self) -> StmtId {
        self.advance();
        self.expect(TokenKind::LParen, "'('");
        let condition = self.parse_expression();
        self.expect(TokenKind::RParen, "')'");
        let body = self.parse_stmt_or_block();
        self.ast.alloc_stmt(Stmt::While { condition, body })
    }

    fn parse_return_stmt(&mut self) -> StmtId {
        self.advance();
        let value = if self.check(TokenKind::Semi) {
            None
        } else {
            Some(self.parse_expression())
        };
        self.expect(TokenKind::Semi, "';'");
        self.ast.alloc_stmt(Stmt::Return(value))
    }

    fn parse_param_list(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        if self.check(TokenKind::RParen) {
            return params;
        }
        loop {
            let ty = match self.peek() {
                Some(TokenKind::KwNumber | TokenKind::KwString | TokenKind::KwBool) => {
                    Some(self.parse_type_annotation())
                }
                _ => None,
            };
            let name = self.expect_ident();
            params.push(Param { name, ty });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        params
    }

    fn parse_function_decl(&mut self) -> StmtId {
        self.advance();
        let name = self.expect_ident();
        self.expect(TokenKind::LParen, "'('");
        let params = self.parse_param_list();
        self.expect(TokenKind::RParen, "')'");
        let body = self.parse_block();
        self.ast
            .alloc_stmt(Stmt::FunctionDecl { name, params, body })
    }

    fn parse_pattern(&mut self) -> Pattern {
        match self.peek() {
            Some(TokenKind::Integer) => {
                let text = self.bump();
                let value = druk_lexer::tibetan_digits_to_i64(text).unwrap_or(0);
                Pattern::Literal(Literal::Integer(value))
            }
            Some(TokenKind::Ident) if self.tokens[self.pos].text == "_" => {
                self.advance();
                Pattern::Wildcard
            }
            _ => {
                let span = self.current_span();
                self.diagnostics.push(
                    Diagnostic::error("expected a pattern")
                        .with_label(span, "expected a literal or '_' here"),
                );
                self.advance();
                Pattern::Wildcard
            }
        }
    }

    fn parse_match_stmt(&mut self) -> StmtId {
        self.advance();
        let subject = self.parse_expression();
        self.expect(TokenKind::LBrace, "'{'");
        let mut arms = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.at_end() {
            let pattern = self.parse_pattern();
            self.expect(TokenKind::Arrow, "'->'");
            let body = self.parse_statement();
            arms.push(MatchArm { pattern, body });
        }
        self.expect(TokenKind::RBrace, "'}'");
        self.ast.alloc_stmt(Stmt::Match { subject, arms })
    }

    fn parse_expr_stmt(&mut self) -> StmtId {
        let expr = self.parse_expression();
        self.expect(TokenKind::Semi, "';'");
        self.ast.alloc_stmt(Stmt::Expr(expr))
    }

    // ---- expressions ----

    fn parse_expression(&mut self) -> ExprId {
        if let Some(lambda) = self.try_parse_lambda() {
            return lambda;
        }
        self.parse_assignment()
    }

    fn try_parse_lambda(&mut self) -> Option<ExprId> {
        match self.peek() {
            Some(TokenKind::Ident) if self.peek_at(1) == Some(TokenKind::Arrow) => {
                let name = self.bump().to_string();
                self.advance(); // '->'
                let body = self.parse_expression();
                Some(self.ast.alloc_expr(Expr::Lambda {
                    params: vec![name],
                    body,
                }))
            }
            Some(TokenKind::LParen) => {
                let saved = self.pos;
                if let Some(params) = self.try_parse_ident_param_list() {
                    if self.check(TokenKind::Arrow) {
                        self.advance();
                        let body = self.parse_expression();
                        return Some(self.ast.alloc_expr(Expr::Lambda { params, body }));
                    }
                }
                self.pos = saved;
                None
            }
            _ => None,
        }
    }

    /// Tentatively parses `(ident, ident, ...)` without consuming input on
    /// failure — used to disambiguate a lambda's parameter list from a
    /// parenthesized grouping expression, which share the same opening token.
    fn try_parse_ident_param_list(&mut self) -> Option<Vec<String>> {
        let saved = self.pos;
        if !self.eat(TokenKind::LParen) {
            return None;
        }
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                match self.peek() {
                    Some(TokenKind::Ident) => {
                        params.push(self.bump().to_string());
                    }
                    _ => {
                        self.pos = saved;
                        return None;
                    }
                }
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        if !self.eat(TokenKind::RParen) {
            self.pos = saved;
            return None;
        }
        Some(params)
    }

    fn parse_assignment(&mut self) -> ExprId {
        let target = self.parse_or();
        if self.eat(TokenKind::Eq) {
            let value = self.parse_assignment();
            return self.ast.alloc_expr(Expr::Assign { target, value });
        }
        target
    }

    fn parse_or(&mut self) -> ExprId {
        let mut lhs = self.parse_and();
        while self.eat(TokenKind::OrOr) {
            let rhs = self.parse_and();
            lhs = self.ast.alloc_expr(Expr::Binary {
                op: BinOp::Or,
                lhs,
                rhs,
            });
        }
        lhs
    }

    fn parse_and(&mut self) -> ExprId {
        let mut lhs = self.parse_equality();
        while self.eat(TokenKind::AndAnd) {
            let rhs = self.parse_equality();
            lhs = self.ast.alloc_expr(Expr::Binary {
                op: BinOp::And,
                lhs,
                rhs,
            });
        }
        lhs
    }

    fn parse_equality(&mut self) -> ExprId {
        let mut lhs = self.parse_comparison();
        loop {
            let op = match self.peek() {
                Some(TokenKind::EqEq) => BinOp::Eq,
                Some(TokenKind::NotEq) => BinOp::NotEq,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_comparison();
            lhs = self.ast.alloc_expr(Expr::Binary { op, lhs, rhs });
        }
        lhs
    }

    fn parse_comparison(&mut self) -> ExprId {
        let mut lhs = self.parse_additive();
        loop {
            let op = match self.peek() {
                Some(TokenKind::Lt) => BinOp::Lt,
                Some(TokenKind::LtEq) => BinOp::LtEq,
                Some(TokenKind::Gt) => BinOp::Gt,
                Some(TokenKind::GtEq) => BinOp::GtEq,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_additive();
            lhs = self.ast.alloc_expr(Expr::Binary { op, lhs, rhs });
        }
        lhs
    }

    fn parse_additive(&mut self) -> ExprId {
        let mut lhs = self.parse_multiplicative();
        loop {
            let op = match self.peek() {
                Some(TokenKind::Plus) => BinOp::Add,
                Some(TokenKind::Minus) => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_multiplicative();
            lhs = self.ast.alloc_expr(Expr::Binary { op, lhs, rhs });
        }
        lhs
    }

    fn parse_multiplicative(&mut self) -> ExprId {
        let mut lhs = self.parse_unary();
        loop {
            let op = match self.peek() {
                Some(TokenKind::Star) => BinOp::Mul,
                Some(TokenKind::Slash) => BinOp::Div,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary();
            lhs = self.ast.alloc_expr(Expr::Binary { op, lhs, rhs });
        }
        lhs
    }

    fn parse_unary(&mut self) -> ExprId {
        let op = match self.peek() {
            Some(TokenKind::Bang) => Some(UnOp::Not),
            Some(TokenKind::Minus) => Some(UnOp::Negate),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let operand = self.parse_unary();
            return self.ast.alloc_expr(Expr::Unary { op, operand });
        }
        self.parse_call()
    }

    fn parse_call(&mut self) -> ExprId {
        let mut expr = self.parse_primary();
        loop {
            match self.peek() {
                Some(TokenKind::LParen) => {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expression());
                            if !self.eat(TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen, "')'");
                    expr = self.ast.alloc_expr(Expr::Call { callee: expr, args });
                }
                Some(TokenKind::LBracket) => {
                    self.advance();
                    let index = self.parse_expression();
                    self.expect(TokenKind::RBracket, "']'");
                    expr = self.ast.alloc_expr(Expr::Index {
                        object: expr,
                        index,
                    });
                }
                Some(TokenKind::Dot) => {
                    self.advance();
                    let field = self.expect_ident();
                    expr = self.ast.alloc_expr(Expr::FieldAccess {
                        object: expr,
                        field,
                    });
                }
                _ => break,
            }
        }
        expr
    }

    fn parse_primary(&mut self) -> ExprId {
        match self.peek() {
            Some(TokenKind::Integer) => {
                let text = self.bump();
                let value = druk_lexer::tibetan_digits_to_i64(text).unwrap_or(0);
                self.ast.alloc_expr(Expr::Literal(Literal::Integer(value)))
            }
            Some(TokenKind::StringLiteral) => {
                let text = self.bump();
                let value = unescape_string_literal(text);
                self.ast.alloc_expr(Expr::Literal(Literal::Str(value)))
            }
            Some(TokenKind::KwTrue) => {
                self.advance();
                self.ast.alloc_expr(Expr::Literal(Literal::Bool(true)))
            }
            Some(TokenKind::KwFalse) => {
                self.advance();
                self.ast.alloc_expr(Expr::Literal(Literal::Bool(false)))
            }
            Some(TokenKind::KwNil) => {
                self.advance();
                self.ast.alloc_expr(Expr::Literal(Literal::Nil))
            }
            Some(TokenKind::Ident) => {
                let text = self.bump().to_string();
                self.ast.alloc_expr(Expr::Ident(text))
            }
            Some(kind) if builtin_name(kind).is_some() => {
                self.advance();
                let name = builtin_name(kind).unwrap_or("builtin").to_string();
                self.ast.alloc_expr(Expr::Ident(name))
            }
            Some(TokenKind::LParen) => {
                self.advance();
                let inner = self.parse_expression();
                self.expect(TokenKind::RParen, "')'");
                inner
            }
            Some(TokenKind::LBracket) => {
                self.advance();
                let mut elements = Vec::new();
                if !self.check(TokenKind::RBracket) {
                    loop {
                        elements.push(self.parse_expression());
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RBracket, "']'");
                self.ast.alloc_expr(Expr::ArrayLiteral(elements))
            }
            Some(TokenKind::LBrace) => {
                self.advance();
                let mut fields = Vec::new();
                if !self.check(TokenKind::RBrace) {
                    loop {
                        let name = self.expect_ident();
                        self.expect(TokenKind::Colon, "':'");
                        let value = self.parse_expression();
                        fields.push((name, value));
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RBrace, "'}'");
                self.ast.alloc_expr(Expr::StructLiteral(fields))
            }
            _ => {
                let span = self.current_span();
                self.diagnostics.push(
                    Diagnostic::error("expected an expression")
                        .with_label(span, "found here instead"),
                );
                if !self.at_end() {
                    self.advance();
                }
                self.ast.alloc_expr(Expr::Literal(Literal::Nil))
            }
        }
    }
}

fn builtin_name(kind: TokenKind) -> Option<&'static str> {
    Some(match kind {
        TokenKind::BuiltinLen => "len",
        TokenKind::BuiltinPush => "push",
        TokenKind::BuiltinPop => "pop",
        TokenKind::BuiltinTypeof => "typeof",
        TokenKind::BuiltinKeys => "keys",
        TokenKind::BuiltinValues => "values",
        TokenKind::BuiltinContains => "contains",
        TokenKind::BuiltinInput => "input",
        TokenKind::GlobalArgv => "argv",
        TokenKind::GlobalArgc => "argc",
        _ => return None,
    })
}

fn unescape_string_literal(text: &str) -> String {
    let inner = &text[1..text.len().saturating_sub(1)];
    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some(other) => result.push(other),
                None => {}
            }
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(source: &str) -> Ast {
        let (ast, diagnostics) = parse(source);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        ast
    }

    #[test]
    fn parses_every_bundled_example_without_diagnostics() {
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let entries = std::fs::read_dir(examples_dir).expect("examples dir exists");
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
                let source = std::fs::read_to_string(&path).expect("example file is readable");
                let (_, diagnostics) = parse(&source);
                assert!(
                    diagnostics.is_empty(),
                    "unexpected diagnostics parsing {path:?}: {diagnostics:?}"
                );
            }
        }
    }

    #[test]
    fn parses_hello_world() {
        let ast = parse_ok(r#"བཀོད་ "Hello, Druk Lang!";"#);
        assert_eq!(ast.top_level.len(), 1);
        match ast.stmt(ast.top_level[0]) {
            Stmt::Print(expr) => match ast.expr(*expr) {
                Expr::Literal(Literal::Str(s)) => assert_eq!(s, "Hello, Druk Lang!"),
                other => panic!("expected string literal, got {other:?}"),
            },
            other => panic!("expected Print statement, got {other:?}"),
        }
    }

    #[test]
    fn parses_let_with_array_type() {
        let ast = parse_ok("གྲངས་[] numbers = [༡, ༢, ༣];");
        match ast.stmt(ast.top_level[0]) {
            Stmt::Let { ty, name, value } => {
                assert!(ty.is_array);
                assert_eq!(ty.base, BaseType::Number);
                assert_eq!(name, "numbers");
                match value.map(|id| ast.expr(id)) {
                    Some(Expr::ArrayLiteral(elements)) => assert_eq!(elements.len(), 3),
                    other => panic!("expected array literal, got {other:?}"),
                }
            }
            other => panic!("expected Let statement, got {other:?}"),
        }
    }

    #[test]
    fn parses_two_arg_lambda_with_parens() {
        let ast = parse_ok("གྲངས་ multiply = (a, b) -> a * b;");
        match ast.stmt(ast.top_level[0]) {
            Stmt::Let {
                value: Some(expr), ..
            } => match ast.expr(*expr) {
                Expr::Lambda { params, .. } => {
                    assert_eq!(params, &vec!["a".to_string(), "b".to_string()])
                }
                other => panic!("expected Lambda, got {other:?}"),
            },
            other => panic!("expected Let statement, got {other:?}"),
        }
    }

    #[test]
    fn parses_single_param_lambda_without_parens() {
        let ast = parse_ok("གྲངས་ double = n -> n * ༢;");
        match ast.stmt(ast.top_level[0]) {
            Stmt::Let {
                value: Some(expr), ..
            } => match ast.expr(*expr) {
                Expr::Lambda { params, .. } => assert_eq!(params, &vec!["n".to_string()]),
                other => panic!("expected Lambda, got {other:?}"),
            },
            other => panic!("expected Let statement, got {other:?}"),
        }
    }

    #[test]
    fn parses_lambda_passed_as_a_call_argument() {
        let ast = parse_ok("གྲངས་ res = run_op(༡༠, ༢, (x, y) -> x + y);");
        match ast.stmt(ast.top_level[0]) {
            Stmt::Let {
                value: Some(expr), ..
            } => match ast.expr(*expr) {
                Expr::Call { args, .. } => {
                    assert_eq!(args.len(), 3);
                    match ast.expr(args[2]) {
                        Expr::Lambda { params, .. } => {
                            assert_eq!(params, &vec!["x".to_string(), "y".to_string()])
                        }
                        other => panic!("expected Lambda argument, got {other:?}"),
                    }
                }
                other => panic!("expected Call, got {other:?}"),
            },
            other => panic!("expected Let statement, got {other:?}"),
        }
    }

    #[test]
    fn parses_if_without_braces_for_single_statement_branch() {
        let ast = parse_ok("ལས་འགན་ fib(གྲངས་ n) { གལ་སྲིད་ (n <= ༡) སླར་ལོག་ n; སླར་ལོག་ n; }");
        match ast.stmt(ast.top_level[0]) {
            Stmt::FunctionDecl { body, .. } => {
                assert_eq!(body.len(), 2);
                match ast.stmt(body[0]) {
                    Stmt::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        assert_eq!(then_branch.len(), 1);
                        assert!(else_branch.is_none());
                    }
                    other => panic!("expected If statement, got {other:?}"),
                }
            }
            other => panic!("expected FunctionDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_else_if_chain() {
        let ast = parse_ok(
            "གྲངས་ score = ༨༥; གལ་སྲིད་ (score >= ༩༠) { བཀོད་ \"A\"; } མེད་ན་ གལ་སྲིད་ (score >= ༨༠) { བཀོད་ \"B\"; } མེད་ན་ { བཀོད་ \"C\"; }",
        );
        match ast.stmt(ast.top_level[1]) {
            Stmt::If {
                else_branch: Some(branch),
                ..
            } => {
                assert_eq!(branch.len(), 1);
                assert!(matches!(ast.stmt(branch[0]), Stmt::If { .. }));
            }
            other => panic!("expected If statement with else branch, got {other:?}"),
        }
    }

    #[test]
    fn parses_match_with_wildcard() {
        let ast = parse_ok(
            "གྲངས་ code = ༢; འགྲིག་པ་ code { ༡ -> བཀོད་ \"Success\"; _ -> བཀོད་ \"Unknown\"; }",
        );
        match ast.stmt(ast.top_level[1]) {
            Stmt::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert!(matches!(
                    arms[0].pattern,
                    Pattern::Literal(Literal::Integer(1))
                ));
                assert!(matches!(arms[1].pattern, Pattern::Wildcard));
            }
            other => panic!("expected Match statement, got {other:?}"),
        }
    }

    #[test]
    fn recovers_after_a_malformed_statement_and_keeps_parsing() {
        let (ast, diagnostics) = parse("@ ; བཀོད་ \"still parsed\";");
        assert!(!diagnostics.is_empty());
        let has_print = ast
            .top_level
            .iter()
            .any(|id| matches!(ast.stmt(*id), Stmt::Print(_)));
        assert!(
            has_print,
            "parser should recover and still see the print statement"
        );
    }
}
