#![allow(dead_code)]
//! A fake Stash GraphQL endpoint for adapter tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

/// Answers `count` requests; `handler` maps each request body to a JSON reply. Request bodies
/// are reported on the returned channel in order.
pub fn fake_stash(count: usize, handler: fn(&str) -> String) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for _ in 0..count {
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
            let body = String::from_utf8_lossy(&body).to_string();
            let reply = handler(&body);
            let mut s = stream;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            let _ = tx.send(body);
        }
    });
    (port, rx)
}

/// Echoes a `configurePlugin` mutation's `input` variable back as the stored map, the way Stash
/// returns the plugin's configuration after writing it.
pub fn echo_configure(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if !v["query"].as_str()?.contains("configurePlugin") {
        return None;
    }
    Some(serde_json::json!({"data": {"configurePlugin": v["variables"]["input"]}}).to_string())
}
