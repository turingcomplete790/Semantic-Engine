//! Spike plugin skeleton (tasks T013).

use serde_json::{json, Value};
use spike::plugin_ops;

fn input(op: Option<&str>, plugin_dir: &str) -> Vec<u8> {
    let mut args = serde_json::Map::new();
    if let Some(op) = op {
        args.insert("op".into(), json!(op));
    }
    json!({
        "server_connection": {
            "Scheme": "http", "Host": "localhost", "Port": 9998,
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": "/tmp", "PluginDir": plugin_dir
        },
        "args": args
    })
    .to_string()
    .into_bytes()
}

fn run(stdin: &[u8]) -> (Value, String) {
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    struct Sink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut logger = plugin::log::Logger::to(Box::new(Sink(buf.clone())));
    let out = plugin_ops::run(stdin, &mut logger);
    let log = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    (out, log)
}

#[test]
fn ping_answers_in_the_envelope() {
    let (out, _) = run(&input(Some("ping"), "/tmp/spike-plugin"));
    assert_eq!(out["error"], Value::Null);
    assert_eq!(out["output"]["ok"], true);
    assert_eq!(out["output"]["result"]["plugin"], "semantic-engine-spike");
}

#[test]
fn unknown_operation_lists_the_known_ones() {
    let (out, _) = run(&input(Some("nope"), "/tmp/spike-plugin"));
    assert_eq!(out["output"]["ok"], false);
    assert_eq!(out["output"]["error"]["code"], "UNKNOWN_OPERATION");
    let known = out["output"]["error"]["details"]["known"]
        .as_array()
        .unwrap();
    assert!(known.iter().any(|k| k == "ping"));
    let (out, _) = run(&input(None, "/tmp/spike-plugin"));
    assert_eq!(out["output"]["error"]["code"], "UNKNOWN_OPERATION");
}

#[test]
fn empty_plugin_dir_and_bad_input_are_invalid_input() {
    let (out, _) = run(&input(Some("ping"), ""));
    assert_eq!(out["output"]["error"]["code"], "INVALID_INPUT");
    let (out, _) = run(b"not json");
    assert_eq!(out["output"]["error"]["code"], "INVALID_INPUT");
}

#[test]
fn the_cookie_never_reaches_output_or_logs() {
    let (out, log) = run(&input(Some("nope"), "/tmp/spike-plugin"));
    assert!(!out.to_string().contains("cookie-secret"));
    assert!(!log.contains("cookie-secret"));
}
