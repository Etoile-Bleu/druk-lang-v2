//! Links a native object file against an embedded copy of the
//! `druk-runtime` static library into a standalone executable, by running
//! the platform's own linker directly: `link.exe` on Windows, the system C
//! compiler driver on Unix. No Rust toolchain is needed on the machine that
//! runs `druk build`.
//!
//! The static library's bytes, and the list of native libraries rustc said
//! it needs when it was built, are embedded into the `druk` binary itself at
//! compile time (see `build.rs`), rather than searched for on disk next to
//! the running executable: a `druk` binary copied anywhere other than its
//! own `cargo build` output tree (every binary this project distributes
//! through GitHub Releases, for instance) would otherwise have nothing to
//! link `druk build`'s output against.

use std::io;
use std::path::Path;
use std::process::Output;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

/// Baked in by `build.rs`, which compiles `druk-runtime` as a standalone
/// static library into a path under this crate's own `OUT_DIR`.
static RUNTIME_STATICLIB: &[u8] = include_bytes!(env!("DRUK_RUNTIME_STATICLIB"));

/// What `rustc --print=native-static-libs` reported for that same build, in
/// the syntax the platform's linker takes (`-lm -ldl` or `kernel32.lib`).
const RUNTIME_NATIVE_LIBS: &str = env!("DRUK_RUNTIME_NATIVE_LIBS");

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("could not write {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("{0}")]
    LinkerMissing(String),
    #[error("the linker could not create the executable:\n{0}")]
    LinkerFailed(String),
}

/// Links `object_path` (a native object file, as produced by
/// `druk_codegen_llvm::compile_to_object`) against the embedded
/// `druk-runtime` static library into a standalone executable at
/// `output_path`. The `main` symbol the object file defines becomes the
/// program's entry point, reached through the C runtime's own startup code.
pub fn link_executable(object_path: &Path, output_path: &Path) -> Result<(), LinkError> {
    let runtime_lib_ext = if cfg!(windows) { "lib" } else { "a" };
    let runtime_lib_path = object_path.with_extension(format!("runtime.{runtime_lib_ext}"));
    std::fs::write(&runtime_lib_path, RUNTIME_STATICLIB).map_err(|source| LinkError::Io {
        path: runtime_lib_path.display().to_string(),
        source,
    })?;

    let result = platform::run_linker(
        object_path,
        &runtime_lib_path,
        RUNTIME_NATIVE_LIBS,
        output_path,
    );

    let _ = std::fs::remove_file(&runtime_lib_path);
    result
}

/// Turns the outcome of running a linker into a [`LinkError`] a person can
/// act on. `missing_hint` names what to install when the program itself
/// could not be started. Some linkers (MSVC's) print their errors to
/// standard output, so both streams are reported.
fn finish(result: io::Result<Output>, missing_hint: &str) -> Result<(), LinkError> {
    match result {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let mut report = String::from_utf8_lossy(&output.stdout).into_owned();
            report.push_str(&String::from_utf8_lossy(&output.stderr));
            Err(LinkError::LinkerFailed(report.trim().to_string()))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            Err(LinkError::LinkerMissing(missing_hint.to_string()))
        }
        Err(source) => Err(LinkError::Io {
            path: "the linker".to_string(),
            source,
        }),
    }
}
