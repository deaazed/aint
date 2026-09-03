//! Verifies `.env` in the current directory is actually loaded into
//! the process before a real model call, without needing it exported
//! by hand first (milestone 41).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

/// Starts a minimal HTTP/1.1 server that accepts exactly one
/// connection, reads (and discards) whatever request arrives, and
/// writes back a fixed JSON chat-completion response. Returns the
/// base URL to point `AINT_MODEL_URL` at. Same shape
/// `aint-runtime`'s own `http_model.rs` test module uses internally —
/// duplicated here rather than exposed across the crate boundary for
/// one integration test.
fn start_mock_model_server(content: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind a local port");
    let addr = listener.local_addr().expect("failed to read local addr");
    let body = format!(r#"{{"choices":[{{"message":{{"content":"{content}"}}}}]}}"#);
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 65536];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://{addr}")
}

/// The real proof: a `.env` sitting in the current directory, never
/// exported into this test process's own environment, still reaches a
/// real (mock) model over HTTP when `aint run` starts in that
/// directory — `AINT_MODEL_URL`/`AINT_MODEL_NAME` are explicitly
/// removed from the child's environment first, so this can only pass
/// if `load_dotenv` actually read the file.
#[test]
fn aint_run_loads_dotenv_and_uses_it_for_a_real_model_call() {
    let base_url = start_mock_model_server("hello from .env");

    let dir = std::env::temp_dir().join(format!("aint_dotenv_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("should create the temp dir");

    std::fs::write(
        dir.join(".env"),
        format!("AINT_MODEL_URL={base_url}\nAINT_MODEL_NAME=test-model\n"),
    )
    .expect("should write .env");
    std::fs::write(
        dir.join("main.an"),
        "infer classify(x: String) -> String\nprint(await classify(\"hi\"))\n",
    )
    .expect("should write main.an");

    let output = Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("run")
        .arg("main.an")
        .current_dir(&dir)
        .env_remove("AINT_MODEL_URL")
        .env_remove("AINT_MODEL_NAME")
        .env_remove("AINT_MODEL_API_KEY")
        .output()
        .expect("failed to spawn the aint binary");

    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello from .env\n");
}

/// A real environment variable always wins over `.env` — `.env` only
/// fills in what isn't already set.
#[test]
fn a_real_env_var_overrides_the_dotenv_value() {
    let base_url = start_mock_model_server("from the real env var");

    let dir =
        std::env::temp_dir().join(format!("aint_dotenv_test_override_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("should create the temp dir");

    std::fs::write(
        dir.join(".env"),
        "AINT_MODEL_URL=http://127.0.0.1:1\nAINT_MODEL_NAME=wrong-model\n",
    )
    .expect("should write .env");
    std::fs::write(
        dir.join("main.an"),
        "infer classify(x: String) -> String\nprint(await classify(\"hi\"))\n",
    )
    .expect("should write main.an");

    let output = Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("run")
        .arg("main.an")
        .current_dir(&dir)
        .env("AINT_MODEL_URL", &base_url)
        .env("AINT_MODEL_NAME", "test-model")
        .env_remove("AINT_MODEL_API_KEY")
        .output()
        .expect("failed to spawn the aint binary");

    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "from the real env var\n"
    );
}
