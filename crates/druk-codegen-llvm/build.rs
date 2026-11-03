//! Refuses to build for Windows MSVC when `druk` and the prebuilt LLVM
//! libraries disagree about which C runtime to use.
//!
//! Memory that LLVM allocates must be freed by the same C runtime, otherwise
//! the heap is corrupted and the process crashes intermittently (see
//! https://github.com/TheDan64/inkwell/issues/736). The official prebuilt
//! LLVM libraries are compiled against the static runtime
//! (`RuntimeLibrary=MT_StaticRelease`) while Rust's MSVC target defaults to
//! the dynamic one, so `druk` has to be built with `crt-static`. Rather than
//! hard-coding that, this reads which runtime the LLVM libraries actually
//! request, so a future LLVM built the other way round is caught just as well.
//! `llvm-sys` itself switches its system libraries to static linking under
//! `crt-static`, so that combination is the supported one, not a workaround.

use std::env;
use std::path::Path;
use std::process::exit;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Runtime {
    Static,
    Dynamic,
}

impl Runtime {
    fn describe(self) -> &'static str {
        match self {
            Runtime::Static => "static",
            Runtime::Dynamic => "dynamic",
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LLVM_SYS_221_PREFIX");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }

    let features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    let building = if features.split(',').any(|feature| feature == "crt-static") {
        Runtime::Static
    } else {
        Runtime::Dynamic
    };

    let Some(prefix) = env::var_os("LLVM_SYS_221_PREFIX") else {
        println!("cargo:warning=LLVM_SYS_221_PREFIX is not set, so the C runtime check against LLVM was skipped");
        return;
    };
    let Some(llvm) = llvm_runtime(Path::new(&prefix)) else {
        println!("cargo:warning=could not tell which C runtime the LLVM libraries use, so the check against LLVM was skipped");
        return;
    };

    if llvm == building {
        return;
    }

    let fix = match llvm {
        Runtime::Static => {
            "Add `-C target-feature=+crt-static` to your Rust flags, for example in\n\
             `.cargo/config.toml`:\n\
             \n\
             [build]\n\
             rustflags = [\"-C\", \"target-feature=+crt-static\"]"
        }
        Runtime::Dynamic => "Remove `-C target-feature=+crt-static` from your Rust flags.",
    };
    eprintln!(
        "\nerror: druk and LLVM disagree about the C runtime on Windows.\n\
         \n\
         The LLVM libraries use the {} C runtime but this build uses the {} one.\n\
         \n\
         {fix}\n\
         \n\
         Mixing them makes the program crash intermittently. See the Building\n\
         section of README.md.\n",
        llvm.describe(),
        building.describe(),
    );
    exit(1);
}

/// Reads the `RuntimeLibrary` annotation the MSVC linker stores in every
/// object of the LLVM static libraries, e.g. `RuntimeLibrary=MT_StaticRelease`.
/// Debug runtimes are reported as unknown: Rust never links those.
fn llvm_runtime(prefix: &Path) -> Option<Runtime> {
    let bytes = std::fs::read(prefix.join("lib").join("LLVMSupport.lib")).ok()?;
    let marker = b"RuntimeLibrary=";
    let start = bytes
        .windows(marker.len())
        .position(|window| window == marker)?
        + marker.len();
    let name: Vec<u8> = bytes[start..]
        .iter()
        .copied()
        .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        .collect();
    match name.as_slice() {
        b"MT_StaticRelease" => Some(Runtime::Static),
        b"MD_DynamicRelease" => Some(Runtime::Dynamic),
        _ => None,
    }
}
