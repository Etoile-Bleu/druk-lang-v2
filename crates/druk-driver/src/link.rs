//! Links a native object file against an embedded copy of the
//! `druk-runtime` static library into a standalone executable, by
//! shelling out to `rustc` itself rather than a system linker directly.
//! `rustc` already knows how to find and invoke the right linker for this
//! host (including its own bundled `rust-lld` on a machine with no
//! separate linker installed) and which native system libraries a Rust
//! static library needs linked alongside it, which this crate has no
//! reason to duplicate.
//!
//! The static library's bytes are embedded into the `druk` binary itself
//! at compile time (see `build.rs`), rather than searched for on disk
//! next to the running executable: a `druk` binary copied anywhere other
//! than its own `cargo build` output tree (every binary this project
//! distributes through GitHub Releases, for instance) would otherwise
//! have nothing to link `druk build`'s output against.

use std::path::Path;
use std::process::Command;

/// Baked in by `build.rs`, which compiles `druk-runtime` as a standalone
/// static library into a path under this crate's own `OUT_DIR` and
/// exposes it via this env var, rather than relying on the hash-suffixed
/// copy Cargo separately produces as a side effect of the ordinary
/// workspace build (that copy sits next to the `druk` binary only inside
/// a full `cargo build` output tree, see `build.rs`'s own module doc).
static RUNTIME_STATICLIB: &[u8] = include_bytes!(env!("DRUK_RUNTIME_STATICLIB"));

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("could not write {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("rustc failed to link the final executable:\n{0}")]
    Rustc(String),
}

/// Links `object_path` (a native object file, as produced by
/// `druk_codegen_llvm::compile_to_object`) against the embedded
/// `druk-runtime` static library into a standalone executable at
/// `output_path`. The `main` symbol the object file defines becomes the
/// real OS entry point of the finished executable; the tiny
/// `#![no_main]` Rust source compiled alongside it (and immediately
/// deleted afterward) contributes nothing but the ordinary Rust/C runtime
/// startup glue, never its own `main`.
pub fn link_executable(object_path: &Path, output_path: &Path) -> Result<(), LinkError> {
    let runtime_lib_ext = if cfg!(target_os = "windows") {
        "lib"
    } else {
        "a"
    };
    let runtime_lib_path = object_path.with_extension(format!("runtime.{runtime_lib_ext}"));
    std::fs::write(&runtime_lib_path, RUNTIME_STATICLIB).map_err(|source| LinkError::Io {
        path: runtime_lib_path.display().to_string(),
        source,
    })?;

    let glue_path = object_path.with_extension("glue.rs");
    let glue_write_result = std::fs::write(&glue_path, "#![no_main]\n");

    let result = glue_write_result.and_then(|()| {
        Command::new("rustc")
            .args([
                "--crate-type",
                "bin",
                "--crate-name",
                "druk_build_glue",
                "--edition",
                "2021",
                "-O",
            ])
            .arg("-C")
            .arg(format!("link-arg={}", object_path.display()))
            .arg("-C")
            .arg(format!("link-arg={}", runtime_lib_path.display()))
            .arg("-o")
            .arg(output_path)
            .arg(&glue_path)
            .output()
    });

    let _ = std::fs::remove_file(&glue_path);
    let _ = std::fs::remove_file(&runtime_lib_path);

    let output = result.map_err(|source| LinkError::Io {
        path: "rustc".to_string(),
        source,
    })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(LinkError::Rustc(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
}
