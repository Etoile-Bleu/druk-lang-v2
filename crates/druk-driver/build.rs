//! Builds `druk-runtime` as a standalone static library into a dedicated
//! target directory under `OUT_DIR`, so `link.rs` can `include_bytes!` it
//! into the `druk` binary itself rather than searching for it next to the
//! running executable at link time.
//!
//! The straightforward approach (have `link.rs` find the `druk-runtime`
//! static library Cargo already builds as a side effect of the ordinary
//! workspace build) only works inside a full `cargo build` output tree:
//! that artifact lands in a hash-suffixed name inside the `deps/`
//! directory next to the `druk` binary, which does not exist once `druk`
//! is copied anywhere else, including every binary this project
//! distributes through GitHub Releases. Embedding the bytes at compile
//! time makes `druk build` work from any standalone copy of the `druk`
//! binary, which is the whole point of distributing a single executable.
//!
//! A nested `cargo build -p druk-runtime` run against a target directory
//! scoped to this crate's own `OUT_DIR` is used instead of trying to
//! locate the hash-suffixed artifact the outer build already produces:
//! Cargo only uplifts a crate's compiled artifact to a clean, unhashed
//! filename when that crate is a *root* build target (built via `-p`),
//! never when it's only pulled in transitively as a dependency, so there
//! is no documented way to predict the hashed filename from here. This
//! costs compiling `druk-runtime` (and its own small dependency tree,
//! `druk-diagnostics` and `druk-gc`, neither of which touch LLVM) a
//! second time, which is cheap, instead of depending on undocumented
//! Cargo internals.

// A build script never processes untrusted `.druk` source; every failure
// here means the build environment itself is broken (cargo didn't set an
// env var it always sets, or the nested `cargo build` below failed), the
// same "this is an environment/compiler-bug class failure" territory the
// `ice!()` panic path exists for elsewhere in this workspace, just not
// expressible through that macro from a build script. Allowed narrowly,
// here only, same as `ice!()`'s own definition site.
#![allow(clippy::expect_used, clippy::panic)]

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo always sets OUT_DIR"));
    let target = env::var("TARGET").expect("cargo always sets TARGET");
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("cargo always sets CARGO_MANIFEST_DIR"),
    );

    // crates/druk-driver -> the workspace root two levels up.
    let workspace_root = manifest_dir
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root should resolve from CARGO_MANIFEST_DIR");

    let embed_target_dir = out_dir.join("druk-runtime-embed");

    let status = Command::new(&cargo)
        .current_dir(&workspace_root)
        .args(["build", "--release", "--locked", "-p", "druk-runtime"])
        .arg("--target")
        .arg(&target)
        .arg("--target-dir")
        .arg(&embed_target_dir)
        .status()
        .expect("failed to invoke cargo to build druk-runtime for embedding");

    assert!(
        status.success(),
        "building druk-runtime for embedding failed (see output above)"
    );

    let lib_name = if target.contains("msvc") {
        "druk_runtime.lib"
    } else {
        "libdruk_runtime.a"
    };
    let lib_path: &Path = &embed_target_dir
        .join(&target)
        .join("release")
        .join(lib_name);
    assert!(
        lib_path.exists(),
        "expected {} to exist after building druk-runtime",
        lib_path.display()
    );

    println!(
        "cargo:rustc-env=DRUK_RUNTIME_STATICLIB={}",
        lib_path.display()
    );
    // Re-run only when druk-runtime's own source (or this workspace's lock
    // file, which pins its transitive dependencies) actually changes, not
    // on every build.
    println!(
        "cargo:rerun-if-changed={}",
        workspace_root.join("crates/druk-runtime/src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        workspace_root.join("Cargo.lock").display()
    );
}
