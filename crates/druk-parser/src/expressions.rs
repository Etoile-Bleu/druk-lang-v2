use druk_ast::{BinOp, Expr, ExprId, Literal, UnOp};
use druk_diagnostics::Diagnostic;
use druk_lexer::TokenKind;

use crate::Parser;

impl<'src> Parser<'src> {
    pub(crate) fn parse_expression(&mut self) -> ExprId {
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
    /// failure. Used to disambiguate a lambda's parameter list from a
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
                    Diagnostic::error("expected a value or a calculation here")
                        .with_label(span, "this is what's here instead"),
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
