//! `aint migrate` (milestone 45): rewrites a project to the latest
//! idiomatic AINT syntax. Two tiers, both narrated, never silent about
//! what changed or why something was left alone:
//!
//! - **Deterministic** (`aint_migrate::migrate_deterministic`, always
//!   applied): a small set of AST rewrites that are provably
//!   behavior-preserving by construction — see that crate's own doc
//!   comment for the two patterns and why each is safe. No AI, no
//!   network call, no quota dependency at all. Verified again anyway,
//!   not just trusted: the rewritten file is re-parsed and re-typechecked
//!   before anything is kept, reverting to the original on the (never
//!   expected) chance the transform itself has a bug.
//! - **AI-assisted** (`--ai`, opt-in): asks the configured model to
//!   propose a broader modernization of the whole file (the kind of
//!   rewrite — hand-built HTML strings becoming `Node` literals, say —
//!   that needs real understanding of intent, not a mechanical pattern
//!   match). Deliberately scoped to files with at least one `test`
//!   block: that's the only case where a real before/after behavioral
//!   oracle exists (this file's own tests, re-run and compared
//!   outcome-for-outcome). A file with no tests is reported as skipped
//!   for this tier, not silently left as-is with no explanation, and
//!   never sent to the model at all (saving a call against whatever
//!   quota is in play). A proposal that doesn't parse, doesn't
//!   type-check, or changes even one test's outcome is discarded — the
//!   file is reverted to what it was before the attempt, never left in
//!   a partially-applied state.
//!
//! See `docs/milestones/45-migrate/SPEC.md`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aint_ast::StmtKind;

use crate::ui::{error_line, fail_line, ok_line, step, warn_line};
use crate::{build_tokio_runtime, extract_source};

