use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use druk_driver::{DriverError, OptLevel};

#[derive(Parser)]
#[command(name = "druk", about = "The Druk language compiler", version)]
struct Cli {
    /// Enable verbose debug logging (overridden by RUST_LOG if set).
    #[arg(long, global = true)]
    debug: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and type-check a .druk file without running it.
    Check { file: PathBuf },
    /// Compile and run a .druk file via the LLVM JIT.
    Run {
        file: PathBuf,
        /// Extra arguments passed through to the Druk program's own
        /// `argv`/`argc`.
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// How hard LLVM optimizes the generated code: 0, 1, 2 or 3.
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: OptLevel,
    },
    /// Compile a .druk file to a standalone native executable. Needs the
    /// platform's linker: the Visual Studio Build Tools on Windows, a C
    /// compiler such as gcc on Linux.
    Build {
        file: PathBuf,
        /// Where to write the executable. Defaults to the input file's
        /// name (without `.druk`) in the current directory.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// How hard LLVM optimizes the generated code: 0, 1, 2 or 3.
        #[arg(short = 'O', long, default_value = "2")]
        opt_level: OptLevel,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.debug);
    match cli.command {
        Command::Check { file } => report(
            &file,
            druk_driver::check_file(&file).map(|hir| {
                format!(
                    "{}: checked {} top-level statement(s), no diagnostics",
                    file.display(),
                    hir.ast.top_level.len()
                )
            }),
        ),
        Command::Run {
            file,
            args,
            opt_level,
        } => report(
            &file,
            druk_driver::run_file(&file, &args, opt_level).map(|()| String::new()),
        ),
        Command::Build {
            file,
            output,
            opt_level,
        } => {
            let output = output.unwrap_or_else(|| default_output_path(&file));
            report(
                &file,
                druk_driver::build_file(&file, &output, opt_level)
                    .map(|()| format!("compiled {} -> {}", file.display(), output.display())),
            )
        }
    }
}

/// Derives a native executable's default output path from its source
/// file's own name: the input's file stem (dropping `.druk`), in the
/// current directory, with the host platform's usual executable suffix
/// (`.exe` on Windows, nothing elsewhere).
fn default_output_path(file: &std::path::Path) -> PathBuf {
    let stem = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "a".to_string());
    PathBuf::from(format!("{stem}{}", std::env::consts::EXE_SUFFIX))
}

fn init_logging(debug: bool) {
    let default_level = if debug { "debug" } else { "warn" };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn report(file: &std::path::Path, result: Result<String, DriverError>) -> ExitCode {
    match result {
        Ok(message) => {
            if !message.is_empty() {
                println!("{message}");
            }
            ExitCode::SUCCESS
        }
        Err(DriverError::Io { path, source }) => {
            eprintln!("error: could not read {path}: {source}");
            ExitCode::FAILURE
        }
        Err(DriverError::Diagnostics(diagnostics)) => {
            let source = std::fs::read_to_string(file).unwrap_or_default();
            eprint!(
                "{}",
                druk_driver::render_diagnostics(&file.display().to_string(), &source, &diagnostics)
            );
            ExitCode::FAILURE
        }
        Err(DriverError::Link(err)) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
