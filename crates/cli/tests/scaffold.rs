//! Exercises `aint scaffold` against the real binary, with a local
//! mock HTTP server standing in for `AINT_MODEL_URL` — the same
//! hand-rolled-mock-server technique `aint-runtime`'s own
//! `http_model.rs` tests use, for the same reason: proving the real
//! wire protocol works, not a model of it.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};

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

fn run_scaffold(base_url: &str, description: &str, dest: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("scaffold")
        .arg(description)
        .arg(dest)
        .env("AINT_MODEL_URL", base_url)
        .env("AINT_MODEL_NAME", "test-model")
        .output()
        .expect("failed to spawn the aint binary")
}

fn temp_dest(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("aint_scaffold_test_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_well_typed_response_is_written_and_reported_as_checked() {
    let dest = temp_dest("ok");
    let content =
        r#"{"choices":[{"message":{"content":"```an\nprint(\"hello from scaffold\")\n```"}}]}"#;
    let base_url = start_mock_server(http_ok(content));

    let output = run_scaffold(&base_url, "print a greeting", &dest);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let main_an = dest.join("main.an");
    assert!(main_an.exists());
    let source = std::fs::read_to_string(&main_an).expect("should read main.an");
    assert_eq!(source, "print(\"hello from scaffold\")");
    assert!(dest.join("aint.toml").exists());

    let _ = std::fs::remove_dir_all(&dest);
}

#[test]
fn a_response_that_fails_to_type_check_is_still_written_but_reported_as_failed() {
    let dest = temp_dest("badcode");
    // `nope(1)` calls an undefined function - a real type error, not a
    // parse error, so this proves the check step actually runs.
    let content = r#"{"choices":[{"message":{"content":"```an\nnope(1)\n```"}}]}"#;
    let base_url = start_mock_server(http_ok(content));

    let output = run_scaffold(&base_url, "call something undefined", &dest);
    assert!(!output.status.success());

    let main_an = dest.join("main.an");
    assert!(
        main_an.exists(),
        "the generated file should still be written for inspection"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not type-check"),
        "expected a clear does-not-type-check warning, got: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dest);
}

#[test]
fn scaffold_without_ain_model_url_fails_clearly() {
    let dest = temp_dest("nourl");
    let output = Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("scaffold")
        .arg("anything")
        .arg(&dest)
        .env_remove("AINT_MODEL_URL")
        .output()
        .expect("failed to spawn the aint binary");

    assert!(!output.status.success());
    assert!(!dest.exists(), "no project should be created");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("AINT_MODEL_URL"));
}
