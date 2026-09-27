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

/// `http_serve_assets` (milestone 52) through the real binary and a
/// real TCP socket: a real file served with the right bytes and
/// `Content-Type`, a path-traversal attempt never escaping the
/// declared asset root, and a request that isn't an asset at all still
/// reaching `handle_request` exactly as if no asset root existed.
#[test]
fn http_serve_assets_serves_real_files_and_rejects_traversal() {
    const PORT: u16 = 18124;
    let asset_dir = std::env::temp_dir().join(format!("aint_cli_assets_{}", std::process::id()));
    fs::create_dir_all(&asset_dir).expect("failed to create a temporary asset directory");
    fs::write(asset_dir.join("hello.txt"), "hi from disk").expect("failed to write hello.txt");
    // A file the traversal attempt below must never be able to reach,
    // planted one directory above the declared asset root.
    let outside_path = asset_dir
        .parent()
        .unwrap()
        .join(format!("outside_the_root_{}.secret", std::process::id()));
    fs::write(&outside_path, "should never be served")
        .expect("failed to write the outside-the-root file");

    let script_path =
        std::env::temp_dir().join(format!("aint_cli_http_assets_{}.an", std::process::id()));
    let asset_dir_literal = asset_dir.display().to_string().replace('\\', "\\\\");
    let source = format!(
        "import http\n\
         fn handle_request(method: String, path: String, body: String) -> String {{\n\
             return \"handled by handle_request\"\n\
         }}\n\
         await http_serve_assets({PORT}, \"{asset_dir_literal}\")\n"
    );
    fs::write(&script_path, source).expect("failed to write a temporary .an file");

    let child = Command::new(env!("CARGO_BIN_EXE_aint"))
        .arg("run")
        .arg(&script_path)
        .spawn()
        .expect("failed to spawn the aint binary");
    let _server = ServerProcess { child };

    assert!(
        wait_for_port(PORT, Duration::from_secs(5)),
        "server never started listening"
    );

    let asset = http_get(PORT, "/hello.txt");
    assert!(asset.starts_with("HTTP/1.1 200 OK"), "got: {asset}");
    assert!(
        asset.contains("Content-Type: text/plain; charset=utf-8"),
        "got: {asset}"
    );
    assert!(asset.ends_with("hi from disk"), "got: {asset}");

    // A path that would escape the declared root if traversal worked -
    // must fall through to handle_request, never serve the planted
    // outside-the-root file.
    let traversal = http_get(
        PORT,
        &format!("/../outside_the_root_{}.secret", std::process::id()),
    );
    assert!(
        traversal.contains("handled by handle_request"),
        "got: {traversal}"
    );
    assert!(
        !traversal.contains("should never be served"),
        "got: {traversal}"
    );

    // A path under the asset root that just doesn't exist there also
    // falls through, same as any other miss.
    let missing = http_get(PORT, "/does-not-exist.txt");
    assert!(
        missing.contains("handled by handle_request"),
        "got: {missing}"
    );

    fs::remove_file(&script_path).ok();
    fs::remove_dir_all(&asset_dir).ok();
    fs::remove_file(&outside_path).ok();
}
