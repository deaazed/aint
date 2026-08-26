//! `aint` — the command-line entry point for the AINT toolchain.
//!
//! Only `aint run <file>` exists right now: it lexes, parses,
//! type-checks, and interprets a `.an` file end to end, driven by a
//! Tokio runtime so `async`/`await` are real (milestone 07). A type
//! error stops the program before the interpreter ever runs it. This
//! command exists so the CLI shape is fixed early and every later
//! milestone has a real place to plug into. See ROADMAP.md.

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

/// AINT's only iteration mechanism is recursion — there are no loops —
/// and once every evaluation step is `async` (milestone 07), each level
/// of AINT-level recursion costs far more Rust stack than the default
/// thread provides (five-ish mutually recursive `async fn`s nest per
/// level). `Interpreter` holds `Rc`, so it can't be built outside and
/// moved into a spawned thread — the whole command runs inside this
/// thread's closure. See `docs/milestones/07-async-concurrency/SPEC.md`.
const STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() -> ExitCode {
    let cli = Cli::parse();

    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || match cli.command {
            Command::Run { path } => run(&path),
        })
        .expect("failed to spawn the interpreter thread")
        .join()
        .expect("the interpreter thread panicked")
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

    if let Err(err) = aint_typechecker::check_program(&program) {
        eprintln!("{}:{}", path.display(), err);
        return ExitCode::FAILURE;
    }

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: could not start the async runtime: {err}");
            return ExitCode::FAILURE;
        }
    };

    let interpreter = aint_runtime::Interpreter::new();
    match runtime.block_on(interpreter.run(&program)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}:{}", path.display(), err);
            ExitCode::FAILURE
        }
    }
}
