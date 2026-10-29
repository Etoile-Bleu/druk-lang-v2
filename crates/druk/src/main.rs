use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
    Check { file: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.debug);
    match cli.command {
        Command::Check { file } => run_check(&file),
    }
}

fn init_logging(debug: bool) {
    let default_level = if debug { "debug" } else { "warn" };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn run_check(file: &std::path::Path) -> ExitCode {
    match druk_driver::check_file(file) {
        Ok(ast) => {
            println!(
                "{}: parsed {} top-level statement(s), no diagnostics",
                file.display(),
                ast.top_level.len()
            );
            ExitCode::SUCCESS
        }
        Err(druk_driver::DriverError::Io { path, source }) => {
            eprintln!("error: could not read {path}: {source}");
            ExitCode::FAILURE
        }
        Err(druk_driver::DriverError::Diagnostics(diagnostics)) => {
            let source = std::fs::read_to_string(file).unwrap_or_default();
            eprint!(
                "{}",
                druk_driver::render_diagnostics(&file.display().to_string(), &source, &diagnostics)
            );
            ExitCode::FAILURE
        }
    }
}
