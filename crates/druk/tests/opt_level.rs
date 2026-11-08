use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type TestResult = Result<(), Box<dyn Error>>;

const LEVELS: [&str; 4] = ["0", "1", "2", "3"];

fn druk() -> Command {
    Command::new(env!("CARGO_BIN_EXE_druk"))
}

fn example_paths() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(examples_dir)? {
        let path = entry?.path();
        if path.extension().map(|ext| ext == "druk").unwrap_or(false) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn first_example() -> Result<PathBuf, Box<dyn Error>> {
    example_paths()?
        .into_iter()
        .next()
        .ok_or_else(|| "no bundled example found".into())
}

fn normalized_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
}

fn run_at(path: &Path, level: &str) -> Result<Output, Box<dyn Error>> {
    Ok(druk().arg("run").arg("-O").arg(level).arg(path).output()?)
}

#[test]
fn run_output_is_identical_at_every_opt_level() -> TestResult {
    for path in example_paths()? {
        let baseline = run_at(&path, "0")?;
        assert!(baseline.status.success(), "{path:?} failed at -O0");
        for level in &LEVELS[1..] {
            let output = run_at(&path, level)?;
            assert!(output.status.success(), "{path:?} failed at -O{level}");
            assert_eq!(
                normalized_stdout(&output),
                normalized_stdout(&baseline),
                "{path:?} prints differently at -O{level}"
            );
        }
    }
    Ok(())
}

#[test]
fn run_defaults_match_explicit_level_zero() -> TestResult {
    let path = first_example()?;
    let default = druk().arg("run").arg(&path).output()?;
    assert_eq!(
        normalized_stdout(&default),
        normalized_stdout(&run_at(&path, "0")?)
    );
    Ok(())
}

#[test]
fn long_flag_and_o_prefixed_value_are_accepted() -> TestResult {
    let path = first_example()?;
    let output = druk()
        .arg("run")
        .arg("--opt-level")
        .arg("O3")
        .arg(&path)
        .output()?;
    assert!(output.status.success());
    Ok(())
}

#[test]
fn unknown_level_is_rejected_with_a_plain_message() -> TestResult {
    let path = first_example()?;
    for subcommand in ["run", "build"] {
        let output = druk()
            .arg(subcommand)
            .arg("-O")
            .arg("4")
            .arg(&path)
            .output()?;
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("is not an optimization level; use 0, 1, 2 or 3"),
            "unexpected stderr: {stderr}"
        );
    }
    Ok(())
}

#[test]
fn built_executables_match_run_output_at_every_opt_level() -> TestResult {
    let path = example_paths()?
        .into_iter()
        .find(|path| path.to_string_lossy().contains("06_recursion"))
        .ok_or("the recursion example is missing")?;
    let expected = normalized_stdout(&run_at(&path, "0")?);
    let dir = std::env::temp_dir().join(format!("druk-opt-level-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    for level in LEVELS {
        let exe = dir.join(format!("prog_o{level}{}", std::env::consts::EXE_SUFFIX));
        let build = druk()
            .arg("build")
            .arg("-O")
            .arg(level)
            .arg(&path)
            .arg("-o")
            .arg(&exe)
            .output()?;
        assert!(
            build.status.success(),
            "build -O{level} failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        let produced = Command::new(&exe).output()?;
        assert!(produced.status.success(), "built -O{level} exited badly");
        assert_eq!(normalized_stdout(&produced), expected, "-O{level} differs");
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
