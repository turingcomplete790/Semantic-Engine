#![allow(dead_code)]
//! Test helpers: a fake Stash GraphQL endpoint and plugin input builders.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

/// Serves `replies` in order, one per request, and reports each request body.
pub fn fake_stash(replies: Vec<&'static str>) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for reply in replies {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).unwrap_or(0) == 0 {
                    break;
                }
                let h = h.trim_end();
                if h.is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; len];
            let _ = reader.read_exact(&mut body);
            let mut s = stream;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            let _ = tx.send(String::from_utf8_lossy(&body).to_string());
        }
    });
    (port, rx)
}

/// A port nothing listens on.
pub fn dead_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

pub fn input(port: u16, op: &str) -> Vec<u8> {
    serde_json::json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": port,
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": "/tmp", "PluginDir": "/tmp"
        },
        "args": {"op": op}
    })
    .to_string()
    .into_bytes()
}

/// Runs one invocation in-process and returns (output envelope, stderr log text).
pub fn run(stdin: &[u8]) -> (serde_json::Value, String) {
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    struct Sink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut logger = plugin::log::Logger::to(Box::new(Sink(buf.clone())));
    let out = plugin::run(stdin, &mut logger);
    assert!(out["error"].is_null());
    let log = String::from_utf8_lossy(&buf.lock().unwrap()).to_string();
    (out["output"].clone(), log)
}

/// Like `fake_stash`, but each reply is computed from the request body. Connections that send
/// no request are ignored, so only real requests count toward `count`.
pub fn fake_stash_fn(count: usize, handler: fn(&str) -> String) -> (u16, mpsc::Receiver<String>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    let served = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let (tx, served) = (tx.clone(), served.clone());
            // Each connection on its own thread: a connection the client opened but never used
            // must neither block the real request nor use up one of the `count` replies.
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
                    return;
                }
                if served.fetch_add(1, Ordering::SeqCst) >= count {
                    return; // more requests than the test expects: no reply
                }
                let mut len = 0usize;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 {
                        break;
                    }
                    let h = h.trim_end();
                    if h.is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                let _ = reader.read_exact(&mut body);
                let body = String::from_utf8_lossy(&body).to_string();
                let reply = handler(&body);
                let _ = tx.send(body);
                let mut s = stream;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    reply.len(),
                    reply
                );
            });
        }
    });
    (port, rx)
}

/// Echoes a configurePlugin `input` back as the stored settings (Stash returns them).
pub fn echo_configure(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if !v["query"].as_str()?.contains("configurePlugin") {
        return None;
    }
    Some(serde_json::json!({"data": {"configurePlugin": v["variables"]["input"]}}).to_string())
}

pub const VALID_ID: &str = "11111111-2222-4333-8444-555555555555";

/// Plugin input for `op` with extra arguments.
pub fn input_args(port: u16, op: &str, args: serde_json::Value) -> Vec<u8> {
    let mut a = args.as_object().cloned().unwrap_or_default();
    a.insert("op".into(), op.into());
    serde_json::json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": port,
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": "/tmp", "PluginDir": "/tmp"
        },
        "args": a
    })
    .to_string()
    .into_bytes()
}

/// A recorded relation response from `crates/stash-adapter/tests/fixtures/relations/`.
pub fn relation_fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../stash-adapter/tests/fixtures/relations")
            .join(name),
    )
    .unwrap()
}

/// Answers each flag-filtered `findTags` with that flag's recorded response.
pub fn tags_by_flag(body: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(body).unwrap();
    let field = v["variables"]["filter"]["custom_fields"][0]["field"]
        .as_str()
        .unwrap_or("none")
        .to_string();
    relation_fixture(&format!("tags_{field}.json"))
}

/// Requests one `relations` call makes: one definitions read per flag.
pub const DEFINITION_READS: usize = 7;
