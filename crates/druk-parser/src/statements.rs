use druk_ast::{BaseType, Param, Stmt, StmtId, TypeAnnotation};
use druk_diagnostics::Diagnostic;
use druk_lexer::TokenKind;

use crate::Parser;

impl<'src> Parser<'src> {
    pub(crate) fn parse_statement(&mut self) -> StmtId {
        match self.peek() {
            Some(TokenKind::KwNumber | TokenKind::KwString | TokenKind::KwBool) => {
                self.parse_let_stmt()
            }
            Some(TokenKind::KwPrint) => self.parse_print_stmt(),
            Some(TokenKind::KwIf) => self.parse_if_stmt(),
            Some(TokenKind::KwWhile) => self.parse_while_stmt(),
            Some(TokenKind::KwFor) => self.parse_for_stmt(),
            Some(TokenKind::KwBreak) => self.parse_break_stmt(),
            Some(TokenKind::KwContinue) => self.parse_continue_stmt(),
            Some(TokenKind::KwReturn) => self.parse_return_stmt(),
            Some(TokenKind::KwFunction) => self.parse_function_decl(),
            Some(TokenKind::KwMatch) => self.parse_match_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    pub(crate) fn parse_block(&mut self) -> Vec<StmtId> {
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

    pub(crate) fn parse_type_annotation(&mut self) -> TypeAnnotation {
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

    /// Parses the one documented three-clause form, every clause required:
    /// `init` is a `let`-shaped declaration (its own trailing `;` doubles as
    /// the separator before `condition`), `condition` is followed by its own
    /// explicit `;`, and `post` is a bare expression with no trailing `;` of
    /// its own, since `)` ends the header. Omitting a clause, as C allows,
    /// is not supported: no documented syntax or bundled example needs it.
    fn parse_for_stmt(&mut self) -> StmtId {
        self.advance();
        self.expect(TokenKind::LParen, "'('");
        let init = self.parse_let_stmt();
        let condition = self.parse_expression();
        self.expect(TokenKind::Semi, "';'");
        let post = self.parse_expression();
        self.expect(TokenKind::RParen, "')'");
        let body = self.parse_block();
        self.ast.alloc_stmt(Stmt::For {
            init,
            condition,
            post,
            body,
        })
    }

    fn parse_break_stmt(&mut self) -> StmtId {
        self.advance();
        self.expect(TokenKind::Semi, "';'");
        self.ast.alloc_stmt(Stmt::Break)
    }

    fn parse_continue_stmt(&mut self) -> StmtId {
        self.advance();
        self.expect(TokenKind::Semi, "';'");
        self.ast.alloc_stmt(Stmt::Continue)
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

    fn parse_match_stmt(&mut self) -> StmtId {
        self.advance();
        let subject = self.parse_expression();
        self.expect(TokenKind::LBrace, "'{'");
        let mut arms = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.at_end() {
            let pattern = self.parse_pattern();
            self.expect(TokenKind::Arrow, "'->'");
            let body = self.parse_statement();
            arms.push(druk_ast::MatchArm { pattern, body });
        }
        self.expect(TokenKind::RBrace, "'}'");
        self.ast.alloc_stmt(Stmt::Match { subject, arms })
    }

    fn parse_expr_stmt(&mut self) -> StmtId {
        let expr = self.parse_expression();
        self.expect(TokenKind::Semi, "';'");
        self.ast.alloc_stmt(Stmt::Expr(expr))
    }
}
