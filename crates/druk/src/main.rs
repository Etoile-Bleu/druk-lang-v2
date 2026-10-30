use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use druk_driver::DriverError;

#[derive(Parser)]
#[command(name = "druk", about = "The Druk language compiler")]
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
        Command::Run { file, args } => report(
            &file,
            druk_driver::run_file(&file, &args).map(|()| String::new()),
        ),
    }
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
    }
}
