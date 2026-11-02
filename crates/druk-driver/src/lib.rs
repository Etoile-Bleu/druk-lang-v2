mod link;

use std::path::Path;

use druk_diagnostics::Diagnostic;
use druk_hir::Hir;

pub use link::LinkError;

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
    #[error(transparent)]
    Link(#[from] LinkError),
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

/// Compiles `source` to a standalone native executable at `output_path`:
/// the full pipeline through `druk-ir`, then `druk-codegen-llvm`'s object
/// emission, then linking that object against the `druk-runtime` static
/// library (see `link::link_executable`). The intermediate object file is
/// written next to `output_path` and removed again once linking finishes
/// (or fails), successful or not.
pub fn build_source(source: &str, output_path: &Path) -> Result<(), DriverError> {
    let hir = check_source(source).map_err(DriverError::Diagnostics)?;
    let ir = druk_ir::lower(&hir).map_err(DriverError::Diagnostics)?;

    let object_path = output_path.with_extension("o");
    druk_codegen_llvm::compile_to_object(&ir, &object_path)
        .map_err(|diagnostic| DriverError::Diagnostics(vec![diagnostic]))?;

    let link_result = link::link_executable(&object_path, output_path);
    let _ = std::fs::remove_file(&object_path);
    link_result.map_err(DriverError::Link)
}

/// Reads `path` from disk and runs [`build_source`] over its contents.
pub fn build_file(path: &Path, output_path: &Path) -> Result<(), DriverError> {
    let source = std::fs::read_to_string(path).map_err(|source| DriverError::Io {
        path: path.display().to_string(),
        source,
    })?;
    build_source(&source, output_path)
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

    #[test]
    fn builds_and_runs_a_standalone_executable() {
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
        let source_path = Path::new(examples_dir).join("01_hello.druk");

        let mut output_path = std::env::temp_dir();
        output_path.push(format!("druk_native_build_test_{}.exe", std::process::id()));

        let build_result = build_file(&source_path, &output_path);
        assert!(
            build_result.is_ok(),
            "failed to build a standalone executable: {build_result:?}"
        );

        let run_output = std::process::Command::new(&output_path)
            .output()
            .expect("the built executable should run");
        let stdout = String::from_utf8_lossy(&run_output.stdout);
        assert!(
            stdout.contains("Hello, Druk Lang!"),
            "unexpected output from the standalone executable: {stdout:?}"
        );

        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn a_nested_loops_break_and_continue_target_only_their_own_loop() {
        // `break`/`continue` compile to a branch on a per-loop target block
        // stack (see druk-codegen-llvm's `loop_stack`). The one way that
        // stack could misbehave is scoping to the wrong loop once two are
        // nested: an inner `break` escaping to the outer loop's exit, or an
        // inner `continue` jumping to the outer loop's condition. This
        // source is built to make either mistake visible in the output: the
        // inner loop should print 1 and 3 (skipping 2 via continue, then
        // breaking at 4) on every one of the outer loop's 3 iterations, and
        // the outer loop's own print after the inner loop must still run
        // every time.
        let source = "\
            གྲངས་ i = ༠;\n\
            ཡང་བསྐྱར་ (i < ༣) {\n\
                i = i + ༡;\n\
                གྲངས་ j = ༠;\n\
                ཡང་བསྐྱར་ (j < ༥) {\n\
                    j = j + ༡;\n\
                    གལ་སྲིད་ (j == ༢) {\n\
                        མུ་མཐུད་;\n\
                    }\n\
                    གལ་སྲིད་ (j == ༤) {\n\
                        ཆད་;\n\
                    }\n\
                    བཀོད་ j;\n\
                }\n\
                བཀོད་ i;\n\
            }\n\
        ";

        let mut output_path = std::env::temp_dir();
        output_path.push(format!(
            "druk_nested_break_continue_test_{}.exe",
            std::process::id()
        ));

        let build_result = build_source(source, &output_path);
        assert!(
            build_result.is_ok(),
            "failed to build a standalone executable: {build_result:?}"
        );

        let run_output = std::process::Command::new(&output_path)
            .output()
            .expect("the built executable should run");
        let stdout = String::from_utf8_lossy(&run_output.stdout);
        let expected = "1\n3\n1\n1\n3\n2\n1\n3\n3\n";
        assert_eq!(
            stdout.replace("\r\n", "\n"),
            expected,
            "break/continue in a nested loop did not stay scoped to the inner loop"
        );

        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn a_for_loops_continue_still_runs_the_post_clause() {
        // `for`'s `continue` branches to its own `post` block rather than
        // straight to `cond` (see `druk-codegen-llvm`'s `compile_for`): the
        // one way that could regress is `continue` targeting `cond`
        // directly, which would skip the increment and infinite-loop
        // forever on `i == 2`. This source makes that failure mode visible
        // as a hang rather than a silently wrong answer, which is the
        // sharper signal for exactly this bug.
        let source = "\
            རེ་རེར་ (གྲངས་ i = ༠; i < ༥; i = i + ༡) {\n\
                གལ་སྲིད་ (i == ༢) {\n\
                    མུ་མཐུད་;\n\
                }\n\
                གལ་སྲིད་ (i == ༤) {\n\
                    ཆད་;\n\
                }\n\
                བཀོད་ i;\n\
            }\n\
        ";

        let mut output_path = std::env::temp_dir();
        output_path.push(format!(
            "druk_for_continue_post_test_{}.exe",
            std::process::id()
        ));

        let build_result = build_source(source, &output_path);
        assert!(
            build_result.is_ok(),
            "failed to build a standalone executable: {build_result:?}"
        );

        let run_output = std::process::Command::new(&output_path)
            .output()
            .expect("the built executable should run");
        let stdout = String::from_utf8_lossy(&run_output.stdout);
        let expected = "0\n1\n3\n";
        assert_eq!(
            stdout.replace("\r\n", "\n"),
            expected,
            "continue inside a for loop did not run the post clause before the next condition check"
        );

        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn a_for_loop_nested_inside_a_while_loop_scopes_break_and_continue_correctly() {
        // The loop-target stack in druk-codegen-llvm is homogeneous
        // ((BasicBlock, BasicBlock) pairs) regardless of which kind of loop
        // pushed it, so mixing loop kinds is the case most likely to expose
        // a scoping bug the same-kind nested test above would not catch.
        let source = "\
            གྲངས་ i = ༠;\n\
            ཡང་བསྐྱར་ (i < ༢) {\n\
                i = i + ༡;\n\
                རེ་རེར་ (གྲངས་ j = ༠; j < ༥; j = j + ༡) {\n\
                    གལ་སྲིད་ (j == ༢) {\n\
                        མུ་མཐུད་;\n\
                    }\n\
                    གལ་སྲིད་ (j == ༤) {\n\
                        ཆད་;\n\
                    }\n\
                    བཀོད་ j;\n\
                }\n\
                བཀོད་ i;\n\
            }\n\
        ";

        let mut output_path = std::env::temp_dir();
        output_path.push(format!("druk_for_in_while_test_{}.exe", std::process::id()));

        let build_result = build_source(source, &output_path);
        assert!(
            build_result.is_ok(),
            "failed to build a standalone executable: {build_result:?}"
        );

        let run_output = std::process::Command::new(&output_path)
            .output()
            .expect("the built executable should run");
        let stdout = String::from_utf8_lossy(&run_output.stdout);
        let expected = "0\n1\n3\n1\n0\n1\n3\n2\n";
        assert_eq!(
            stdout.replace("\r\n", "\n"),
            expected,
            "break/continue did not stay scoped correctly across mixed while/for nesting"
        );

        let _ = std::fs::remove_file(&output_path);
    }
}
