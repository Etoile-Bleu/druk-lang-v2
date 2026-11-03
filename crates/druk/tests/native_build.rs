//! `druk build` must work from a terminal that has neither a Rust toolchain
//! nor LLVM on `PATH`: all it may rely on is the platform's own linker.

use std::path::PathBuf;
use std::process::Command;

const MINIMAL_PATH: &str = if cfg!(windows) {
    r"C:\Windows\System32;C:\Windows"
} else {
    "/usr/bin:/bin"
};

#[test]
fn druk_build_works_without_rust_or_llvm_on_path() {
    let source =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples")).join("01_hello.druk");
    let mut output = std::env::temp_dir();
    output.push(format!("druk_no_toolchain_test_{}.exe", std::process::id()));

    let build = Command::new(env!("CARGO_BIN_EXE_druk"))
        .env("PATH", MINIMAL_PATH)
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .output()
        .expect("the druk binary should run");
    assert!(
        build.status.success(),
        "druk build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&output)
        .output()
        .expect("the built program should run");
    let _ = std::fs::remove_file(&output);
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("Hello, Druk Lang!"),
        "unexpected output from the built program: {stdout:?}"
    );
}
