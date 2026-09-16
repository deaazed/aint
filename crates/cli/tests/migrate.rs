//! Exercises `aint migrate` against the real binary. The deterministic
//! tier needs no mock server (it's pure AST rewriting); the AI tier
//! uses the same hand-rolled-mock-server technique `scaffold.rs`'s own
//! tests use, proving the real wire protocol and the accept/reject
//! verification gate both work, not a model of them.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};

fn temp_file(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "aint_migrate_test_{name}_{}.an",
        std::process::id()
    ));
    std::fs::write(&path, contents).expect("failed to write a temporary .an file");
    path
}

fn run_migrate(args: &[&str], base_url: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_aint"));
    cmd.arg("migrate").args(args);
    if let Some(url) = base_url {
        cmd.env("AINT_MODEL_URL", url)
            .env("AINT_MODEL_NAME", "test-model");
    } else {
        cmd.env_remove("AINT_MODEL_URL");
    }
    cmd.output().expect("failed to spawn the aint binary")
}

fn run_migrate_in(args: &[&str], cwd: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("migrate")
        .args(args)
        .current_dir(cwd)
        .env_remove("AINT_MODEL_URL")
        .output()
        .expect("failed to spawn the aint binary")
}

#[test]
fn rewrites_an_if_return_else_return_into_a_return_if_expression() {
    let path = temp_file(
        "det",
        "fn sign(n: Int) -> String {\nif n < 0 {\nreturn \"neg\"\n} else {\nreturn \"pos\"\n}\n}\nprint(sign(-1))\n",
    );

    let output = run_migrate(&[path.to_str().unwrap()], None);
    let contents = std::fs::read_to_string(&path).expect("should read the file");
    std::fs::remove_file(&path).ok();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(contents.contains("return if n < 0"), "got: {contents}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 scanned, 1 migrated, 0 unchanged"));
}

#[test]
fn an_already_modern_file_is_reported_unchanged_and_left_untouched() {
    let original = "fn f() -> Int {\nreturn 1\n}\nprint(f())\n";
    let path = temp_file("modern", original);

    let output = run_migrate(&[path.to_str().unwrap()], None);
    let contents = std::fs::read_to_string(&path).expect("should read the file");
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert_eq!(
        contents, original,
        "an already-modern file must not be rewritten"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 scanned, 0 migrated, 1 unchanged"));
}

#[test]
fn check_reports_without_writing_and_exits_nonzero_when_something_would_change() {
    let original =
        "fn f(n: Int) -> Int {\nif n < 0 {\nreturn 0\n} else {\nreturn n\n}\n}\nprint(f(5))\n";
    let path = temp_file("check", original);

    let output = run_migrate(&["--check", path.to_str().unwrap()], None);
    let contents = std::fs::read_to_string(&path).expect("should read the file");
    std::fs::remove_file(&path).ok();

    assert!(
        !output.status.success(),
        "--check should exit non-zero when something would change"
    );
    assert_eq!(contents, original, "--check must never write to the file");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("would change"));
}

#[test]
fn migrate_ai_without_aint_model_url_fails_clearly() {
    let path = temp_file("noairl", "print(1)\n");
    let output = run_migrate(&["--ai", path.to_str().unwrap()], None);
    std::fs::remove_file(&path).ok();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("AINT_MODEL_URL"));
}

#[test]
fn a_file_with_no_test_blocks_skips_the_ai_tier_without_calling_the_model() {
    // The mock server accepts zero connections and would hang forever
    // if asked to - if the AI tier ever called it, this test would
    // time out instead of finishing quickly, which is itself the proof
    // no call was made.
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind a local port");
    let addr = listener.local_addr().expect("failed to read local addr");
    let base_url = format!("http://{addr}");
    std::mem::forget(listener); // never accept, never respond

    let original = "print(1)\n";
    let path = temp_file("noaitests", original);

    let output = run_migrate(&["--ai", path.to_str().unwrap()], Some(&base_url));
    let contents = std::fs::read_to_string(&path).expect("should read the file");
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert_eq!(contents, original);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no test blocks") && stderr.contains("to verify a rewrite against"),
        "stderr: {stderr}"
    );
}

fn start_mock_server(raw_response: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind a local port");
    let addr = listener.local_addr().expect("failed to read local addr");
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 65536];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(raw_response.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://{addr}")
}

fn http_ok(json_body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        json_body.len(),
        json_body
    )
}

