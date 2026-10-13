//! End-to-end tests for `ck --serve-http`: spawn the binary on an ephemeral port and speak
//! MCP's Streamable HTTP to it over a raw TCP connection.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;

struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start `ck --serve-http 127.0.0.1:0` in `root`, optionally with a bearer key file, and
/// read the bound port from its startup log.
fn start(root: &TempDir, key_file: Option<&PathBuf>) -> Server {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ck"));
    cmd.args(["--serve-http", "127.0.0.1:0"])
        .current_dir(root.path())
        .env_remove("CK_INDEX_DIR")
        .env_remove("CK_MCP_ALLOWED_ROOTS")
        .env("NO_COLOR", "1")
        .env("RUST_LOG", "info")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    match key_file {
        Some(path) => cmd.env("CK_MCP_API_KEY_FILE", path),
        None => cmd.env_remove("CK_MCP_API_KEY_FILE"),
    };
    let mut child = cmd.spawn().expect("spawn ck --serve-http");
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    let port = loop {
        line.clear();
        assert!(
            stderr.read_line(&mut line).unwrap() > 0,
            "ck exited before listening"
        );
        if let Some(rest) = line.split("listening on http://127.0.0.1:").nth(1) {
            break rest.trim().parse().expect("port in startup log");
        }
    };
    // Keep draining the log so a full pipe never blocks the server.
    std::thread::spawn(move || std::io::copy(&mut stderr, &mut std::io::sink()));
    Server { child, port }
}

/// POST one JSON-RPC message; returns the status code and body.
fn post(server: &Server, headers: &[(&str, &str)], body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    let mut request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\
         Content-Type: application/json\r\nAccept: application/json, text/event-stream\r\n\
         MCP-Protocol-Version: 2025-06-18\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response[9..12].parse().unwrap();
    let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

fn project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "fn needle_function() {}\n").unwrap();
    dir
}

const LIST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;

#[test]
fn bearer_key_is_required_when_configured() {
    let root = project();
    let keys = TempDir::new().unwrap();
    let key_file = keys.path().join("key");
    std::fs::write(&key_file, "s3cret\n").unwrap();
    let server = start(&root, Some(&key_file));

    assert_eq!(post(&server, &[], LIST).0, 401);
    assert_eq!(
        post(&server, &[("Authorization", "Bearer wrong")], LIST).0,
        401
    );

    let auth = [("Authorization", "Bearer s3cret")];
    let (status, body) = post(&server, &auth, LIST);
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"semantic_search\""), "{body}");

    let search = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "regex_search",
                    "arguments": { "pattern": "needle_function", "path": root.path() } }
    })
    .to_string();
    let (status, body) = post(&server, &auth, &search);
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("needle_function"), "{body}");

    // The stdio server's sandbox applies: paths outside cwd are refused.
    let outside = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"regex_search","arguments":{"pattern":"x","path":"/"}}}"#;
    let (_, body) = post(&server, &auth, outside);
    assert!(body.contains("outside the MCP allowed roots"), "{body}");
}

#[test]
fn without_a_key_requests_are_accepted_but_host_is_checked() {
    let root = project();
    let server = start(&root, None);

    let (status, body) = post(&server, &[], LIST);
    assert_eq!(status, 200, "{body}");

    // rmcp's DNS-rebinding guard: a non-loopback Host is refused.
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    write!(
        stream,
        "POST /mcp HTTP/1.1\r\nHost: evil.example\r\nConnection: close\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n\r\n{LIST}",
        LIST.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 403"), "{response}");
}
