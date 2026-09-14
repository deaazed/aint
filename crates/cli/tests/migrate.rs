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
        stderr.contains("no test blocks to verify a rewrite against"),
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
