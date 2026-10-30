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
/// This stops at the HIR stage by design, since `druk check` only needs to
/// know whether the source is valid. See [`run_source`] for the full
/// pipeline through `druk-ir`/`druk-codegen-llvm`.
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

/// Runs `source` through the full pipeline (frontend, HIR, IR, LLVM JIT)
/// and executes it, with `args` as the program's own `argv`/`argc`.
pub fn run_source(source: &str, args: &[String]) -> Result<(), Vec<Diagnostic>> {
    let hir = check_source(source)?;
    let ir = druk_ir::lower(&hir)?;
    druk_codegen_llvm::run(&ir, args).map_err(|diagnostic| vec![diagnostic])
}

/// Reads `path` from disk and runs [`run_source`] over its contents.
pub fn run_file(path: &Path, args: &[String]) -> Result<(), DriverError> {
    let source = std::fs::read_to_string(path).map_err(|source| DriverError::Io {
        path: path.display().to_string(),
        source,
    })?;
    run_source(&source, args).map_err(DriverError::Diagnostics)
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
    fn runs_every_bundled_example_successfully() {
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let entries = std::fs::read_dir(examples_dir).expect("examples dir exists");
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
                let result = run_file(&path, &[]);
                assert!(result.is_ok(), "failed to run {path:?}: {result:?}");
            }
        }
    }

    #[test]
    fn reports_a_driver_error_for_a_missing_file() {
        let result = check_file(Path::new("this/path/does/not/exist.druk"));
        assert!(matches!(result, Err(DriverError::Io { .. })));
    }
}
