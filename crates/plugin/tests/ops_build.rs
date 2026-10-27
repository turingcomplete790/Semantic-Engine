//! Contract test for the "Build semantic graph" task (spec 007 T024; contracts/operations-1.2.md).
//! The fake Stash answers the reader's queries from the adapter's reader fixture library.

#[path = "../../stash-adapter/tests/common/mod.rs"]
mod adapter;
mod common;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn library() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stash-adapter/tests/fixtures/reader/library.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn settings(server_id: Option<&str>) -> String {
    let plugin = match server_id {
        Some(id) => json!({ "serverId": id }),
        None => json!({}),
    };
    json!({"data": {"configuration": {"plugins": {"semantic-engine": plugin}}}}).to_string()
}

/// A fake Stash: the plugin settings, then the reader's queries. With `status` other than 200,
/// every reader query is refused with it.
fn stash(server_id: Option<&'static str>, status: u16) -> u16 {
    let lib = library_handler();
    let (port, _rx) = adapter::serve(move |body| {
        if adapter::root_field(body) == "configuration" {
            return (200, settings(server_id));
        }
        if status != 200 {
            return (status, "{}".into());
        }
        lib(body)
    });
    port
}

fn library_handler() -> impl Fn(&str) -> (u16, String) + Send + Sync + 'static {
    adapter::library_handler(library())
}

fn plugin_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("se-ops-build-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn input(port: u16, dir: &Path) -> Vec<u8> {
    json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": port,
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": dir, "PluginDir": dir
        },
        "args": {"op": "buildGraph"}
    })
    .to_string()
    .into_bytes()
}

#[test]
fn a_build_returns_its_report_and_logs_progress_and_a_summary() {
    let dir = plugin_dir("ok");
    let port = stash(Some(common::VALID_ID), 200);
    let (env, log) = common::run(&input(port, &dir));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["op"], "buildGraph");
    assert_eq!(env["contractVersion"], engine::CONTRACT_VERSION);
    let r = &env["result"];
    assert_eq!(r["generation"], 1);
    assert!(r["facts"].as_u64().unwrap() > 0);
    for k in ["tags", "performers", "scenes", "images"] {
        assert!(r["records"][k].is_u64(), "{k}");
    }
    assert!(r["notes"]["skipped"].is_object() && r["notes"]["mismatches"].is_object());
    assert!(r["seconds"].is_f64());
    assert!(r.get("cleared").is_none());

    assert!(log.contains("\u{1}p\u{2}1.00"), "progress reaches 1");
    assert!(log.contains("build: generation 1 published"), "{log}");
    assert!(!log.contains("cookie-secret"));
    assert!(dir.join("graph/current").exists());
}

#[test]
fn no_server_id_names_the_task_to_run() {
    let dir = plugin_dir("noid");
    let port = stash(None, 200);
    let (env, _) = common::run(&input(port, &dir));
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "SERVER_ID_NOT_GENERATED");
    assert!(env["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Generate server identifier"));
    assert!(!dir.join("graph").exists());
}

#[test]
fn a_held_store_gives_busy() {
    let dir = plugin_dir("busy");
    let store = engine::graph::GraphStore::new(dir.join("graph"), common::VALID_ID);
    store.clear_for_rebuild().unwrap();
    let _held = store.writer().unwrap();
    let port = stash(Some(common::VALID_ID), 200);
    let (env, _) = common::run(&input(port, &dir));
    assert_eq!(env["error"]["code"], "BUSY", "{env}");
}

#[test]
fn refused_credentials_are_internal_and_named() {
    let dir = plugin_dir("401");
    let port = stash(Some(common::VALID_ID), 401);
    let (env, _) = common::run(&input(port, &dir));
    assert_eq!(env["error"]["code"], "INTERNAL", "{env}");
    assert!(env["error"]["message"]
        .as_str()
        .unwrap()
        .contains("refused the credentials"));
    assert!(!dir.join("graph/current").exists(), "nothing published");
}
