use druk_ast::{Literal, Pattern};
use druk_diagnostics::Diagnostic;
use druk_lexer::TokenKind;

use crate::Parser;

impl<'src> Parser<'src> {
    pub(crate) fn parse_pattern(&mut self) -> Pattern {
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
}