pub fn migrate(path: Option<PathBuf>, project: bool, ai: bool, check_only: bool) -> ExitCode {
    let target = match resolve_target(path, project) {
        Ok(target) => target,
        Err(message) => {
            error_line(format!("error: {message}"));
            return ExitCode::FAILURE;
        }
    };

    let files = match collect_an_files(&target) {
        Ok(files) => files,
        Err(err) => {
            error_line(format!("error: {err}"));
            return ExitCode::FAILURE;
        }
    };
    if files.is_empty() {
        warn_line(format!("no .an files found under {}", target.display()));
        return ExitCode::SUCCESS;
    }

    if ai && check_only {
        warn_line("--check skips AI-assisted migration: verifying a proposal requires writing it");
    }

    let ai_client = if ai && !check_only {
        match std::env::var("AINT_MODEL_URL") {
            Ok(base_url) => {
                let model_name =
                    std::env::var("AINT_MODEL_NAME").unwrap_or_else(|_| "default".to_string());
                let mut client = aint_runtime::ChatClient::new(base_url, model_name);
                if let Ok(api_key) = std::env::var("AINT_MODEL_API_KEY") {
                    client = client.with_api_key(api_key);
                }
                Some(client)
            }
            Err(_) => {
                error_line("error: aint migrate --ai requires AINT_MODEL_URL to be set");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };

    let runtime = if ai_client.is_some() {
        match build_tokio_runtime() {
            Ok(runtime) => Some(runtime),
            Err(code) => return code,
        }
    } else {
        None
    };

    // Captured once, from every file's pre-migration content, before
    // any write anywhere in the batch — the cross-file regression
    // oracle `attempt_ai_migration` checks every other file against
    // after tentatively accepting a proposal. Tier 1 doesn't need this
    // (it never changes a signature, so a caller's type-checking can't
    // be affected — see `crates/migrate`'s own doc comment); Tier 2
    // does, since an AI proposal could otherwise change a function's
    // signature, pass its own file's check, and silently break every
    // importer elsewhere in the project. See
    // `docs/milestones/45-migrate/SPEC.md`.
    let batch_baselines: HashMap<PathBuf, Result<TestOutcomes, String>> =
        if let Some(runtime) = &runtime {
            step("establishing a whole-project baseline for AI-assisted verification".to_string());
            files
                .iter()
                .map(|f| (f.clone(), capture_test_outcomes(f, runtime)))
                .collect()
        } else {
            HashMap::new()
        };

    let mut changed = 0usize;
    let mut unchanged = 0usize;
    let mut bugs = 0usize;

    for file in &files {
        step(format!("scanning {}", file.display()));
        let (outcome, notes) = migrate_file(
            file,
            check_only,
            ai_client.as_ref().zip(runtime.as_ref()),
            &files,
            &batch_baselines,
        );
        for note in &notes {
            warn_line(format!("  {}: {note}", file.display()));
        }
        match outcome {
            FileOutcome::Unchanged => unchanged += 1,
            FileOutcome::Changed(descriptions) => {
                changed += 1;
                let verb = if check_only {
                    "would change"
                } else {
                    "migrated"
                };
                ok_line(format!("{} {verb}", file.display()));
                for description in descriptions {
                    ok_line(format!("  - {description}"));
                }
            }
            FileOutcome::TransformBug(reason) => {
                bugs += 1;
                error_line(format!(
                    "error: {}: {reason} (this is a bug in aint-migrate, please report it)",
                    file.display()
                ));
            }
        }
    }

    let summary = format!(
        "{} scanned, {changed} {}, {unchanged} unchanged",
        files.len(),
        if check_only {
            "would change"
        } else {
            "migrated"
        }
    );
    if bugs > 0 {
        fail_line(format!("{summary}, {bugs} failed"));
        ExitCode::FAILURE
    } else if check_only && changed > 0 {
        fail_line(summary);
        ExitCode::FAILURE
    } else {
        ok_line(summary);
        ExitCode::SUCCESS
    }
}

enum FileOutcome {
    Unchanged,
    /// Written (or, under `--check`, would be) — one line per applied
    /// change, deterministic and/or the AI-assisted note.
    Changed(Vec<String>),
    /// The deterministic rewrite itself produced something that no
    /// longer parses/type-checks — never expected, reverted
    /// immediately, reported loudly rather than silently kept.
    TransformBug(String),
}

/// Migrates one file, returning what happened plus any informational
/// notes (a skipped AI attempt, a rejected proposal) worth surfacing
/// regardless of the overall outcome.
fn migrate_file(
    path: &Path,
    check_only: bool,
    ai: Option<(&aint_runtime::ChatClient, &tokio::runtime::Runtime)>,
    files: &[PathBuf],
    batch_baselines: &HashMap<PathBuf, Result<TestOutcomes, String>>,
) -> (FileOutcome, Vec<String>) {
    let mut notes = Vec::new();
    let mut descriptions = Vec::new();
    let mut wrote_anything = false;

    let original_text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            notes.push(format!("could not read: {err}"));
            return (FileOutcome::Unchanged, notes);
        }
    };

    let original_program = match aint_parser::parse_source(&original_text) {
        Ok(program) => program,
        Err(err) => {
            notes.push(format!("does not parse, left untouched: {err}"));
            return (FileOutcome::Unchanged, notes);
        }
    };

    let (migrated_program, log) = aint_migrate::migrate_deterministic(original_program);
    let mut current_text = original_text.clone();

    if !log.is_empty() {
        let formatted = aint_fmt::format_program(&migrated_program);
        if aint_parser::parse_source(&formatted).is_err() {
            return (
                FileOutcome::TransformBug("the deterministic rewrite no longer parses".to_string()),
                notes,
            );
        }

        if check_only {
            for migration in &log {
                descriptions.push(migration.description.clone());
            }
            return (FileOutcome::Changed(descriptions), notes);
        }

        if let Err(err) = fs::write(path, &formatted) {
            notes.push(format!("could not write: {err}"));
            return (FileOutcome::Unchanged, notes);
        }
        if let Err(reason) = verify_type_checks(path) {
            let _ = fs::write(path, &original_text);
            return (
                FileOutcome::TransformBug(format!(
                    "the deterministic rewrite no longer type-checks ({reason})"
                )),
                notes,
            );
        }
        current_text = formatted;
        wrote_anything = true;
        for migration in &log {
            descriptions.push(migration.description.clone());
        }
    }

    if let Some((client, runtime)) = ai {
        if !has_verifiable_coverage(path, &current_text, files) {
            notes.push(
                "skipped AI-assisted modernization: no test blocks (in this file or a companion \
                 file that imports it) to verify a rewrite against"
                    .to_string(),
            );
        } else {
            match attempt_ai_migration(path, &current_text, client, runtime, files, batch_baselines)
            {
                AiOutcome::Accepted => {
                    descriptions.push(
                        "AI-assisted modernization applied, verified against this file's own tests"
                            .to_string(),
                    );
                    wrote_anything = true;
                }
                AiOutcome::Rejected(reason) => {
                    notes.push(format!("AI-assisted modernization rejected: {reason}"));
                }
            }
        }
    }

    if wrote_anything {
        (FileOutcome::Changed(descriptions), notes)
    } else {
        (FileOutcome::Unchanged, notes)
    }
}

