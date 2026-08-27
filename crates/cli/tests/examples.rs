//! Exercises the actual built `aint` binary, not just library code —
//! `CARGO_BIN_EXE_aint` is a path Cargo provides specifically for this
//! in integration tests. What matters most here is proving that a type
//! error stops the program *before* the interpreter ever runs it, not
//! just that it eventually fails.

use std::process::{Command, Output};

fn example_path(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run_aint(path: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("run")
        .arg(path)
        .output()
        .expect("failed to spawn the aint binary")
}

fn test_aint(path: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("test")
        .arg(path)
        .output()
        .expect("failed to spawn the aint binary")
}

#[test]
fn hello_an_prints_and_exits_zero() {
    let output = run_aint(&example_path("hello.an"));
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello, AINT!\n");
}

#[test]
fn fibonacci_an_prints_and_exits_zero() {
    let output = run_aint(&example_path("fibonacci.an"));
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "55\n");
}

#[test]
fn stdlib_an_prints_and_exits_zero() {
    let output = run_aint(&example_path("stdlib.an"));
    assert!(output.status.success());

    let total: f64 = 4.0 + 8.0 + 15.0 + 16.0 + 23.0 + 42.0;
    let expected = format!("{total}\n{}\nHELLO, AINT\ntrue\n", total.sqrt());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn showcase_an_prints_and_exits_zero() {
    let output = run_aint(&example_path("showcase.an"));
    assert!(output.status.success());

    let expected = "\
11
111
false
3
4
4
2.4
3.7
-2.4
256
16
AINT can already do quite a lot
31
AINT CAN ALREADY DO QUITE A LOT
true
";
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn async_an_prints_and_exits_zero() {
    let output = run_aint(&example_path("async.an"));
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n36\ndone\n");
}

#[test]
fn enums_an_prints_and_exits_zero() {
    let output = run_aint(&example_path("enums.an"));
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "North\nEast\nNorth\ntrue\ntrue\ntrue\ntrue\nfalse\n"
    );
}

#[test]
fn a_type_error_is_rejected_before_anything_runs() {
    let path = std::env::temp_dir().join(format!("aint_cli_type_error_{}.an", std::process::id()));
    std::fs::write(
        &path,
        "fn add(a: Int, b: Int) -> Int { return a + b }\nprint(add(\"hello\", true))\n",
    )
    .expect("failed to write a temporary .an file");

    let output = run_aint(path.to_str().expect("temp path should be utf8"));
    std::fs::remove_file(&path).ok();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "expected no stdout output at all, since the interpreter should never run for an ill-typed program; got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("argument"),
        "expected the argument-type-mismatch message on stderr, got: {stderr}"
    );
}

/// Milestone 08's honest gap, verified through the real binary: `aint
/// run` has no AINT-level way to configure a mock response yet (that's
/// milestone 15), and no real model backend yet (milestone 16) — so
/// awaiting an `infer` call fails clearly instead of guessing an
/// answer. See `docs/milestones/08-first-ai-primitive/SPEC.md`.
#[test]
fn awaiting_an_unconfigured_infer_call_fails_clearly_through_the_real_binary() {
    let path = std::env::temp_dir().join(format!("aint_cli_infer_{}.an", std::process::id()));
    std::fs::write(
        &path,
        "infer is_positive(text: String) -> Bool\n\
         print(await is_positive(\"great product\"))\n",
    )
    .expect("failed to write a temporary .an file");

    let output = run_aint(path.to_str().expect("temp path should be utf8"));
    std::fs::remove_file(&path).ok();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "expected no stdout output, since the model call fails before print runs; got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no mock response configured for `is_positive`"),
        "expected the model-error message on stderr, got: {stderr}"
    );
}

/// Milestone 11's identical gap for `tool` instead of `infer` — see
/// `docs/milestones/11-typed-tools/SPEC.md`.
#[test]
fn awaiting_an_unconfigured_tool_call_fails_clearly_through_the_real_binary() {
    let path = std::env::temp_dir().join(format!("aint_cli_tool_{}.an", std::process::id()));
    std::fs::write(
        &path,
        "tool database_get_email(id: String) -> String\n\
         print(await database_get_email(\"1\"))\n",
    )
    .expect("failed to write a temporary .an file");

    let output = run_aint(path.to_str().expect("temp path should be utf8"));
    std::fs::remove_file(&path).ok();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "expected no stdout output, since the tool call fails before print runs; got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no mock response configured for `database_get_email`"),
        "expected the tool-error message on stderr, got: {stderr}"
    );
}

/// `examples/testing.an` (milestone 15) — the first example able to
/// meaningfully use `infer`/`tool` at all, since `aint test` is
/// finally the AINT-level way to configure what they return. All
/// three of its tests are expected to pass.
#[test]
fn testing_an_all_tests_pass_via_aint_test() {
    let output = test_aint(&example_path("testing.an"));
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("3 run, 3 passed, 0 failed"),
        "expected a passing summary, got: {stdout}"
    );
    assert!(stdout.contains("positive review gets a thank-you"));
    assert!(stdout.contains("negative review gets acknowledged"));
    assert!(stdout.contains("neutral review still gets acknowledged"));
}

/// `test` blocks are inert during `aint run` — the file has no
/// top-level `print`, so running it should produce no output and
/// exit cleanly, same as any other example.
#[test]
fn testing_an_runs_cleanly_via_aint_run_too() {
    let output = run_aint(&example_path("testing.an"));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

/// `examples/security.an` (milestone 20) — a `permissions`-restricted
/// `infer` type-checks and still behaves normally through `aint test`
/// when mocked with a direct answer. The tool-call enforcement itself
/// (rejecting a model-requested call outside `permissions`) has no
/// AINT-source-level way to exercise it: `mock` can only script a
/// direct answer, not a `CallTool` outcome (milestone 15's DSL never
/// grew that), so it's covered by Rust-level interpreter tests
/// instead. See `docs/milestones/20-security-model/SPEC.md`.
#[test]
fn security_an_passes_via_aint_test() {
    let output = test_aint(&example_path("security.an"));
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1 run, 1 passed, 0 failed"),
        "expected a passing summary, got: {stdout}"
    );
}

#[test]
fn security_an_runs_cleanly_via_aint_run_too() {
    let output = run_aint(&example_path("security.an"));
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn aint_test_reports_a_failing_assertion_and_exits_nonzero() {
    let path = std::env::temp_dir().join(format!("aint_cli_test_fail_{}.an", std::process::id()));
    std::fs::write(
        &path,
        "test \"this should fail\" {\n\
             assert 1 == 2\n\
         }\n",
    )
    .expect("failed to write a temporary .an file");

    let output = test_aint(path.to_str().expect("temp path should be utf8"));
    std::fs::remove_file(&path).ok();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("this should fail"));
    assert!(stdout.contains("FAILED"));
    assert!(stdout.contains("1 run, 0 passed, 1 failed"));
}
