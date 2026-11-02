//! Runs the real built `druk` binary against every bundled example and
//! snapshots its actual stdout, rather than only checking it ran without
//! erroring (`druk-driver`'s own `runs_every_bundled_example_successfully`
//! already covers that, faster, in-process). This lives here, not in
//! `druk-driver`, because `CARGO_BIN_EXE_druk` is only set for a crate's
//! own integration tests testing its own binary, not for a different
//! crate depending on it.

#[test]
fn bundled_examples_produce_the_expected_output() {
    let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
    let entries = std::fs::read_dir(examples_dir).expect("examples dir exists");
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("example file has a UTF-8 stem")
                .to_string();
            let output = std::process::Command::new(env!("CARGO_BIN_EXE_druk"))
                .arg("run")
                .arg(&path)
                .output()
                .expect("the druk binary should run");
            assert!(
                output.status.success(),
                "druk run {path:?} exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
            insta::assert_snapshot!(name, stdout);
        }
    }
}
