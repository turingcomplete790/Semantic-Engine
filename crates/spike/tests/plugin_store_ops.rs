//! `write` and `query` ops on a local plugin folder (tasks T019, T020).

use serde_json::{json, Value};
use spike::plugin_ops;

fn call(dir: &str, args: Value) -> Value {
    let stdin = json!({
        "server_connection": {"Scheme": "http", "Host": "localhost", "Port": 1, "Dir": "/tmp", "PluginDir": dir},
        "args": args
    })
    .to_string();
    let mut logger = plugin::log::Logger::to(Box::new(std::io::sink()));
    plugin_ops::run(stdin.as_bytes(), &mut logger)["output"].clone()
}

fn tmp(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("spike-ops-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d.to_string_lossy().into_owned()
}

#[test]
fn option_b_write_publishes_and_queries_answer_every_id() {
    let dir = tmp("b");
    let w = call(
        &dir,
        json!({"op": "write", "option": "B", "store": "s", "scale": "1000"}),
    );
    assert_eq!(w["ok"], true, "{w}");
    assert_eq!(w["result"]["generation"], 1);
    assert!(w["result"]["triples"].as_u64().unwrap() > 2000);
    for id in 1..=5 {
        let q = call(&dir, json!({"op": "query", "store": "s", "queryId": id}));
        assert_eq!(q["ok"], true, "query {id}: {q}");
        let r = &q["result"];
        assert!(r["rowCount"].as_u64().unwrap() > 0, "query {id} found rows");
        assert_eq!(r["consistency"]["mixed"], false);
        for t in ["storeOpen", "query", "serialise"] {
            assert!(r["timings"][t].as_f64().unwrap() >= 0.0);
        }
    }
}

#[test]
fn options_a_and_c_read_the_working_store() {
    let dir = tmp("ac");
    for option in ["A", "C"] {
        let store = format!("s{option}");
        let w = call(
            &dir,
            json!({"op": "write", "option": option, "store": store, "scale": "50"}),
        );
        assert_eq!(w["ok"], true, "{w}");
        assert_eq!(
            w["result"]["generation"],
            Value::Null,
            "no publish for {option}"
        );
        let q = call(
            &dir,
            json!({"op": "query", "store": store, "queryId": 1, "option": option}),
        );
        assert_eq!(q["ok"], true, "{q}");
        assert_eq!(q["result"]["rowCount"], 50);
    }
}

#[test]
fn a_missing_store_is_an_error_not_an_empty_answer() {
    let dir = tmp("missing");
    let q = call(
        &dir,
        json!({"op": "query", "store": "nothing", "queryId": 1}),
    );
    assert_eq!(q["ok"], false);
    assert_eq!(q["error"]["code"], "INTERNAL");
    let q = call(&dir, json!({"op": "query", "queryId": 9}));
    assert_eq!(q["error"]["code"], "INVALID_INPUT");
}

#[test]
fn a_second_writer_with_lock_mode_fail_is_busy() {
    let dir = tmp("busy");
    let path = std::path::Path::new(&dir).join("store").join("s");
    std::fs::create_dir_all(&path).unwrap();
    let _held = spike::store::lock_writer(&path, spike::store::LockMode::Fail).unwrap();
    let w = call(
        &dir,
        json!({"op": "write", "option": "B", "store": "s", "scale": "10", "lockMode": "fail"}),
    );
    assert_eq!(w["ok"], false);
    assert_eq!(w["error"]["details"]["busy"], true);
}

#[test]
fn record_hook_appends_one_line_per_call() {
    let dir = tmp("hook");
    std::fs::create_dir_all(&dir).unwrap();
    for id in [5, 6] {
        let r = call(
            &dir,
            json!({"op": "recordHook", "hookContext": {"id": id, "type": "Scene.Update.Post", "input": {"id": "5"}, "inputFields": ["custom_fields"]}}),
        );
        assert_eq!(r["ok"], true, "{r}");
    }
    let text = std::fs::read_to_string(std::path::Path::new(&dir).join("hooks.jsonl")).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["type"], "Scene.Update.Post");
    assert_eq!(lines[1]["id"], 6);
    assert_eq!(lines[0]["inputFields"], json!(["custom_fields"]));
    assert!(lines[0]["receivedMs"].as_f64().unwrap() > 1.7e12);
}
