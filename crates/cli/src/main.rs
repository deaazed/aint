//! `aint` — the command-line entry point for the AINT toolchain.
//!
//! Only `aint run <file>` exists right now: it lexes, parses, and
//! interprets a `.an` file end to end. This command exists so the CLI
//! shape is fixed early and every later milestone has a real place to
//! plug into. See ROADMAP.md.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "aint",
    version,
    about = "The AINT programming language toolchain"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run an AINT source file.
    Run {
        /// Path to a .an source file.
        path: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { path } => run(&path),
    }
}

fn run(path: &Path) -> ExitCode {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: could not read {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let program = match aint_parser::parse_source(&source) {
        Ok(program) => program,
        Err(err) => {
            eprintln!("{}:{}", path.display(), err);
            return ExitCode::FAILURE;
        }
    };

    let interpreter = aint_runtime::Interpreter::new();
    match interpreter.run(&program) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}:{}", path.display(), err);
            ExitCode::FAILURE
        }
    }
}