/// Serves one response per connection from `raw_responses`, in order —
/// for a scenario where more than one file is independently eligible
/// for AI-assisted migration in the same run (each gets its own call).
fn start_mock_server_sequence(raw_responses: Vec<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind a local port");
    let addr = listener.local_addr().expect("failed to read local addr");
    std::thread::spawn(move || {
        for raw_response in raw_responses {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 65536];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(raw_response.as_bytes());
                let _ = stream.flush();
            }
        }
    });
    format!("http://{addr}")
}

fn an_response(source: &str) -> String {
    let escaped = source
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!(r#"{{"choices":[{{"message":{{"content":"```an\n{escaped}```"}}}}]}}"#)
}

#[test]
fn a_well_typed_behavior_preserving_ai_proposal_is_accepted() {
    let original = "fn double(n: Int) -> Int {\nreturn n * 2\n}\ntest \"doubles\" {\nassert double(3) == 6\n}\n";
    let path = temp_file("aiok", original);

    // The "proposal" is behaviorally identical (just a cosmetic rename
    // of the parameter) - it must type-check and pass the same test.
    let proposal = "fn double(x: Int) -> Int {\nreturn x * 2\n}\ntest \"doubles\" {\nassert double(3) == 6\n}\n";
    let escaped = proposal
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let content = format!(r#"{{"choices":[{{"message":{{"content":"```an\n{escaped}```"}}}}]}}"#);
    let base_url = start_mock_server(http_ok(&content));

    let output = run_migrate(&["--ai", path.to_str().unwrap()], Some(&base_url));
    let contents = std::fs::read_to_string(&path).expect("should read the file");
    std::fs::remove_file(&path).ok();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(contents.contains("fn double(x: Int)"), "got: {contents}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("AI-assisted modernization applied"),
        "stdout: {stdout}"
    );
}

#[test]
fn an_ai_proposal_that_breaks_a_test_is_rejected_and_the_file_reverts() {
    let original = "fn double(n: Int) -> Int {\nreturn n * 2\n}\ntest \"doubles\" {\nassert double(3) == 6\n}\n";
    let path = temp_file("aibad", original);

    // A real, well-typed, but behaviorally *wrong* proposal (off-by-one)
    // - must be rejected because the test would then fail.
    let proposal = "fn double(n: Int) -> Int {\nreturn n * 2 + 1\n}\ntest \"doubles\" {\nassert double(3) == 6\n}\n";
    let escaped = proposal
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let content = format!(r#"{{"choices":[{{"message":{{"content":"```an\n{escaped}```"}}}}]}}"#);
    let base_url = start_mock_server(http_ok(&content));

    let output = run_migrate(&["--ai", path.to_str().unwrap()], Some(&base_url));
    let contents = std::fs::read_to_string(&path).expect("should read the file");
    std::fs::remove_file(&path).ok();

    assert!(
        output.status.success(),
        "a rejected AI proposal isn't itself a hard failure"
    );
    assert_eq!(
        contents, original,
        "a behavior-changing proposal must be reverted"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rejected") || stderr.contains("changed at least one test"),
        "stderr: {stderr}"
    );
}

#[test]
fn migrating_a_directory_walks_every_an_file_recursively() {
    let dir = std::env::temp_dir().join(format!("aint_migrate_dir_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(
        dir.join("a.an"),
        "fn f() -> Int {\nif true {\nreturn 1\n} else {\nreturn 2\n}\n}\nprint(f())\n",
    )
    .unwrap();
    std::fs::write(dir.join("sub").join("b.an"), "print(1)\n").unwrap();

    let output = run_migrate(&[dir.to_str().unwrap()], None);
    let a_contents = std::fs::read_to_string(dir.join("a.an")).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    assert!(output.status.success());
    assert!(a_contents.contains("return if true"), "got: {a_contents}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2 scanned, 1 migrated, 1 unchanged"));
}

/// `aint-loader` forbids a `test` block in any file reached through
/// `import "..." as ...`, so a shared library file can never carry its
/// own — real coverage for one only ever lives in a companion file
/// that imports it and tests it externally (the same shape
/// `examples/router/router_test.an` already uses in the real project).
/// A library file with no tests of its own but a tested companion must
/// still be attempted, not skipped.
fn library_and_companion_test(dir: &std::path::Path, lib_body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("lib.an"), lib_body).unwrap();
    std::fs::write(
        dir.join("caller.an"),
        "import \"./lib.an\" as lib\ntest \"uses double\" {\nassert lib_double(3) == 6\n}\n",
    )
    .unwrap();
}

#[test]
fn a_library_file_with_no_tests_of_its_own_is_still_eligible_via_a_companion_test_file() {
    let dir =
        std::env::temp_dir().join(format!("aint_migrate_companion_ok_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let lib_original = "fn double(n: Int) -> Int {\nreturn n * 2\n}\n";
    library_and_companion_test(&dir, lib_original);

    // caller.an is scanned first (alphabetical) and has its own test
    // block, so it gets its own AI attempt too - answered with itself,
    // verbatim, so it's a behavioral no-op and the interesting proposal
    // (for lib.an) is the second response served.
    let caller_verbatim = std::fs::read_to_string(dir.join("caller.an")).unwrap();
    let lib_proposal = "fn double(x: Int) -> Int {\nreturn x * 2\n}\n"; // cosmetic rename only
    let base_url = start_mock_server_sequence(vec![
        http_ok(&an_response(&caller_verbatim)),
        http_ok(&an_response(lib_proposal)),
    ]);

    let output = run_migrate(&["--ai", dir.to_str().unwrap()], Some(&base_url));
    let lib_contents = std::fs::read_to_string(dir.join("lib.an")).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        lib_contents.contains("fn double(x: Int)"),
        "lib.an should have been eligible and accepted via its companion test file, got: {lib_contents}"
    );
}

#[test]
fn an_ai_proposal_that_breaks_an_importer_is_rejected_even_though_that_files_own_check_passed() {
    let dir =
        std::env::temp_dir().join(format!("aint_migrate_companion_bad_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let lib_original = "fn double(n: Int) -> Int {\nreturn n * 2\n}\n";
    library_and_companion_test(&dir, lib_original);

    // The proposal for lib.an keeps the same name/signature (so lib.an
    // alone, which has no tests of its own, has nothing to catch this)
    // but changes the multiplier - caller.an's test (untouched, still
    // asserting the *old* behavior) must catch it via the whole-batch
    // check.
    let caller_verbatim = std::fs::read_to_string(dir.join("caller.an")).unwrap();
    let lib_bad_proposal = "fn double(n: Int) -> Int {\nreturn n * 3\n}\n";
    let base_url = start_mock_server_sequence(vec![
        http_ok(&an_response(&caller_verbatim)),
        http_ok(&an_response(lib_bad_proposal)),
    ]);

    let output = run_migrate(&["--ai", dir.to_str().unwrap()], Some(&base_url));
    let lib_contents = std::fs::read_to_string(dir.join("lib.an")).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        output.status.success(),
        "a rejected AI proposal isn't itself a hard failure"
    );
    assert_eq!(
        lib_contents, lib_original,
        "lib.an must revert to its original content once caller.an's test would break"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("caller.an") || stderr.contains("broke"),
        "expected the rejection reason to name the broken importer, stderr: {stderr}"
    );
}

#[test]
fn project_finds_the_nearest_aint_toml_and_migrates_everything_under_it() {
    let dir = std::env::temp_dir().join(format!("aint_migrate_project_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("aint.toml"),
        "[package]\nname = \"p\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("a.an"),
        "fn f() -> Int {\nif true {\nreturn 1\n} else {\nreturn 2\n}\n}\nprint(f())\n",
    )
    .unwrap();

    // Run from a subdirectory with no aint.toml of its own - --project
    // must walk up to find it, the same way a package import resolves.
    let output = run_migrate_in(&["--project"], &dir.join("src"));
    let contents = std::fs::read_to_string(dir.join("src").join("a.an")).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(contents.contains("return if true"), "got: {contents}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 scanned, 1 migrated, 0 unchanged"));
}

#[test]
fn project_with_no_aint_toml_anywhere_fails_clearly() {
    let dir = std::env::temp_dir().join(format!("aint_migrate_no_project_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.an"), "print(1)\n").unwrap();

    // A bare temp directory has no aint.toml anywhere above it either
    // (assuming the OS temp root itself doesn't), so this should fail
    // rather than accidentally walking up into an unrelated project.
    let output = run_migrate_in(&["--project"], &dir);
    std::fs::remove_dir_all(&dir).ok();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("aint.toml"), "stderr: {stderr}");
}

#[test]
fn project_and_an_explicit_path_are_mutually_exclusive() {
    let path = temp_file("projectconflict", "print(1)\n");
    let output = run_migrate(&["--project", path.to_str().unwrap()], None);
    std::fs::remove_file(&path).ok();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--project") && stderr.to_lowercase().contains("path"),
        "expected clap's conflicts_with error naming both, stderr: {stderr}"
    );
}

#[test]
fn migrate_with_neither_a_path_nor_project_fails_clearly() {
    let output = run_migrate(&[], None);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("path") || stderr.contains("required"),
        "stderr: {stderr}"
    );
}
