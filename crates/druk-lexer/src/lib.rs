use druk_diagnostics::{Diagnostic, Span};
use logos::Logos;

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n]+")]
pub enum TokenKind {
    #[token("ལས་འགན་")]
    KwFunction,
    #[token("གྲངས་")]
    KwNumber,
    #[token("ཡིག་འབྲུ་")]
    KwString,
    #[token("བདེན་རྫུན་")]
    KwBool,
    #[token("བདེན་པ་")]
    KwTrue,
    #[token("རྫུན་མ་")]
    KwFalse,
    #[token("ཅི་མེད")]
    KwNil,
    #[token("གལ་སྲིད་")]
    KwIf,
    #[token("མེད་ན་")]
    KwElse,
    #[token("ཡང་བསྐྱར་")]
    KwWhile,
    #[token("རེ་རེར་")]
    KwFor,
    #[token("ཆད་")]
    KwBreak,
    #[token("མུ་མཐུད་")]
    KwContinue,
    #[token("སླར་ལོག་")]
    KwReturn,
    #[token("འགྲིག་པ་")]
    KwMatch,
    #[token("བཀོད་")]
    KwPrint,

    #[token("རིང་ཚད")]
    BuiltinLen,
    #[token("སྣོན་")]
    BuiltinPush,
    #[token("བཏོན་")]
    BuiltinPop,
    #[token("རིགས་")]
    BuiltinTypeof,
    #[token("ལྡེ་མིག་")]
    BuiltinKeys,
    #[token("གནས་གོང་")]
    BuiltinValues,
    #[token("ནང་འདུས་")]
    BuiltinContains,
    #[token("ནང་འཇུག་ཐོ་")]
    GlobalArgv,
    #[token("ནང་འཇུག་གྲངས་")]
    GlobalArgc,
    #[token("ནང་འཇུག་")]
    BuiltinInput,

    // ASCII letters/digits/underscore, or Tibetan consonants (ཀ-ཬ), vowel
    // signs (ཱ-྄), subjoined consonants (ྐ-ྼ), and the tsheg syllable
    // separator (་) once past the first character, so a real multi-syllable
    // Dzongkha compound identifier like སློབ་ཕྲུག་གི་སྐར་གྲངས་ lexes as one
    // token instead of failing one codepoint at a time.
    #[regex("[a-zA-Z_ཀ-ཬཱ-྄ྐ-ྼ][a-zA-Z0-9_ཀ-ཬཱ-྄ྐ-ྼ་]*")]
    Ident,
    #[regex("[༠-༩]+")]
    Integer,
    #[regex(r#""([^"\\]|\\.)*""#)]
    StringLiteral,

    #[token("->")]
    Arrow,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("!")]
    Bang,
    #[token("=")]
    Eq,
    #[token(".")]
    Dot,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedToken<'src> {
    pub kind: TokenKind,
    pub span: Span,
    pub text: &'src str,
}

/// Tokenizes `source` fully, collecting every diagnostic for unrecognized
/// input instead of stopping at the first one.
pub fn tokenize(source: &str) -> (Vec<SpannedToken<'_>>, Vec<Diagnostic>) {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut lexer = TokenKind::lexer(source);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        match result {
            Ok(kind) => tokens.push(SpannedToken {
                kind,
                span: span.clone(),
                text: &source[span],
            }),
            Err(()) => diagnostics.push(
                Diagnostic::error("unrecognized character")
                    .with_label(span, "not valid Druk source"),
            ),
        }
    }

    (tokens, diagnostics)
}

