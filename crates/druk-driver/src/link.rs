//! Locates the `druk-runtime` static library built alongside the running
//! `druk` binary and links a native object file against it into a
//! standalone executable, by shelling out to `rustc` itself rather than a
//! system linker directly. `rustc` already knows how to find and invoke
//! the right linker for this host (including its own bundled `rust-lld`
//! on a machine with no separate linker installed) and which native
//! system libraries a Rust static library needs linked alongside it,
//! which this crate has no reason to duplicate.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error(
        "could not find the druk-runtime static library next to the current executable \
         (looked in {0}); build druk with `cargo build` rather than copying just the binary"
    )]
    RuntimeNotFound(String),
    #[error("could not determine the current executable's own location: {0}")]
    CurrentExe(#[source] std::io::Error),
    #[error("could not write {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("rustc failed to link the final executable:\n{0}")]
    Rustc(String),
}

/// Finds the most recently built `druk_runtime` static library
/// (`druk_runtime-<hash>.lib` on the MSVC toolchain, `libdruk_runtime-<hash>.a`
/// elsewhere) in the `deps` directory next to the currently running `druk`
/// executable. Cargo never uplifts a dependency's static library to a
/// predictable, unhashed path the way it does for the final binary itself,
/// so a glob-and-pick-newest search in `deps/` is the practical way to
/// find it from inside an already-built `druk` binary, rather than
/// depending on cargo metadata this process has no access to once built.
fn find_runtime_staticlib() -> Result<PathBuf, LinkError> {
    let exe = std::env::current_exe().map_err(LinkError::CurrentExe)?;
    let bin_dir = exe.parent().unwrap_or_else(|| Path::new("."));
    // A real `druk` binary lives directly in `target/<profile>/`, whose
    // sibling `deps/` holds every dependency's own build artifacts
    // (including this static library). A `#[test]` binary, however, is
    // itself already inside `target/<profile>/deps/`, so appending
    // another `deps` would look one directory too deep.
    let deps_dir = if bin_dir.file_name().and_then(|name| name.to_str()) == Some("deps") {
        bin_dir.to_path_buf()
    } else {
        bin_dir.join("deps")
    };

    let mut candidates: Vec<(SystemTime, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&deps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let is_candidate = (name.starts_with("druk_runtime-") && name.ends_with(".lib"))
                || (name.starts_with("libdruk_runtime-") && name.ends_with(".a"));
            if !is_candidate {
                continue;
            }
            if let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) {
                candidates.push((modified, path));
            }
        }
    }

    candidates
        .into_iter()
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
        .ok_or_else(|| LinkError::RuntimeNotFound(deps_dir.display().to_string()))
}

/// Links `object_path` (a native object file, as produced by
/// `druk_codegen_llvm::compile_to_object`) against the `druk-runtime`
/// static library into a standalone executable at `output_path`. The
/// `main` symbol the object file defines becomes the real OS entry point
/// of the finished executable; the tiny `#![no_main]` Rust source compiled
/// alongside it (and immediately deleted afterward) contributes nothing
/// but the ordinary Rust/C runtime startup glue, never its own `main`.
pub fn link_executable(object_path: &Path, output_path: &Path) -> Result<(), LinkError> {
    let runtime_lib = find_runtime_staticlib()?;

    let glue_path = object_path.with_extension("glue.rs");
    std::fs::write(&glue_path, "#![no_main]\n").map_err(|source| LinkError::Io {
        path: glue_path.display().to_string(),
        source,
    })?;

    let result = Command::new("rustc")
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
        .arg(format!("link-arg={}", runtime_lib.display()))
        .arg("-o")
        .arg(output_path)
        .arg(&glue_path)
        .output();

    let _ = std::fs::remove_file(&glue_path);

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
