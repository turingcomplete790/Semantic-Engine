//! Contract test for the "Update semantic graph" task (spec 008 T021; contracts/operations-1.3.md).

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

fn stash(server_id: Option<&'static str>) -> u16 {
    let lib = adapter::library_handler(library());
    let (port, _rx) = adapter::serve(move |body| {
        if adapter::root_field(body) == "configuration" {
            let plugin = match server_id {
                Some(id) => json!({ "serverId": id }),
                None => json!({}),
            };
            return (
                200,
                json!({"data": {"configuration": {"plugins": {"semantic-engine": plugin}}}})
                    .to_string(),
            );
        }
        lib(body)
    });
    port
}

fn plugin_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("se-ops-update-{name}-{}", std::process::id()));
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
        "args": {"op": "updateGraph"}
    })
    .to_string()
    .into_bytes()
}

#[test]
fn a_first_update_rebuilds_and_a_second_finds_the_graph_current() {
    let dir = plugin_dir("first");
    let port = stash(Some(common::VALID_ID));
    let (env, log) = common::run(&input(port, &dir));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["op"], "updateGraph");
    assert_eq!(env["contractVersion"], engine::CONTRACT_VERSION);
    assert_eq!(env["result"]["mode"], "rebuild");
    assert_eq!(env["result"]["rebuildReason"]["cause"], "firstBuild");
    assert!(env["result"]["build"]["facts"].as_u64().unwrap() > 0);
    assert!(
        log.contains("update: rebuilt in full (first build)"),
        "{log}"
    );
    assert!(!log.contains("cookie-secret"));

    let (env, log) = common::run(&input(port, &dir));
    assert_eq!(env["result"]["mode"], "current", "{env}");
    assert_eq!(env["result"]["generation"], 1);
    assert!(log.contains("update: the graph is current"), "{log}");
}

#[test]
fn a_held_store_gives_busy() {
    let dir = plugin_dir("busy");
    let port = stash(Some(common::VALID_ID));
    common::run(&input(port, &dir));
    let store = engine::graph::GraphStore::new(dir.join("graph"), common::VALID_ID);
    let _held = store.writer().unwrap();
    let (env, _) = common::run(&input(port, &dir));
    assert_eq!(env["error"]["code"], "BUSY", "{env}");
}

#[test]
fn no_server_id_names_the_task_to_run() {
    let dir = plugin_dir("noid");
    let port = stash(None);
    let (env, _) = common::run(&input(port, &dir));
    assert_eq!(env["error"]["code"], "SERVER_ID_NOT_GENERATED");
}