/// Converts a run of Tibetan digits (U+0F20-U+0F29) into its integer value.
/// Returns `None` if `text` contains any character outside that digit block,
/// which should never happen for text already classified as `TokenKind::Integer`.
pub fn tibetan_digits_to_i64(text: &str) -> Option<i64> {
    let mut value: i64 = 0;
    for ch in text.chars() {
        let digit = (ch as u32).checked_sub(0x0F20)?;
        if digit > 9 {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(digit as i64)?;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        let (tokens, diagnostics) = tokenize(source);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        tokens.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn lexes_every_declared_keyword_individually() {
        let keywords: &[(&str, TokenKind)] = &[
            ("ལས་འགན་", TokenKind::KwFunction),
            ("གྲངས་", TokenKind::KwNumber),
            ("ཡིག་འབྲུ་", TokenKind::KwString),
            ("བདེན་རྫུན་", TokenKind::KwBool),
            ("བདེན་པ་", TokenKind::KwTrue),
            ("རྫུན་མ་", TokenKind::KwFalse),
            ("ཅི་མེད", TokenKind::KwNil),
            ("གལ་སྲིད་", TokenKind::KwIf),
            ("མེད་ན་", TokenKind::KwElse),
            ("ཡང་བསྐྱར་", TokenKind::KwWhile),
            ("རེ་རེར་", TokenKind::KwFor),
            ("ཆད་", TokenKind::KwBreak),
            ("མུ་མཐུད་", TokenKind::KwContinue),
            ("སླར་ལོག་", TokenKind::KwReturn),
            ("འགྲིག་པ་", TokenKind::KwMatch),
            ("བཀོད་", TokenKind::KwPrint),
        ];

        for (text, expected) in keywords {
            let got = kinds(text);
            assert_eq!(got, vec![*expected], "lexing {text:?}");
        }
    }

    #[test]
    fn distinguishes_argv_argc_from_the_input_builtin_prefix() {
        assert_eq!(kinds("ནང་འཇུག་"), vec![TokenKind::BuiltinInput]);
        assert_eq!(kinds("ནང་འཇུག་ཐོ་"), vec![TokenKind::GlobalArgv]);
        assert_eq!(kinds("ནང་འཇུག་གྲངས་"), vec![TokenKind::GlobalArgc]);
    }

    #[test]
    fn parses_multi_digit_tibetan_integers() {
        assert_eq!(kinds("༡༠"), vec![TokenKind::Integer]);
        assert_eq!(tibetan_digits_to_i64("༡༠"), Some(10));
        assert_eq!(tibetan_digits_to_i64("༡༢༣"), Some(123));
        assert_eq!(tibetan_digits_to_i64("༠"), Some(0));
    }

    #[test]
    fn lexes_hello_world_example() {
        let source = r#"བཀོད་ "Hello, Druk Lang!";"#;
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::KwPrint,
                TokenKind::StringLiteral,
                TokenKind::Semi
            ]
        );
    }

    #[test]
    fn lexes_arithmetic_declaration() {
        let source = "གྲངས་ x = ༡༠;";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::KwNumber,
                TokenKind::Ident,
                TokenKind::Eq,
                TokenKind::Integer,
                TokenKind::Semi,
            ]
        );
    }

    #[test]
    fn lexes_lambda_arrow_distinct_from_minus() {
        let source = "n -> n * ༢";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::Ident,
                TokenKind::Arrow,
                TokenKind::Ident,
                TokenKind::Star,
                TokenKind::Integer,
            ]
        );
    }

    #[test]
    fn lexes_break_and_continue_inside_a_while_body() {
        let source = "ཡང་བསྐྱར་ (བདེན་པ་) { ཆད་; མུ་མཐུད་; }";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::KwWhile,
                TokenKind::LParen,
                TokenKind::KwTrue,
                TokenKind::RParen,
                TokenKind::LBrace,
                TokenKind::KwBreak,
                TokenKind::Semi,
                TokenKind::KwContinue,
                TokenKind::Semi,
                TokenKind::RBrace,
            ]
        );
    }

    #[test]
    fn lexes_a_multi_syllable_dzongkha_compound_identifier_as_one_token() {
        let source = "གྲངས་ སློབ་ཕྲུག་གི་སྐར་གྲངས་ = ༡༠;";
        let (tokens, diagnostics) = tokenize(source);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwNumber,
                TokenKind::Ident,
                TokenKind::Eq,
                TokenKind::Integer,
                TokenKind::Semi,
            ]
        );
        assert_eq!(tokens[1].text, "སློབ་ཕྲུག་གི་སྐར་གྲངས་");
    }

    #[test]
    fn reports_a_diagnostic_for_unrecognized_input_without_panicking() {
        let (tokens, diagnostics) = tokenize("x = @;");
        assert!(!diagnostics.is_empty());
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Ident));
    }
}
