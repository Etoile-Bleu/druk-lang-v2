//! Links through the system C compiler driver, which knows where the C
//! runtime's startup files and libraries live. `CC` overrides the program
//! name, like every other build tool on the platform.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use super::{finish, LinkError};

const MISSING: &str = "druk needs a C compiler to create an executable, and could not find one. On Debian or Ubuntu, install one with: sudo apt install build-essential";

pub(super) fn run_linker(
    object_path: &Path,
    runtime_lib_path: &Path,
    native_libs: &str,
    output_path: &Path,
) -> Result<(), LinkError> {
    let compiler = std::env::var_os("CC")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from("cc"));
    run_with(
        &compiler,
        object_path,
        runtime_lib_path,
        native_libs,
        output_path,
    )
}

fn run_with(
    compiler: &OsStr,
    object_path: &Path,
    runtime_lib_path: &Path,
    native_libs: &str,
    output_path: &Path,
) -> Result<(), LinkError> {
    let mut command = Command::new(compiler);
    command
        .arg("-o")
        .arg(output_path)
        .arg(object_path)
        .arg(runtime_lib_path);
    if cfg!(target_os = "linux") {
        command.arg("-Wl,--gc-sections");
    }
    command.args(native_libs.split_whitespace());

    finish(command.output(), MISSING)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_missing_compiler_in_plain_language() {
        let result = run_with(
            OsStr::new("druk-test-no-such-compiler"),
            Path::new("a.o"),
            Path::new("b.a"),
            "-lc",
            Path::new("out"),
        );
        match result {
            Err(LinkError::LinkerMissing(message)) => {
                assert!(
                    message.contains("C compiler"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected LinkerMissing, got {other:?}"),
        }
    }
}
