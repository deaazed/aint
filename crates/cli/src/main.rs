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
    /// Scaffold a new AINT package: an `aint.toml` manifest and a
    /// starter `main.an`.
    Init {
        /// Directory to create the package in. Defaults to the
        /// current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Add a local path dependency to the current package's
    /// `aint.toml`, then re-resolve and re-lock the whole dependency
    /// graph. There's no registry yet — see
    /// docs/milestones/23-package-manager/SPEC.md — so this is the
    /// only form `aint add` takes.
    Add {
        /// Path to the dependency's own package directory (one
        /// containing its own `aint.toml`).
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
            Command::Init { path } => init(&path),
            Command::Add { path } => add(&path),
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

/// `aint init` (milestone 23): scaffolds `<path>/aint.toml` and a
/// starter `<path>/main.an`. Refuses to overwrite an existing
/// manifest — this creates a new package, it doesn't reset one.
fn init(path: &Path) -> ExitCode {
    if let Err(err) = fs::create_dir_all(path) {
        eprintln!("error: could not create {}: {err}", path.display());
        return ExitCode::FAILURE;
    }

    let manifest_path = path.join(aint_package::MANIFEST_FILE_NAME);
    if manifest_path.exists() {
        eprintln!("error: {} already exists", manifest_path.display());
        return ExitCode::FAILURE;
    }

    let name = path
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "my-project".to_string());

    let manifest = aint_package::Manifest::new(name, "0.1.0");
    if let Err(err) = manifest.write_to_dir(path) {
        eprintln!("error: {err}");
        return ExitCode::FAILURE;
    }

    let main_an = path.join("main.an");
    if !main_an.exists() {
        if let Err(err) = fs::write(&main_an, "print(\"Hello, AINT!\")\n") {
            eprintln!("error: could not write {}: {err}", main_an.display());
            return ExitCode::FAILURE;
        }
    }

    println!("created {}", manifest_path.display());
    ExitCode::SUCCESS
}

/// `aint add --path` (milestone 23): adds a local path dependency to
/// the current directory's `aint.toml`, then re-resolves and
/// re-writes `aint.lock` for the whole graph — not just appending the
/// one new entry, since adding one dependency can change what the
/// full, flattened graph looks like (a shared transitive dependency,
/// a newly-introduced cycle, and so on).
fn add(dep_path: &Path) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("error: could not determine the current directory: {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut manifest = match aint_package::Manifest::read_from_dir(&cwd) {
        Ok(manifest) => manifest,
        Err(err) => {
            eprintln!("error: {err} — run `aint init` first?");
            return ExitCode::FAILURE;
        }
    };

    let dep_manifest = match aint_package::Manifest::read_from_dir(dep_path) {
        Ok(manifest) => manifest,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let name = dep_manifest.package.name.clone();
    manifest.dependencies.insert(
        name.clone(),
        aint_package::Dependency {
            path: dep_path.display().to_string(),
        },
    );
    if let Err(err) = manifest.write_to_dir(&cwd) {
        eprintln!("error: {err}");
        return ExitCode::FAILURE;
    }

    let lockfile = match aint_package::resolve(&cwd) {
        Ok(lockfile) => lockfile,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = lockfile.write_to_dir(&cwd) {
        eprintln!("error: {err}");
        return ExitCode::FAILURE;
    }

    println!("added `{name}` (path: {})", dep_path.display());
    ExitCode::SUCCESS
}