/// Whether *some* file in the batch gives `attempt_ai_migration` a
/// real oracle to check a proposal for `path` against: `path`'s own
/// test blocks, if it has any, or — since `aint-loader` forbids a
/// `test` block in any file reached through `import "..." as ...`, so
/// a shared library file can never have its own — a companion file in
/// the batch that imports `path` and has test blocks of its own (the
/// same shape `examples/router/router_test.an`/`examples/
/// customer_support/priority_logic_test.an` already use in this
/// project). The actual before/after comparison for that companion
/// file happens in `verify_batch_unaffected`, not here — this only
/// decides whether attempting AI migration is safe to try at all.
fn has_verifiable_coverage(path: &Path, current_text: &str, files: &[PathBuf]) -> bool {
    if has_test_block(current_text) {
        return true;
    }
    files.iter().any(|other| {
        other != path
            && imports_file(other, path)
            && fs::read_to_string(other)
                .map(|text| has_test_block(&text))
                .unwrap_or(false)
    })
}

fn has_test_block(text: &str) -> bool {
    aint_parser::parse_source(text)
        .map(|program| {
            program
                .statements
                .iter()
                .any(|stmt| matches!(stmt.kind, StmtKind::Test { .. }))
        })
        .unwrap_or(false)
}

/// Whether `candidate`'s own `import "..." as alias` statements
/// (resolved relative to `candidate`'s directory, the same way
/// `aint-loader` resolves them) reach `target`.
fn imports_file(candidate: &Path, target: &Path) -> bool {
    let Ok(text) = fs::read_to_string(candidate) else {
        return false;
    };
    let Ok(program) = aint_parser::parse_source(&text) else {
        return false;
    };
    let Some(dir) = candidate.parent() else {
        return false;
    };
    program.statements.iter().any(|stmt| match &stmt.kind {
        StmtKind::ImportFile { path, .. } => same_file(&dir.join(path), target),
        _ => false,
    })
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

enum AiOutcome {
    Accepted,
    Rejected(String),
}

/// Asks the model to modernize `current_text`, and only ever keeps the
/// result if it parses, type-checks, this file's own tests produce
/// exactly the same pass/fail outcomes before and after, *and* no
/// other file in `files` regresses — reverting to `current_text` the
/// instant any of those doesn't hold. The whole-batch check matters
/// specifically because a proposal could change a function's
/// signature, pass its own file's check, and silently break an
/// importer elsewhere in the project; `batch_baselines` (captured once,
/// before any write in this run) is what catches that.
fn attempt_ai_migration(
    path: &Path,
    current_text: &str,
    client: &aint_runtime::ChatClient,
    runtime: &tokio::runtime::Runtime,
    files: &[PathBuf],
    batch_baselines: &HashMap<PathBuf, Result<TestOutcomes, String>>,
) -> AiOutcome {
    let Some(baseline) = batch_baselines.get(path) else {
        return AiOutcome::Rejected("no baseline was established for this file".to_string());
    };
    let baseline = match baseline {
        Ok(outcomes) => outcomes,
        Err(reason) => {
            return AiOutcome::Rejected(format!("could not establish a baseline: {reason}"))
        }
    };

    let response = match runtime.block_on(client.complete(MIGRATE_SYSTEM_PROMPT, current_text)) {
        Ok(text) => text,
        Err(err) => return AiOutcome::Rejected(format!("model call failed: {err}")),
    };
    let proposal = extract_source(&response);

    if aint_parser::parse_source(&proposal).is_err() {
        return AiOutcome::Rejected("the proposal did not parse".to_string());
    }

    if let Err(err) = fs::write(path, &proposal) {
        return AiOutcome::Rejected(format!("could not write the proposal: {err}"));
    }

    let verdict = verify_type_checks(path)
        .and_then(|()| {
            let after = capture_test_outcomes(path, runtime)
                .map_err(|reason| format!("could not re-run tests: {reason}"))?;
            if &after == baseline {
                Ok(())
            } else {
                Err("it changed at least one test's outcome".to_string())
            }
        })
        .and_then(|()| verify_batch_unaffected(path, files, batch_baselines, runtime));

    match verdict {
        Ok(()) => AiOutcome::Accepted,
        Err(reason) => {
            let _ = fs::write(path, current_text);
            AiOutcome::Rejected(reason)
        }
    }
}

/// Re-verifies every file in the batch *other than* `changed_path`
/// against its own pre-migration baseline — the check that makes it
/// safe to `--ai`-migrate a file other files import, by catching a
/// signature change (or any other cross-file behavior change) the
/// changed file's own verification can't see.
fn verify_batch_unaffected(
    changed_path: &Path,
    files: &[PathBuf],
    batch_baselines: &HashMap<PathBuf, Result<TestOutcomes, String>>,
    runtime: &tokio::runtime::Runtime,
) -> Result<(), String> {
    for other in files {
        if other == changed_path {
            continue;
        }
        let Some(baseline) = batch_baselines.get(other) else {
            continue;
        };
        let now = capture_test_outcomes(other, runtime);
        if &now != baseline {
            return Err(format!(
                "it broke {} (was {}, now {})",
                other.display(),
                describe_baseline(baseline),
                describe_baseline(&now)
            ));
        }
    }
    Ok(())
}

fn describe_baseline(result: &Result<TestOutcomes, String>) -> String {
    match result {
        Ok(outcomes) if outcomes.is_empty() => "type-checking cleanly".to_string(),
        Ok(outcomes) => {
            let failed = outcomes.iter().filter(|(_, r)| r.is_err()).count();
            format!("{} test(s) with {failed} failing", outcomes.len())
        }
        Err(reason) => format!("not type-checking ({reason})"),
    }
}

/// Re-reads `path` from disk through the same loader/typechecker
/// pipeline `aint check` uses — correctly resolves whatever cross-file
/// imports the file has, so this is a real check, not just "does this
/// one file's text parse in isolation."
fn verify_type_checks(path: &Path) -> Result<(), String> {
    let program = aint_loader::load(path).map_err(|err| err.to_string())?;
    aint_typechecker::check_program(&program).map_err(|err| err.to_string())
}

/// `(test name, Ok(()) or the error text)` for every `test` block in a
/// file, in file order — the comparable shape `attempt_ai_migration`
/// diffs before vs. after.
type TestOutcomes = Vec<(String, Result<(), String>)>;

/// Fails outright (rather than returning an empty list) if the file
/// doesn't even type-check, since "no test blocks ran" and "this file
/// is broken" must never look the same.
fn capture_test_outcomes(
    path: &Path,
    runtime: &tokio::runtime::Runtime,
) -> Result<TestOutcomes, String> {
    let program = aint_loader::load(path).map_err(|err| err.to_string())?;
    aint_typechecker::check_program(&program).map_err(|err| err.to_string())?;
    let outcomes = runtime.block_on(aint_runtime::run_tests(&program));
    Ok(outcomes
        .into_iter()
        .map(|outcome| (outcome.name, outcome.result.map_err(|err| err.to_string())))
        .collect())
}

/// `--project` and an explicit `path` are mutually exclusive at the
/// `clap` level already (`conflicts_with`); this handles the two ways
/// *neither* was given a usable value. `--project` reuses
/// `aint_loader::find_package_root` — the exact "walk up looking for
/// `aint.toml`" convention a package import already resolves against,
/// rather than a second, possibly-diverging implementation.
fn resolve_target(path: Option<PathBuf>, project: bool) -> Result<PathBuf, String> {
    if project {
        let cwd = std::env::current_dir()
            .map_err(|err| format!("could not read the current directory: {err}"))?;
        let root = aint_loader::find_package_root(&cwd).ok_or_else(|| {
            format!(
                "--project found no {} in {} or any parent directory — run `aint init` first, \
                 or pass an explicit path instead",
                aint_package::MANIFEST_FILE_NAME,
                cwd.display()
            )
        })?;
        return Ok(strip_verbatim_prefix(root));
    }
    path.ok_or_else(|| "aint migrate needs either a path or --project".to_string())
}

/// `find_package_root` canonicalizes, which on Windows produces a
/// `\\?\`-prefixed "verbatim" path — correct and fully usable, but ugly
/// in every narration line this prints from here on. Every other path
/// `aint migrate` prints (an explicit `path` argument, or a file found
/// by walking) is whatever the user actually typed, so `--project`
/// matches that instead of standing out as the one oddly-formatted one.
/// Purely cosmetic: stripped only for display/further joining, not
/// applied anywhere the extended-length guarantee might matter.
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    match path.to_str() {
        Some(s) if s.starts_with(r"\\?\") => PathBuf::from(&s[4..]),
        _ => path,
    }
}

/// Every `.an` file under `path`, recursively if it's a directory —
/// `target`/`.git`/hidden directories excluded, the same set `aint
/// fmt`'s own example-walking would want, so a build directory or
/// vendored dependency copy is never rewritten by accident.
fn collect_an_files(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(format!("{} does not exist", path.display()));
    }
    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            fs::read_dir(&dir).map_err(|err| format!("could not read {}: {err}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("could not read {}: {err}", dir.display()))?;
            let entry_path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry_path.is_dir() {
                if name.starts_with('.') || name == "target" {
                    continue;
                }
                stack.push(entry_path);
            } else if entry_path.extension().is_some_and(|ext| ext == "an") {
                files.push(entry_path);
            }
        }
    }
    files.sort();
    Ok(files)
}

