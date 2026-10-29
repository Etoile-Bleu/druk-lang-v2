use std::fmt;
use std::ops::Range;

use ariadne::{Color, Config, IndexType, Label as AriadneLabel, Report, ReportKind, Source};

pub type Span = Range<usize>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

impl Label {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            labels: Vec::new(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            labels: Vec::new(),
        }
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::new(span, message));
        self
    }

    fn primary_span(&self) -> Span {
        self.labels
            .first()
            .map(|label| label.span.clone())
            .unwrap_or(0..0)
    }

    /// Renders this diagnostic as a human-readable, ariadne-formatted report
    /// against `source`. `source_name` is the display name (usually the file
    /// path) ariadne attributes the report to.
    pub fn render(&self, source_name: &str, source: &str) -> String {
        let kind = match self.severity {
            Severity::Error => ReportKind::Error,
            Severity::Warning => ReportKind::Warning,
        };
        let color = match self.severity {
            Severity::Error => Color::Red,
            Severity::Warning => Color::Yellow,
        };

        let mut builder = Report::build(kind, (source_name, self.primary_span()))
            .with_config(Config::default().with_index_type(IndexType::Byte))
            .with_message(&self.message);
        for label in &self.labels {
            builder = builder.with_label(
                AriadneLabel::new((source_name, label.span.clone()))
                    .with_message(&label.message)
                    .with_color(color),
            );
        }

        let mut buffer = Vec::new();
        let result = builder
            .finish()
            .write((source_name, Source::from(source)), &mut buffer);
        tracing::debug!(?result, buffer_len = buffer.len(), "rendered diagnostic");
        String::from_utf8_lossy(&buffer).into_owned()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Diagnostic {}

/// Renders a batch of diagnostics against the same source, in order, each
/// separated by a blank line — the shape `druk-driver` hands to the user
/// after a pass that collects every error it found instead of stopping at
/// the first one.
pub fn render_all(source_name: &str, source: &str, diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.render(source_name, source))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_single_error_with_label() {
        let source = "གྲངས་ x = ;";
        let semicolon = source.find(';').expect("source contains a semicolon");
        let diagnostic = Diagnostic::error("expected an expression")
            .with_label(semicolon..semicolon + 1, "expected an expression here");

        let rendered = diagnostic.render("test.druk", source);

        assert!(rendered.contains("expected an expression"));
        assert!(rendered.contains(';'));
    }

    #[test]
    fn renders_multiple_diagnostics_in_order() {
        let source = "a b";
        let diagnostics = vec![
            Diagnostic::error("first").with_label(0..1, "a"),
            Diagnostic::warning("second").with_label(2..3, "b"),
        ];

        let rendered = render_all("test.druk", source, &diagnostics);

        let first_index = rendered.find("first").expect("first message present");
        let second_index = rendered.find("second").expect("second message present");
        assert!(first_index < second_index);
    }
}
