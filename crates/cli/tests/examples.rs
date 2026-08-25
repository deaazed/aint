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
