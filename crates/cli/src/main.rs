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
    /// Add a dependency (a local path, or — milestone 36 — a git URL)
    /// to the current package's `aint.toml`, then re-resolve and
    /// re-lock the whole dependency graph. There's still no hosted
    /// registry — see docs/milestones/36-git-dependencies/SPEC.md —
    /// so `--git` always takes a real URL, never a bare name.
    Add {
        /// Path to the dependency's own package directory (one
        /// containing its own `aint.toml`). Omit when using `--git`.
        path: Option<PathBuf>,
        /// A git URL to depend on instead of a local path.
        #[arg(long, conflicts_with = "path")]
        git: Option<String>,
        /// With `--git`: a tag, branch, or commit to pin to. Omit to
        /// resolve the default branch once, at first use.
        #[arg(long, requires = "git")]
        rev: Option<String>,
    },
    /// Parse and type-check a file without running it. Exit code
    /// reflects success; nothing is printed on success (matching
    /// `gofmt -l`/`tsc --noEmit`'s "silence means it's fine").
    Check {
        /// Path to a .an source file.
        path: PathBuf,
    },
    /// Reformat a file to AINT's canonical style, in place. Refuses
    /// (exit non-zero, file untouched) rather than silently deleting
    /// a `//` comment — see
    /// docs/milestones/24-language-tooling/SPEC.md.
    Fmt {
        /// Path to a .an source file.
        path: PathBuf,
        /// Report whether the file is already formatted instead of
        /// writing to it; exits non-zero if it isn't. For CI.
        #[arg(long)]
        check: bool,
    },
    /// Scaffold a new AINT project from a plain-English description,
    /// using the same model backend `aint run` does — requires
    /// `AINT_MODEL_URL`. One-shot: creates a new project, doesn't edit
    /// an existing one. Generated code is always run through the same
    /// gate `aint check` uses before being reported as done — see
    /// docs/milestones/32-ai-scaffolding/SPEC.md.
    Scaffold {
        /// A plain-English description of the program to generate.
        description: String,
        /// Directory to create the new package in.
        path: PathBuf,
    },
    /// Replaces the running `aint` with the latest GitHub Release
    /// build, if one is newer (milestone 42) — the same download
    /// `install.sh`/`install.ps1` do, just able to replace itself
    /// in place instead of being fetched fresh each time. Never
    /// silent or automatic: this only ever runs when explicitly
    /// invoked.
    Upgrade {
        /// Report whether a newer version is available without
        /// installing it. Exits non-zero if one is (the same
        /// CI-friendly convention `aint fmt --check` uses).
        #[arg(long)]
        check: bool,
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
    load_dotenv();
    let cli = Cli::parse();

    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || match cli.command {
            Command::Run { path, vm: false } => run(&path),
            Command::Run { path, vm: true } => run_vm(&path),
            Command::Test { path } => test(&path),
            Command::Init { path } => init(&path),
            Command::Add { path, git, rev } => add(path, git, rev),
            Command::Check { path } => check(&path),
            Command::Fmt { path, check } => fmt(&path, check),
            Command::Scaffold { description, path } => scaffold(&description, &path),
            Command::Upgrade { check } => upgrade(check),
        })
        .expect("failed to spawn the interpreter thread")
        .join()
        .expect("the interpreter thread panicked")
}

