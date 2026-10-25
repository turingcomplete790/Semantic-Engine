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

// ---- A fake Stash backed by a JSON library (spec 005 reader tests)

use serde_json::{json, Value};
use std::sync::Arc;

/// Answers every request, forever: `handler` maps a request body to an HTTP status and a body.
/// Request bodies are reported on the returned channel in order.
pub fn serve(
    handler: impl Fn(&str) -> (u16, String) + Send + Sync + 'static,
) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    let handler = Arc::new(handler);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let (tx, handler) = (tx.clone(), handler.clone());
            // Each connection on its own thread, so an idle one never blocks a real request.
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut len = 0usize;
                let mut lines = 0;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 {
                        break;
                    }
                    lines += 1;
                    let h = h.trim_end();
                    if h.is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                if lines == 0 || len == 0 {
                    return; // an unused or empty connection
                }
                let mut body = vec![0; len];
                let _ = reader.read_exact(&mut body);
                let body = String::from_utf8_lossy(&body).to_string();
                let (status, reply) = handler(&body);
                // Reported before replying, so a caller that has its answer sees the request.
                let _ = tx.send(body);
                let reason = match status {
                    200 => "OK",
                    401 => "Unauthorized",
                    403 => "Forbidden",
                    503 => "Service Unavailable",
                    _ => "Status",
                };
                let mut s = stream;
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    reply.len(),
                    reply
                );
            });
        }
    });
    (port, rx)
}

/// The root field a request asks for, for example `findTags` or `version`.
pub fn root_field(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap();
    let q = v["query"].as_str().unwrap();
    let inner = &q[q.find('{').unwrap() + 1..];
    inner
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

fn ids_of(list: &Value) -> Value {
    Value::Array(
        list.as_array()
            .map(|a| a.iter().map(|id| json!({ "id": id })).collect())
            .unwrap_or_default(),
    )
}

fn custom_ok(item: &Value, criteria: &Value) -> bool {
    criteria.as_array().into_iter().flatten().all(|c| {
        let field = c["field"].as_str().unwrap();
        let v = &item["custom_fields"][field];
        match c["modifier"].as_str().unwrap() {
            "NOT_NULL" => !v.is_null(),
            "IS_NULL" => v.is_null(),
            m => panic!("fake Stash: custom field modifier {m}"),
        }
    })
}

/// Stash's filter rules, for the criteria the reader sends.
fn matches(item: &Value, filter: &Value) -> bool {
    let Some(f) = filter.as_object() else {
        return true;
    };
    for (key, c) in f {
        let ok = match key.as_str() {
            "custom_fields" => custom_ok(item, c),
            "updated_at" => {
                assert_eq!(c["modifier"], "GREATER_THAN");
                item["updated_at"].as_str().unwrap() > c["value"].as_str().unwrap()
            }
            "tags" => {
                assert_eq!(c["modifier"], "INCLUDES");
                let want: Vec<&str> = c["value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                item["tags"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|t| want.contains(&t.as_str().unwrap()))
            }
            "measurements" => {
                assert_eq!(c["modifier"], "NOT_NULL");
                item["measurements"].as_str().is_some_and(|m| !m.is_empty())
            }
            other => panic!("fake Stash: filter key {other}"),
        };
        if !ok {
            return false;
        }
    }
    true
}

fn shaped(kind: &str, item: &Value) -> Value {
    let mut out = item.clone();
    for list in ["parents", "tags", "performers"] {
        if item.get(list).is_some() {
            out[list] = ids_of(&item[list]);
        }
    }
    if kind == "performers" && out.get("measurements").is_none() {
        out["measurements"] = Value::Null;
    }
    out
}

/// A handler answering the reader's queries from `lib`:
/// `{"version": "v0.31.1", "tags": [...], "performers": [...], "scenes": [...], "images": [...]}`.
/// Items hold plain id lists (`parents`, `tags`, `performers`), `custom_fields`, and
/// `updated_at`. Results come back in descending id order, so callers must sort. As in Stash,
/// a filter is ignored when `ids` is given.
pub fn library_handler(lib: Value) -> impl Fn(&str) -> (u16, String) + Send + Sync + 'static {
    move |body| {
        let req: Value = serde_json::from_str(body).unwrap();
        let root = root_field(body);
        if root == "version" {
            let v = lib["version"].as_str().unwrap_or("v0.31.1");
            return (
                200,
                json!({"data": {"version": {"version": v}}}).to_string(),
            );
        }
        let kind = match root.as_str() {
            "findTags" => "tags",
            "findPerformers" => "performers",
            "findScenes" => "scenes",
            "findImages" => "images",
            other => panic!("fake Stash: root field {other}"),
        };
        let query = req["query"].as_str().unwrap();
        let mut ids: Option<Vec<String>> = None;
        let mut filter = Value::Null;
        for (_, v) in req["variables"].as_object().into_iter().flatten() {
            match v {
                Value::Array(a) => {
                    ids = Some(a.iter().map(|x| x.as_str().unwrap().to_string()).collect())
                }
                Value::Object(_) => filter = v.clone(),
                _ => {}
            }
        }
        let empty = vec![];
        let all = lib[kind].as_array().unwrap_or(&empty);
        let mut items: Vec<&Value> = match &ids {
            Some(ids) => all
                .iter()
                .filter(|i| ids.iter().any(|id| i["id"] == *id))
                .collect(),
            None => all.iter().filter(|i| matches(i, &filter)).collect(),
        };
        if query.contains("per_page: 1,") || query.contains("per_page: 1}") {
            items.sort_by(|a, b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str()));
            items.truncate(1);
        } else {
            items.sort_by_key(|i| {
                std::cmp::Reverse(i["id"].as_str().unwrap().parse::<u64>().unwrap())
            });
        }
        let rows: Vec<Value> = items.iter().map(|i| shaped(kind, i)).collect();
        (200, json!({"data": {root: {kind: rows}}}).to_string())
    }
}

/// The JSON library at `tests/fixtures/reader/{name}`.
pub fn library(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/reader/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A client of the fake server on `port`.
pub fn client(port: u16) -> stash_adapter::Client {
    stash_adapter::Client::with_api_key(&format!("http://127.0.0.1:{port}"), "test-key").unwrap()
}
