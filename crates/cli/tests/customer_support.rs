//! End-to-end proof that milestone 25's customer-support app
//! (`examples/customer_support/server.an`) actually works: spawns the
//! real, unmodified example file as a real server process in an
//! isolated scratch directory (so its `.aintdb` doesn't touch the
//! real repo), and drives it with real HTTP requests over a real TCP
//! socket - register, a duplicate-email rejection, login (and a
//! wrong-password rejection), an unauthenticated request rejection,
//! and listing tickets for a session with none yet.
//!
//! Ticket *creation* needs a real `Model` (`classify_sentiment` is an
//! `infer` call, and `aint run` uses an unconfigured `MockModel` by
//! default - see `docs/milestones/25-real-application/SPEC.md`), so
//! it isn't exercised live here; the AI-driven priority decision
//! itself is covered deterministically by
//! `examples/customer_support/priority_logic_test.an` via `aint test`
//! instead. This file proves the surrounding http/db/auth path is
//! real and correct.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const SERVER_AN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/customer_support/server.an"
));

struct ServerProcess {
    child: Child,
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn http_post(port: u16, path: &str, body: &str) -> String {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("should connect to the running server");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .expect("should write the request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("should read the response");
    response
}

fn json_field<'a>(response: &'a str, field: &str) -> &'a str {
    let needle = format!("\"{field}\":\"");
    let start = response
        .find(&needle)
        .unwrap_or_else(|| panic!("expected field `{field}` in response body: {response}"))
        + needle.len();
    let end = response[start..].find('"').expect("closing quote") + start;
    &response[start..end]
}

#[test]
fn customer_support_server_handles_register_login_and_list() {
    const PORT: u16 = 18234;
    let scratch_dir =
        std::env::temp_dir().join(format!("aint_customer_support_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch_dir);
    fs::create_dir_all(&scratch_dir).expect("should create the scratch dir");
    let server_path = scratch_dir.join("server.an");
    let source = SERVER_AN.replace("http_serve(8080)", &format!("http_serve({PORT})"));
    fs::write(&server_path, source).expect("should write server.an into the scratch dir");

    let child = Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("run")
        .arg(&server_path)
        .current_dir(&scratch_dir)
        .spawn()
        .expect("failed to spawn the aint binary");
    let _server = ServerProcess { child };

    assert!(
        wait_for_port(PORT, Duration::from_secs(5)),
        "server never started listening"
    );

    let register = http_post(
        PORT,
        "/register",
        "{\"email\":\"ada@example.com\",\"password\":\"hunter2\"}",
    );
    assert!(register.contains("\"user_id\""), "got: {register}");

    let duplicate = http_post(
        PORT,
        "/register",
        "{\"email\":\"ada@example.com\",\"password\":\"hunter2\"}",
    );
    assert!(duplicate.contains("already registered"), "got: {duplicate}");

    let wrong_password = http_post(
        PORT,
        "/login",
        "{\"email\":\"ada@example.com\",\"password\":\"wrong\"}",
    );
    assert!(
        wrong_password.contains("invalid email or password"),
        "got: {wrong_password}"
    );

    let login = http_post(
        PORT,
        "/login",
        "{\"email\":\"ada@example.com\",\"password\":\"hunter2\"}",
    );
    assert!(login.contains("\"token\""), "got: {login}");
    let token = json_field(&login, "token").to_string();

    let list = http_post(PORT, "/tickets/list", &format!("{{\"token\":\"{token}\"}}"));
    assert!(
        list.contains("\"tickets\": []"),
        "expected no tickets yet, got: {list}"
    );

    let unauthenticated = http_post(PORT, "/tickets/list", "{\"token\":\"not-a-real-token\"}");
    assert!(
        unauthenticated.contains("not authenticated"),
        "got: {unauthenticated}"
    );

    fs::remove_dir_all(&scratch_dir).ok();
}
