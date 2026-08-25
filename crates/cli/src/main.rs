//! `aint` — the command-line entry point for the AINT toolchain.
//!
//! Only `aint run <file>` exists right now, and it doesn't run anything
//! yet: the lexer, parser, and interpreter land in milestones 02-04.
//! This command exists so the CLI shape is fixed early and every later
//! milestone has a real place to plug into. See ROADMAP.md.

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
    if !path.exists() {
        eprintln!("error: no such file: {}", path.display());
        return ExitCode::FAILURE;
    }

    eprintln!(
        "error: the AINT interpreter isn't implemented yet.\n\
         Lexing, parsing, and evaluation land in milestones 02-04 (see ROADMAP.md).\n\
         `{}` was found but can't be run yet.",
        path.display()
    );
    ExitCode::FAILURE
}
