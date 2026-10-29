use std::path::Path;

use druk_diagnostics::Diagnostic;
use druk_hir::Hir;

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("could not read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{} diagnostic(s) reported", .0.len())]
    Diagnostics(Vec<Diagnostic>),
}

/// Runs the frontend and HIR passes (lexer, parser, name resolution, type
/// checking) over `source`, returning the checked [`Hir`], or every
/// diagnostic collected if any pass failed.
///
/// Later pipeline stages (IR, codegen) are not wired in yet — see
/// `druk-ir`/`druk-codegen-llvm`.
pub fn check_source(source: &str) -> Result<Hir, Vec<Diagnostic>> {
    let (ast, parse_diagnostics) = druk_parser::parse(source);
    if !parse_diagnostics.is_empty() {
        return Err(parse_diagnostics);
    }
    druk_hir::check(ast)
}

/// Reads `path` from disk and runs [`check_source`] over its contents.
pub fn check_file(path: &Path) -> Result<Hir, DriverError> {
    let source = std::fs::read_to_string(path).map_err(|source| DriverError::Io {
        path: path.display().to_string(),
        source,
    })?;
    check_source(&source).map_err(DriverError::Diagnostics)
}

/// Renders every diagnostic in `diagnostics` against `source_name`/`source`,
/// in the order they were reported.
pub fn render_diagnostics(source_name: &str, source: &str, diagnostics: &[Diagnostic]) -> String {
    druk_diagnostics::render_all(source_name, source, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_every_bundled_example_successfully() {
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let entries = std::fs::read_dir(examples_dir).expect("examples dir exists");
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
                let result = check_file(&path);
                assert!(result.is_ok(), "failed to check {path:?}: {result:?}");
            }
        }
    }

    #[test]
    fn reports_a_driver_error_for_a_missing_file() {
        let result = check_file(Path::new("this/path/does/not/exist.druk"));
        assert!(matches!(result, Err(DriverError::Io { .. })));
    }
}
