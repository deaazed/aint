//! Exercises `http_serve` (milestone 25) through the real built
//! `aint` binary: spawns a real server process, sends real HTTP/1.1
//! requests over a real TCP socket, and asserts on the real response
//! bytes — the only way to actually prove this native speaks HTTP.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

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

fn http_get(port: u16, path: &str) -> String {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("should connect to the running server");
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .expect("should write the request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("should read the response");
    response
}

#[test]
fn http_serve_dispatches_real_requests_to_handle_request() {
    const PORT: u16 = 18123;
    let path = std::env::temp_dir().join(format!("aint_cli_http_{}.an", std::process::id()));
    let source = format!(
        "import http\n\
         fn handle_request(method: String, path: String, body: String) -> String {{\n\
             if path == \"/hello\" {{\n\
                 return \"{{\\\"message\\\": \\\"hi\\\"}}\"\n\
             }} else {{\n\
                 return \"{{\\\"message\\\": \\\"not found\\\"}}\"\n\
             }}\n\
         }}\n\
         await http_serve({PORT})\n"
    );
    fs::write(&path, source).expect("failed to write a temporary .an file");

    let child = Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("run")
        .arg(&path)
        .spawn()
        .expect("failed to spawn the aint binary");
    let _server = ServerProcess { child };

    assert!(
        wait_for_port(PORT, Duration::from_secs(5)),
        "server never started listening"
    );

    let hello = http_get(PORT, "/hello");
    assert!(hello.starts_with("HTTP/1.1 200 OK"), "got: {hello}");
    assert!(hello.contains("\"message\": \"hi\""), "got: {hello}");

    let other = http_get(PORT, "/nonexistent");
    assert!(other.starts_with("HTTP/1.1 200 OK"), "got: {other}");
    assert!(other.contains("\"message\": \"not found\""), "got: {other}");

    fs::remove_file(&path).ok();
}
