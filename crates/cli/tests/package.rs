//! Exercises `aint init`/`aint add` through the real built binary,
//! against real temporary directories on disk — dependency resolution
//! genuinely reads the filesystem (milestone 23), so there's no way
//! to test the CLI path without real directories to point it at.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "aint_cli_package_test_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("should create the test workspace");
    dir
}

fn run_aint_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aint"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to spawn the aint binary")
}

#[test]
fn init_creates_a_manifest_and_a_starter_file() {
    let dir = workspace("init_creates");
    let output = run_aint_in(&dir, &["init", "."]);
    assert!(output.status.success());

    let manifest = fs::read_to_string(dir.join("aint.toml")).expect("aint.toml should exist");
    assert!(manifest.contains("[package]"));
    assert!(manifest.contains("version = \"0.1.0\""));

    let main_an = fs::read_to_string(dir.join("main.an")).expect("main.an should exist");
    assert!(main_an.contains("print("));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn init_refuses_to_overwrite_an_existing_manifest() {
    let dir = workspace("init_refuses_overwrite");
    assert!(run_aint_in(&dir, &["init", "."]).status.success());

    let second = run_aint_in(&dir, &["init", "."]);
    assert!(!second.status.success());
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("already exists"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn add_records_a_path_dependency_and_writes_a_lockfile() {
    let root = workspace("add_records");
    let dep_dir = root.join("some-lib");
    let app_dir = root.join("app");
    fs::create_dir_all(&dep_dir).expect("create dep dir");
    fs::create_dir_all(&app_dir).expect("create app dir");

    assert!(run_aint_in(&dep_dir, &["init", "."]).status.success());
    assert!(run_aint_in(&app_dir, &["init", "."]).status.success());

    let add_output = run_aint_in(&app_dir, &["add", "../some-lib"]);
    assert!(
        add_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    let manifest = fs::read_to_string(app_dir.join("aint.toml")).expect("manifest should exist");
    assert!(manifest.contains("[dependencies.some-lib]"));
    assert!(manifest.contains("../some-lib"));

    let lockfile = fs::read_to_string(app_dir.join("aint.lock")).expect("lockfile should exist");
    assert!(lockfile.contains("name = \"app\""));
    assert!(lockfile.contains("name = \"some-lib\""));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn add_without_a_manifest_fails_clearly() {
    let root = workspace("add_without_manifest");
    let dep_dir = root.join("some-lib");
    let no_manifest_dir = root.join("no-manifest-here");
    fs::create_dir_all(&dep_dir).expect("create dep dir");
    fs::create_dir_all(&no_manifest_dir).expect("create dir with no manifest");
    assert!(run_aint_in(&dep_dir, &["init", "."]).status.success());

    let output = run_aint_in(&no_manifest_dir, &["add", "../some-lib"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("aint init"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn add_rejects_a_cyclic_dependency() {
    let root = workspace("add_cycle");
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    fs::create_dir_all(&a_dir).expect("create a");
    fs::create_dir_all(&b_dir).expect("create b");
    assert!(run_aint_in(&a_dir, &["init", "."]).status.success());
    assert!(run_aint_in(&b_dir, &["init", "."]).status.success());

    assert!(run_aint_in(&b_dir, &["add", "../a"]).status.success());
    let output = run_aint_in(&a_dir, &["add", "../b"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cyclic"),
        "expected a cycle error, got: {stderr}"
    );

    let _ = fs::remove_dir_all(&root);
}
