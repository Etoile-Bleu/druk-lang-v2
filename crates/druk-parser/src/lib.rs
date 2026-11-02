//! Hand-written recursive-descent parser with Pratt-style precedence climbing
//! for expressions. Parse errors are collected as diagnostics rather than
//! aborting the pass: a single `parse` call reports every statement it
//! could not make sense of, recovering at the next statement boundary.

mod expressions;
mod patterns;
mod statements;

use druk_ast::Ast;
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
                    | TokenKind::KwBreak
                    | TokenKind::KwContinue
                    | TokenKind::KwReturn
                    | TokenKind::KwPrint
                    | TokenKind::KwMatch
            ) {
                return;
            }
            self.advance();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use druk_ast::{BaseType, Expr, Literal, Pattern, Stmt};

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
    fn parses_break_and_continue_inside_a_while_body() {
        let ast = parse_ok("ཡང་བསྐྱར་ (བདེན་པ་) { ཆད་; མུ་མཐུད་; }");
        match ast.stmt(ast.top_level[0]) {
            Stmt::While { body, .. } => {
                assert_eq!(body.len(), 2);
                assert!(matches!(ast.stmt(body[0]), Stmt::Break));
                assert!(matches!(ast.stmt(body[1]), Stmt::Continue));
            }
            other => panic!("expected While statement, got {other:?}"),
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