const MIGRATE_SYSTEM_PROMPT: &str = r#"You modernize existing AINT source code to use the latest idiomatic syntax, preserving its behavior exactly. AINT is a statically-typed language. Use ONLY the syntax listed here - never invent syntax that doesn't exist.

Types: Int, Float, Bool, String, Unit, List<T>, Option<T>, Node, and user-declared enums.
Bindings: `let name = expr` - one-time only, there is no reassignment and no loops anywhere. Iteration is recursion.
Functions: `fn name(param: Type, ...) -> ReturnType { ... }`, optionally `async fn`. An optional `effects [pure]` clause marks a function as calling nothing beyond other pure/stdlib functions.
Closures: `fn(param: Type) -> ReturnType { ... }` as an expression (no name) is a closure value; its type is written `fn(Type, Type) -> ReturnType`.
Control flow: `if condition { ... } else { ... }`; `else if` chains normally. There is no `while`/`for`. Operators: `+ - * / == != < > <= >= && || !`. `if condition { value } else { value }` is also usable directly as an expression (each branch exactly one expression, else required) - prefer this over a function whose body is only `if cond { return a } else { return b }`.
Enums: `enum Name { Variant1 Variant2 }` - a variant value is `Name_Variant1` (one identifier).
Node literals (UI, milestone 44): `Role { name: expr ... expr ... }` builds a `Node` value - `Role` is any short tag naming a kind of UI element (Heading, Paragraph, Group, Button, Link, List, Image, Text, or one you choose); each item is either `name: expr` (a prop, must be a String) or a bare expr (a child - must be Node, String, or List<Node>, spliced in as multiple children); no commas between items. `import ui` provides `render_html(node: Node) -> String`. An `infer`/`tool` declaration can return `Node` too. Only migrate a function to Node literals if it is CLEARLY hand-building an HTML string (nested string_concat/join_lines calls producing tags like <div>, <h1>, <p>, <a>, <ul>, <button>) - never introduce Node literals into code that isn't already building markup.
Cross-file imports: `import "./other.an" as alias` - leave these exactly as they are, never rewrite the path or alias.
Stdlib imports: `import math` / `string` / `time` / `collections` / `distribution` / `option` / `json` / `db` / `auth` / `log` / `http` / `ui` bind that module's native functions. `print(s: String)` needs no import.
Key stdlib functions: string_concat, string_split, string_replace(s, target, replacement), string_url_decode, string_length/trim/contains/to_upper/to_lower, math_sqrt/pow/floor/ceil/round/abs/min/max, collections_length, time_now_seconds, http_serve(port), render_html(node).
Testing: `test "name" { ... }` blocks contain `assert condition` statements; `mock function_name -> value` only works inside a test block, for a declared infer/tool - `value` may be a literal, an EnumName_Variant, or a node literal built from those.

