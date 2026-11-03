//! Refuses to build for Windows MSVC without a static C runtime.
//!
//! The official prebuilt LLVM libraries are compiled against the static C
//! runtime (`RuntimeLibrary=MT_StaticRelease`), while Rust's MSVC target links
//! the dynamic one by default. Mixing the two means memory LLVM allocates is
//! freed by a different C runtime, which corrupts the heap and crashes the
//! process (see https://github.com/TheDan64/inkwell/issues/736). `llvm-sys`
//! itself switches its system libraries to static linking under `crt-static`,
//! so this is the supported combination, not a workaround.

use std::env;
use std::process::exit;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }

    let features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    if features.split(',').any(|feature| feature == "crt-static") {
        return;
    }

    eprintln!(
        "\nerror: druk must be built with a static C runtime on Windows.\n\
         \n\
         Add `-C target-feature=+crt-static` to your Rust flags, for example in\n\
         `.cargo/config.toml`:\n\
         \n\
         [build]\n\
         rustflags = [\"-C\", \"target-feature=+crt-static\"]\n\
         \n\
         Without it, LLVM and Rust use two different C runtimes and the program\n\
         crashes intermittently. See the Building section of README.md.\n"
    );
    exit(1);
}
