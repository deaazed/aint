//! `aint` — the command-line entry point for the AINT toolchain.
//!
//! `aint run <file>` lexes, parses, type-checks, and interprets a
//! `.an` file end to end, driven by a Tokio runtime so `async`/`await`
//! are real (milestone 07). A type error stops the program before the
//! interpreter ever runs it.
//!
//! `aint test <file>` (milestone 15) runs every top-level `test` block
//! in a file, each in its own fresh, isolated `Interpreter` configured
//! from that block's own `mock` statements — see
//! `docs/milestones/15-deterministic-ai-testing/SPEC.md`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aint_ast::Program;
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
        /// Execute via the milestone-22 bytecode VM
        /// (`AST -> AIR -> Bytecode -> AINT VM`) instead of the
        /// tree-walking interpreter. Covers AINT's deterministic core
        /// only - fails clearly, not silently, on `infer`/`tool`/
        /// `async`/`Distribution<T>`. See
        /// docs/milestones/22-bytecode-vm/SPEC.md.
        #[arg(long)]
        vm: bool,
    },
    /// Run every `test` block in an AINT source file.
    Test {
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
            Command::Run { path, vm: false } => run(&path),
            Command::Run { path, vm: true } => run_vm(&path),
            Command::Test { path } => test(&path),
        })
        .expect("failed to spawn the interpreter thread")
        .join()
        .expect("the interpreter thread panicked")
}

/// Reads, parses, and type-checks `path` — the first three steps
/// `run` and `test` both need, and both report failures in it
/// identically.
fn parse_and_check(path: &Path) -> Result<Program, ExitCode> {
    let source = fs::read_to_string(path).map_err(|err| {
        eprintln!("error: could not read {}: {err}", path.display());
        ExitCode::FAILURE
    })?;

    let program = aint_parser::parse_source(&source).map_err(|err| {
        eprintln!("{}:{}", path.display(), err);
        ExitCode::FAILURE
    })?;

    aint_typechecker::check_program(&program).map_err(|err| {
        eprintln!("{}:{}", path.display(), err);
        ExitCode::FAILURE
    })?;

    Ok(program)
}

fn build_tokio_runtime() -> Result<tokio::runtime::Runtime, ExitCode> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| {
            eprintln!("error: could not start the async runtime: {err}");
            ExitCode::FAILURE
        })
}

fn run(path: &Path) -> ExitCode {
    let program = match parse_and_check(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let runtime = match build_tokio_runtime() {
        Ok(runtime) => runtime,
        Err(code) => return code,
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

/// `aint run --vm` (milestone 22): the same parse-and-type-check gate
/// as `run`, then `AST -> AIR -> Bytecode -> AINT VM` instead of the
/// tree-walking interpreter - synchronously, no Tokio runtime needed,
/// since the VM's in-scope subset has no `async`/`await` at all. See
/// `docs/milestones/22-bytecode-vm/SPEC.md` for exactly what's
/// covered and what fails clearly instead of running.
fn run_vm(path: &Path) -> ExitCode {
    let program = match parse_and_check(path) {
        Ok(program) => program,
        Err(code) => return code,
    };

    let air = match aint_ir::lower(&program) {
        Ok(air) => air,
        Err(err) => {
            eprintln!("{}: {err:?}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let compiled = match aint_vm::compile(&air) {
        Ok(compiled) => compiled,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let mut vm = aint_vm::Vm::new(std::io::stdout());
    match vm.run(&compiled) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}:{}", path.display(), err);
            ExitCode::FAILURE
        }
    }
}

fn test(path: &Path) -> ExitCode {
    let program = match parse_and_check(path) {
        Ok(program) => program,
        Err(code) => return code,
    };
    let runtime = match build_tokio_runtime() {
        Ok(runtime) => runtime,
        Err(code) => return code,
    };

    let outcomes = runtime.block_on(aint_runtime::run_tests(&program));
    if outcomes.is_empty() {
        println!("no test blocks found in {}", path.display());
        return ExitCode::SUCCESS;
    }

    let mut failed = 0usize;
    for outcome in &outcomes {
        match &outcome.result {
            Ok(()) => println!("test \"{}\" ... ok", outcome.name),
            Err(err) => {
                failed += 1;
                println!("test \"{}\" ... FAILED", outcome.name);
                println!("  {}:{err}", path.display());
            }
        }
    }

    let passed = outcomes.len() - failed;
    println!();
    println!("{} run, {passed} passed, {failed} failed", outcomes.len());

    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