/// Loads `.env` from the current directory into the process
/// environment, if one exists (milestone 41) — every real model call
/// needed `AINT_MODEL_URL`/`AINT_MODEL_NAME`/`AINT_MODEL_API_KEY`
/// exported by hand before this, which is why a standalone project
/// using `infer` against a real model needed its own wrapper script
/// just to start with real credentials. A real environment variable
/// always wins — a `.env` value only fills in one that isn't already
/// set, never overrides one that is. Silently does nothing if no
/// `.env` file exists: this is a convenience, not a requirement, and
/// every command that doesn't touch a model (`check`/`fmt`/`init`/
/// `add`/`test`, which always mocks regardless — see
/// `CONTRIBUTING.md`) is completely unaffected either way. Runs once,
/// on the main thread, before the interpreter thread is ever spawned
/// — `std::env::set_var` documents itself as unsound to call
/// concurrently with another thread reading the environment, which is
/// exactly what would happen if this ran any later.
fn load_dotenv() {
    let Ok(text) = fs::read_to_string(".env") else {
        return;
    };
    for (key, value) in parse_dotenv(&text) {
        if std::env::var_os(&key).is_none() {
            // SAFETY: called once, from `main`, before the interpreter
            // thread (the only other thread `aint` ever spawns) exists
            // — nothing else can be reading the environment
            // concurrently yet.
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }
}

/// Parses `.env`-file text into `(key, value)` pairs — blank lines and
/// `#`-comment lines skipped, `KEY=VALUE` split at the first `=` (a
/// value may itself contain one), both sides trimmed. No quote-
/// stripping, no multi-line values, no `export` prefix — the same
/// minimal shape this project's own `.env.example` files already use.
fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let (key, value) = trimmed.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/// Reads, resolves cross-file imports, parses, and type-checks `path` —
/// the steps `run` and `test` both need, and both report failures in
/// identically. `aint-loader` (milestone 29) does the read/parse/import
/// resolution in one step, folding every `import "path" as alias` it
/// reaches into one flat `Program` before the type checker ever sees it.
fn parse_and_check(path: &Path) -> Result<Program, ExitCode> {
    let program = aint_loader::load(path).map_err(|err| {
        eprintln!("error: {err}");
        ExitCode::FAILURE
    })?;

    aint_typechecker::check_program(&program).map_err(|err| {
        eprintln!("{}:{}", path.display(), err);
        ExitCode::FAILURE
    })?;

    Ok(program)
}

/// `aint check` (milestone 24): exactly `parse_and_check`'s gate,
/// exposed on its own — for a fast "does this even type-check"
/// signal (an editor's on-save hook, CI) without paying for a
/// `Tokio` runtime or actually running the program.
fn check(path: &Path) -> ExitCode {
    match parse_and_check(path) {
        Ok(_) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

/// `aint fmt` (milestone 24). `--check` reports without writing,
/// matching `rustfmt --check`'s CI-friendly convention.
fn fmt(path: &Path, check_only: bool) -> ExitCode {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: could not read {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let formatted = match aint_fmt::format(&source) {
        Ok(formatted) => formatted,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    if check_only {
        if formatted == source {
            ExitCode::SUCCESS
        } else {
            println!("{}", path.display());
            ExitCode::FAILURE
        }
    } else {
        if formatted == source {
            return ExitCode::SUCCESS;
        }
        match fs::write(path, formatted) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("error: could not write {}: {err}", path.display());
                ExitCode::FAILURE
            }
        }
    }
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

    // `AINT_MODEL_URL` (milestone 25): `aint run` had no way to use a
    // real `Model` at all before this — every `infer` call failed
    // with "no mock response configured," even outside `aint test`,
    // since `HttpModel` (milestone 16) was never wired into the CLI.
    // Unset (the default) keeps today's behavior exactly. Tool calls
    // still have no real backend regardless — `MockTool` is the only
    // one that's ever existed; see
    // docs/milestones/25-real-application/SPEC.md.
    match std::env::var("AINT_MODEL_URL") {
        Ok(url) => {
            let model_name =
                std::env::var("AINT_MODEL_NAME").unwrap_or_else(|_| "default".to_string());
            let mut model = aint_runtime::HttpModel::new(url, model_name);
            if let Ok(api_key) = std::env::var("AINT_MODEL_API_KEY") {
                model = model.with_api_key(api_key);
            }
            let interpreter =
                aint_runtime::Interpreter::with_output_and_model(std::io::stdout(), model);
            match runtime.block_on(interpreter.run(&program)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{}:{}", path.display(), err);
                    ExitCode::FAILURE
                }
            }
        }
        Err(_) => {
            let interpreter = aint_runtime::Interpreter::new();
            match runtime.block_on(interpreter.run(&program)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{}:{}", path.display(), err);
                    ExitCode::FAILURE
                }
            }
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

/// `aint add` (milestone 23; git sources since milestone 36): adds a
/// dependency — a local path, or a git URL — to the current
/// directory's `aint.toml`, then re-resolves and re-writes `aint.lock`
/// for the whole graph — not just appending the one new entry, since
/// adding one dependency can change what the full, flattened graph
/// looks like (a shared transitive dependency, a newly-introduced
/// cycle, and so on). `clap`'s `conflicts_with`/`requires` on the
/// `Command::Add` variant already guarantee `path` and `git` can't
/// both be set, and `rev` can't appear without `git`; whether *neither*
/// `path` nor `git` was given still needs checking here.
fn add(path: Option<PathBuf>, git: Option<String>, rev: Option<String>) -> ExitCode {
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

    let (name, dependency, source_description) = if let Some(url) = git {
        let (dep_dir, _source) = match aint_package::materialize_git(&url, rev.as_deref()) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
        };
        let dep_manifest = match aint_package::Manifest::read_from_dir(&dep_dir) {
            Ok(manifest) => manifest,
            Err(err) => {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
        };
        let name = dep_manifest.package.name.clone();
        let dependency = aint_package::Dependency::Git {
            git: url.clone(),
            rev: rev.clone(),
        };
        (name, dependency, format!("git: {url}"))
    } else if let Some(dep_path) = path {
        let dep_manifest = match aint_package::Manifest::read_from_dir(&dep_path) {
            Ok(manifest) => manifest,
            Err(err) => {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
        };
        let name = dep_manifest.package.name.clone();
        let dependency = aint_package::Dependency::Path {
            path: dep_path.display().to_string(),
        };
        (name, dependency, format!("path: {}", dep_path.display()))
    } else {
        eprintln!("error: `aint add` needs either a path or `--git <url>`");
        return ExitCode::FAILURE;
    };

    manifest.dependencies.insert(name.clone(), dependency);
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

    println!("added `{name}` ({source_description})");
    ExitCode::SUCCESS
}

/// A condensed, accurate reference for AINT's actual syntax — every
/// rule here is something a real `.an` file in this repository does,
/// not a guess at what the language might support. Kept in the CLI
/// rather than generated from `docs/SPECIFICATION.md` — that document
/// is the exhaustive reference; this is deliberately the smallest
/// version that still keeps a model from inventing syntax (loops,
/// `Option` construction, dotted access) AINT doesn't have.
const SCAFFOLD_SYSTEM_PROMPT: &str = r#"You generate AINT source code. AINT is a statically-typed language. Follow these rules exactly - do not use any syntax not listed here.

Types: Int, Float, Bool, String, Unit, List<T>, Option<T>, and user-declared enums.
Bindings: `let name = expr` - one-time only, there is no reassignment and no loops anywhere. Iteration is recursion.
Functions: `fn name(param: Type, ...) -> ReturnType { ... }`, optionally `async fn`. An optional `effects [pure]` clause marks a function as calling nothing beyond other pure/stdlib functions.
Closures: `fn(param: Type) -> ReturnType { ... }` as an expression (no name) is a closure value; its type is written `fn(Type, Type) -> ReturnType`. A plain fn referenced by name without calling it also becomes a closure value.
Control flow: `if condition { ... } else { ... }` - else, when present, is always followed by a block; `else if` is allowed and chains normally. There is no `while` or `for` - iteration is recursion. Comparison/logical operators: `+ - * / == != < > <= >= && || !` - `&&`/`||` short-circuit (the right side isn't evaluated once the left already decides the result) and `!` is a unary prefix (`!condition`, not `condition!`). `if condition { value } else { value }` (each branch exactly one expression, else required) is also usable directly as a value, e.g. `let x = if a < b { "low" } else if a == b { "equal" } else { "high" }` - prefer this over duplicating a return statement in every branch when a function is just picking one of several values.
Enums: `enum Name { Variant1 Variant2 }` - a variant value is written `Name_Variant1` (one identifier, underscore-joined), used as a plain expression.
Cross-file imports: `import "./other.an" as alias` makes every fn/enum/tool/infer in other.an available as `alias_name`. A file reached this way may only contain fn/enum/tool/infer/import at its top level.
Stdlib imports: `import math` / `import string` / `import time` / `import collections` / `import distribution` / `import option` / `import json` / `import db` / `import auth` / `import log` / `import http` binds that module's native functions. `print(s: String)` needs no import.
Key stdlib functions: string_concat(a, b), string_split(s, sep) -> List<String>, string_replace(s, target, replacement) -> String, string_url_decode(s) -> String (percent-decoding; compose string_replace(s, "+", " ") first for query-string values), string_length/trim/contains/to_upper/to_lower, math_sqrt/pow/floor/ceil/round/abs/min/max, collections_length(list) -> Int, time_now_seconds() -> Int, http_serve(port: Int) (async - a program using it defines `fn handle_request(method: String, path: String, body: String) -> String` and ends with `await http_serve(port)`).
Testing: `test "name" { ... }` blocks contain `assert condition` statements; `mock function_name -> value` only works inside a test block, for a declared infer/tool.
No list literal can be empty (element type can't be inferred). No Option<T> construction syntax exists - only specific stdlib functions produce one; use an empty-string or sentinel value instead when you need "not found". No Int/String conversion exists.

Respond with ONLY the AINT source code for the requested program, wrapped in a single ```an code fence, and nothing else - no explanation before or after."#;

/// Strips a ` ```an ` / ` ```aint ` / plain ` ``` ` code fence if the
/// model wrapped its answer in one (most do, even when asked not to);
/// otherwise returns the trimmed response as-is.
fn extract_source(response: &str) -> String {
    let trimmed = response.trim();
    let Some(after_open) = trimmed.strip_prefix("```") else {
        return trimmed.to_string();
    };
    let without_lang = ["an\n", "aint\n", "\n"]
        .iter()
        .find_map(|prefix| after_open.strip_prefix(prefix))
        .unwrap_or(after_open);
    let body = without_lang
        .rsplit_once("```")
        .map(|(body, _)| body)
        .unwrap_or(without_lang);
    body.trim().to_string()
}

/// `aint scaffold "description" <path>` (milestone 32): generates a
/// starter AINT project from a plain-English description, using the
/// same OpenAI-compatible backend `aint run` optionally uses via
/// `AINT_MODEL_URL` — required here (not optional), since there's
/// nothing to scaffold without a real model. Refuses to overwrite an
/// existing package, the same rule `aint init` follows. Generated code
/// is always run through the same check `aint check` uses before being
/// reported as done — a program that fails to type-check is still
/// written to disk (so it's there to inspect and fix), but the command
/// exits non-zero and says so, never silently passed off as finished.
fn scaffold(description: &str, path: &Path) -> ExitCode {
    let base_url = match std::env::var("AINT_MODEL_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("error: aint scaffold requires AINT_MODEL_URL to be set");
            return ExitCode::FAILURE;
        }
    };

    let manifest_path = path.join(aint_package::MANIFEST_FILE_NAME);
    if manifest_path.exists() {
        eprintln!("error: {} already exists", manifest_path.display());
        return ExitCode::FAILURE;
    }

    let model_name = std::env::var("AINT_MODEL_NAME").unwrap_or_else(|_| "default".to_string());
    let mut client = aint_runtime::ChatClient::new(base_url, model_name);
    if let Ok(api_key) = std::env::var("AINT_MODEL_API_KEY") {
        client = client.with_api_key(api_key);
    }

    let runtime = match build_tokio_runtime() {
        Ok(runtime) => runtime,
        Err(code) => return code,
    };

    let response = match runtime.block_on(client.complete(SCAFFOLD_SYSTEM_PROMPT, description)) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    let source = extract_source(&response);

    if let Err(err) = fs::create_dir_all(path) {
        eprintln!("error: could not create {}: {err}", path.display());
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
    if let Err(err) = fs::write(&main_an, &source) {
        eprintln!("error: could not write {}: {err}", main_an.display());
        return ExitCode::FAILURE;
    }

    match parse_and_check(&main_an) {
        Ok(_) => {
            println!("created {} — type-checks cleanly", main_an.display());
            ExitCode::SUCCESS
        }
        Err(code) => {
            eprintln!(
                "warning: the generated program at {} does not type-check — left on disk to inspect, not reported as done",
                main_an.display()
            );
            code
        }
    }
}

const RELEASE_REPO: &str = "deaazed/aint";

/// `aint upgrade` (milestone 42). Never runs on its own — only when
/// explicitly invoked — and never touches anything but the running
/// binary itself.
fn upgrade(check_only: bool) -> ExitCode {
    let current = env!("CARGO_PKG_VERSION");

    let client = match reqwest::blocking::Client::builder()
        .user_agent(format!("aint-upgrade/{current}"))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            eprintln!("error: could not build an HTTP client: {err}");
            return ExitCode::FAILURE;
        }
    };

    let tag = match fetch_latest_tag(&client) {
        Ok(tag) => tag,
        Err(err) => {
            eprintln!("error: could not check the latest release: {err}");
            return ExitCode::FAILURE;
        }
    };
    let latest = tag.strip_prefix('v').unwrap_or(&tag);

    // Not just an equality check: a dev build (or one running between
    // this tag's push and its release workflow actually finishing —
    // `releases/latest` still reports the *previous* tag until then)
    // can genuinely be ahead of the last published release. Only ever
    // "upgrade" to something that actually compares newer, falling
    // back to plain inequality if either version doesn't parse as
    // three dot-separated numbers (never expected in practice, but
    // safer than assuming the shape).
    let is_newer = match (parse_semver(current), parse_semver(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => latest != current,
    };
    if !is_newer {
        println!("aint {current} is already the latest version");
        return ExitCode::SUCCESS;
    }

    if check_only {
        println!("aint {current} -> {latest} is available (run `aint upgrade` to install it)");
        return ExitCode::FAILURE;
    }

    let Some(asset) = platform_asset() else {
        eprintln!(
            "error: no prebuilt aint for this platform — build from source instead: https://github.com/{RELEASE_REPO}#building-from-source"
        );
        return ExitCode::FAILURE;
    };

    println!("upgrading aint {current} -> {latest}...");
    match download_and_replace(&client, &tag, asset) {
        Ok(()) => {
            println!("upgraded to aint {latest}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Parses a plain `major.minor.patch` version string (no pre-release
/// or build-metadata suffix — `aint` has never used either) into
/// comparable integers. Small and local rather than pulling in the
/// `semver` crate for one comparison.
fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn fetch_latest_tag(client: &reqwest::blocking::Client) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
    }
    let url = format!("https://api.github.com/repos/{RELEASE_REPO}/releases/latest");
    let release: Release = client
        .get(&url)
        .send()
        .map_err(|err| format!("request to {url} failed: {err}"))?
        .error_for_status()
        .map_err(|err| format!("{url} responded with an error: {err}"))?
        .json()
        .map_err(|err| format!("could not parse the response from {url}: {err}"))?;
    Ok(release.tag_name)
}

/// Mirrors `install.sh`/`install.ps1`'s own OS/arch detection and
/// asset naming exactly — `None` for a platform neither script covers
/// (Linux/aarch64, or anything that isn't Linux/macOS/Windows).
fn platform_asset() -> Option<&'static str> {
    platform_asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn platform_asset_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("aint-windows-x86_64"),
        ("macos", "x86_64") => Some("aint-macos-x86_64"),
        ("macos", "aarch64") => Some("aint-macos-aarch64"),
        ("linux", "x86_64") => Some("aint-linux-x86_64"),
        _ => None,
    }
}

fn archive_extension() -> &'static str {
    if cfg!(windows) {
        "zip"
    } else {
        "tar.gz"
    }
}

fn download_and_replace(
    client: &reqwest::blocking::Client,
    tag: &str,
    asset: &str,
) -> Result<(), String> {
    let ext = archive_extension();
    let url = format!("https://github.com/{RELEASE_REPO}/releases/download/{tag}/{asset}.{ext}");
    let bytes = client
        .get(&url)
        .send()
        .map_err(|err| format!("request to {url} failed: {err}"))?
        .error_for_status()
        .map_err(|err| format!("{url} responded with an error: {err}"))?
        .bytes()
        .map_err(|err| format!("could not read the downloaded archive: {err}"))?;

    let tmp_dir = std::env::temp_dir().join(format!("aint-upgrade-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir)
        .map_err(|err| format!("could not create a temp directory: {err}"))?;
    let archive_path = tmp_dir.join(format!("{asset}.{ext}"));
    fs::write(&archive_path, &bytes)
        .map_err(|err| format!("could not write {}: {err}", archive_path.display()))?;

    let result = extract_binary(&archive_path, &tmp_dir)
        .and_then(|new_binary| replace_running_binary(&new_binary));

    let _ = fs::remove_dir_all(&tmp_dir);
    result
}

/// Shells out to whatever this platform's own install script already
/// assumes is present (`tar` on Unix, PowerShell's `Expand-Archive` on
/// Windows) rather than adding an archive-extraction dependency for
/// one command.
fn extract_binary(archive_path: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    if cfg!(windows) {
        let script = format!(
            "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
            archive_path.display(),
            dest_dir.display()
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status()
            .map_err(|err| format!("could not run powershell: {err}"))?;
        if !status.success() {
            return Err(format!(
                "Expand-Archive of {} failed",
                archive_path.display()
            ));
        }
        Ok(dest_dir.join("aint.exe"))
    } else {
        let status = std::process::Command::new("tar")
            .arg("-xzf")
            .arg(archive_path)
            .arg("-C")
            .arg(dest_dir)
            .status()
            .map_err(|err| format!("could not run tar: {err}"))?;
        if !status.success() {
            return Err(format!("tar could not extract {}", archive_path.display()));
        }
        Ok(dest_dir.join("aint"))
    }
}

/// Replaces the currently-running executable with `new_binary`.
/// Neither OS allows overwriting a running process's own file
/// directly, but both allow *renaming* it — the running process keeps
/// its open handle to the underlying file regardless of what it's
/// currently called, on Unix and Windows alike.
fn replace_running_binary(new_binary: &Path) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|err| format!("could not determine the running binary's own path: {err}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(new_binary)
            .map_err(|err| format!("could not read {}: {err}", new_binary.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(new_binary, perms)
            .map_err(|err| format!("could not make {} executable: {err}", new_binary.display()))?;
        fs::rename(new_binary, &current_exe)
            .map_err(|err| format!("could not replace {}: {err}", current_exe.display()))?;
    }

    #[cfg(windows)]
    {
        // Move the running binary aside first, then move the new one
        // into the name it vacated. A leftover `.old` file from an
        // interrupted previous upgrade is removed first, best-effort;
        // the final cleanup is best-effort too, since Windows may
        // still hold this process's own now-renamed-away file open
        // until it exits.
        let old_path = current_exe.with_extension("exe.old");
        let _ = fs::remove_file(&old_path);
        fs::rename(&current_exe, &old_path)
            .map_err(|err| format!("could not move aside the running binary: {err}"))?;
        if let Err(err) = fs::rename(new_binary, &current_exe) {
            let _ = fs::rename(&old_path, &current_exe);
            return Err(format!("could not install the new binary: {err}"));
        }
        let _ = fs::remove_file(&old_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_source_strips_a_language_tagged_fence() {
        let response = "```an\nprint(\"hi\")\n```";
        assert_eq!(extract_source(response), "print(\"hi\")");
    }

    #[test]
    fn extract_source_strips_a_plain_fence() {
        let response = "```\nprint(\"hi\")\n```";
        assert_eq!(extract_source(response), "print(\"hi\")");
    }

    #[test]
    fn extract_source_passes_through_unfenced_text() {
        let response = "print(\"hi\")";
        assert_eq!(extract_source(response), "print(\"hi\")");
    }

    #[test]
    fn extract_source_strips_surrounding_whitespace() {
        let response = "\n\n  ```an\nprint(\"hi\")\n```\n\n";
        assert_eq!(extract_source(response), "print(\"hi\")");
    }

    #[test]
    fn parse_dotenv_reads_key_value_pairs() {
        let text =
            "AINT_MODEL_URL=https://api.mistral.ai/v1\nAINT_MODEL_NAME=mistral-small-latest\n";
        assert_eq!(
            parse_dotenv(text),
            vec![
                (
                    "AINT_MODEL_URL".to_string(),
                    "https://api.mistral.ai/v1".to_string()
                ),
                (
                    "AINT_MODEL_NAME".to_string(),
                    "mistral-small-latest".to_string()
                ),
            ]
        );
    }

    #[test]
    fn parse_dotenv_skips_blank_lines_and_comments() {
        let text = "\n# a comment\n\nKEY=value\n  # another comment\n";
        assert_eq!(
            parse_dotenv(text),
            vec![("KEY".to_string(), "value".to_string())]
        );
    }

    #[test]
    fn parse_dotenv_trims_whitespace_around_key_and_value() {
        let text = "  KEY  =  value with spaces  \n";
        assert_eq!(
            parse_dotenv(text),
            vec![("KEY".to_string(), "value with spaces".to_string())]
        );
    }

    #[test]
    fn parse_dotenv_splits_only_on_the_first_equals() {
        // A value containing `=` (a URL query string, say) survives
        // intact rather than getting truncated.
        let text = "KEY=a=b=c\n";
        assert_eq!(
            parse_dotenv(text),
            vec![("KEY".to_string(), "a=b=c".to_string())]
        );
    }

    #[test]
    fn parse_dotenv_ignores_a_line_with_no_equals_sign() {
        let text = "not a valid line\nKEY=value\n";
        assert_eq!(
            parse_dotenv(text),
            vec![("KEY".to_string(), "value".to_string())]
        );
    }

    #[test]
    fn parse_dotenv_on_empty_text_yields_nothing() {
        assert_eq!(parse_dotenv(""), Vec::<(String, String)>::new());
    }

    #[test]
    fn parse_semver_reads_major_minor_patch() {
        assert_eq!(parse_semver("0.2.0"), Some((0, 2, 0)));
        assert_eq!(parse_semver("12.34.56"), Some((12, 34, 56)));
    }

    #[test]
    fn parse_semver_rejects_anything_else() {
        assert_eq!(parse_semver("0.2"), None);
        assert_eq!(parse_semver("0.2.0-beta"), None);
        assert_eq!(parse_semver("0.2.0.1"), None);
        assert_eq!(parse_semver("not a version"), None);
    }

    #[test]
    fn parse_semver_orders_correctly() {
        assert!(parse_semver("0.2.0") > parse_semver("0.1.9"));
        assert!(parse_semver("1.0.0") > parse_semver("0.99.99"));
        assert!(parse_semver("0.2.1") > parse_semver("0.2.0"));
        assert_eq!(parse_semver("0.2.0"), parse_semver("0.2.0"));
    }

    #[test]
    fn platform_asset_names_match_install_sh_and_install_ps1_exactly() {
        // The install scripts are the source of truth for these
        // names - this just guards against the two drifting apart.
        assert_eq!(
            platform_asset_for("windows", "x86_64"),
            Some("aint-windows-x86_64")
        );
        assert_eq!(
            platform_asset_for("macos", "x86_64"),
            Some("aint-macos-x86_64")
        );
        assert_eq!(
            platform_asset_for("macos", "aarch64"),
            Some("aint-macos-aarch64")
        );
        assert_eq!(
            platform_asset_for("linux", "x86_64"),
            Some("aint-linux-x86_64")
        );
    }

    #[test]
    fn platform_asset_is_none_for_an_uncovered_platform() {
        // Matches install.sh's own explicit linux/aarch64 rejection.
        assert_eq!(platform_asset_for("linux", "aarch64"), None);
        assert_eq!(platform_asset_for("freebsd", "x86_64"), None);
    }
}
