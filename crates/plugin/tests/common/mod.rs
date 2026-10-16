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