Rules for this task specifically:
- Preserve behavior exactly - every function's signature, every test's expected outcome, every side effect must be identical after your rewrite.
- Only apply a rewrite where the older code is CLEARLY expressing something the newer syntax says more directly. Do not restructure code that's already idiomatic just to change something.
- Never remove or alter a `test`/`assert`/`mock` statement's meaning.
- Respond with ONLY the complete, modernized AINT source for the whole file, wrapped in a single ```an code fence, and nothing else - no explanation before or after."#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_an_files_finds_every_an_file_recursively() {
        let dir = std::env::temp_dir().join(format!("aint_migrate_collect_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.an"), "print(1)").unwrap();
        fs::write(dir.join("sub").join("b.an"), "print(2)").unwrap();
        fs::write(dir.join("not_an_file.txt"), "x").unwrap();

        let files = collect_an_files(&dir).unwrap();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.extension().unwrap() == "an"));
    }

    #[test]
    fn collect_an_files_on_a_single_file_returns_just_that_file() {
        let path =
            std::env::temp_dir().join(format!("aint_migrate_single_{}.an", std::process::id()));
        fs::write(&path, "print(1)").unwrap();
        let files = collect_an_files(&path).unwrap();
        fs::remove_file(&path).ok();
        assert_eq!(files, vec![path]);
    }
}
