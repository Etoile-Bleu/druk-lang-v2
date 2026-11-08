//! Links with the Microsoft linker. `find-msvc-tools` is the same discovery
//! `rustc` and the `cc` crate use: it finds `link.exe` through the Visual
//! Studio installation records and returns the environment (`LIB`, `PATH`)
//! it needs, so this works from a plain terminal, not only a developer prompt.

use std::ffi::OsString;
use std::path::Path;

use super::{finish, LinkError};

const TARGET: &str = "x86_64-pc-windows-msvc";

const MISSING: &str = "druk needs the Microsoft C++ build tools to create an executable, and could not find them. Install \"Build Tools for Visual Studio\" with the \"Desktop development with C++\" workload, then try again.";

pub(super) fn run_linker(
    object_path: &Path,
    runtime_lib_path: &Path,
    native_libs: &str,
    output_path: &Path,
) -> Result<(), LinkError> {
    let tool = find_msvc_tools::find_tool(TARGET, "link.exe")
        .ok_or_else(|| LinkError::LinkerMissing(MISSING.to_string()))?;

    let mut out_arg = OsString::from("/OUT:");
    out_arg.push(output_path);

    let result = tool
        .to_command()
        .args(["/NOLOGO", "/INCREMENTAL:NO", "/SUBSYSTEM:CONSOLE"])
        .arg(out_arg)
        .arg(object_path)
        .arg(runtime_lib_path)
        .args(native_libs.split_whitespace())
        .output();

    finish(result, MISSING)
}
