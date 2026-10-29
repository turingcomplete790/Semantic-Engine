mod common;

const FIELDS: [&str; 7] = [
    "engineVersion",
    "contractVersion",
    "vocabularyVersion",
    "oxigraphVersion",
    "rocksdbVersion",
    "stashMinimumVersion",
    "build",
];

#[test]
fn status_reports_versions_and_the_server_version() {
    let (port, rx) = common::fake_stash(vec![r#"{"data":{"version":{"version":"v0.31.1"}}}"#]);
    let (env, log) = common::run(&common::input(port, "status"));
    assert_eq!(env["ok"], true, "{env}");
    let r = &env["result"];
    for f in FIELDS {
        assert!(r.get(f).is_some(), "missing {f}: {r}");
    }
    assert_eq!(r["stashVersion"], "v0.31.1");
    assert!(r["serverId"].is_null());
    assert_eq!(r["rocksdb"]["available"], true);
    assert_eq!(r["rocksdb"]["version"], r["rocksdbVersion"]);
    assert!(rx.recv().unwrap().contains("version"));
    assert!(!log.contains("cookie-secret"));
}

#[test]
fn unreachable_stash_still_returns_local_versions() {
    let (env, _) = common::run(&common::input(common::dead_port(), "status"));
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "STASH_UNREACHABLE");
    for f in FIELDS {
        assert!(env["error"]["details"].get(f).is_some(), "missing {f}");
    }
}

#[test]
fn older_stash_is_too_old() {
    let (port, _rx) = common::fake_stash(vec![r#"{"data":{"version":{"version":"v0.30.0"}}}"#]);
    let (env, _) = common::run(&common::input(port, "status"));
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "STASH_TOO_OLD");
    assert_eq!(env["error"]["details"]["stashVersion"], "v0.30.0");
}

// ---- The graph object (spec 009 T037; contracts/operations-1.4.md)

use engine::build::stamp::{BuildStamp, How, Items};
use engine::build::{run, BuildOptions};
use engine::graph::GraphStore;
use serde_json::{json, Value};
use stash_adapter::reader::{LibraryRead, MemoryReader};
use std::path::{Path, PathBuf};

/// The example server id of the compatibility fixture (spec 007).
const SERVER: &str = "00000000-0000-4000-8000-000000000001";

fn plugin_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("se-ops-status-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn built(name: &str) -> PathBuf {
    let dir = plugin_dir(name);
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../engine/tests/fixtures/build/library.json");
    let lib: LibraryRead = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    run(
        &MemoryReader::new(lib),
        &GraphStore::new(dir.join("graph"), SERVER),
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
    dir
}

fn status_input(port: u16, dir: &Path) -> Vec<u8> {
    json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": port,
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": dir, "PluginDir": dir
        },
        "args": {"op": "status"}
    })
    .to_string()
    .into_bytes()
}

/// `graph` from a status call with Stash unreachable, where it sits in the error's details.
fn graph(dir: &Path) -> Value {
    let (env, _) = common::run(&status_input(common::dead_port(), dir));
    assert_eq!(env["error"]["code"], "STASH_UNREACHABLE", "{env}");
    env["error"]["details"]["graph"].clone()
}

#[test]
fn a_built_graph_is_ready_with_how_and_items() {
    let dir = built("ready");
    let (port, _rx) = common::fake_stash(vec![r#"{"data":{"version":{"version":"v0.31.1"}}}"#]);
    let (env, _) = common::run(&status_input(port, &dir));
    assert_eq!(env["ok"], true, "{env}");
    let g = &env["result"]["graph"];
    assert_eq!(g["state"], "ready", "{g}");
    assert_eq!(g["format"], engine::graph::STORE_FORMAT);
    assert_eq!(g["generation"], 1);
    assert!(g["publishedAt"].as_str().unwrap().ends_with('Z'));
    assert_eq!(g["how"], "build");
    assert!(g["facts"].as_u64().unwrap() > 0);
    for k in ["tags", "performers", "scenes", "images"] {
        assert!(g["items"][k].as_u64().unwrap() > 0, "{k}: {g}");
    }
    assert_eq!(g["pending"], false);
    assert_eq!(graph(&dir), *g, "the same in the error details");
}

#[test]
fn an_update_and_a_generation_without_a_stamp() {
    let dir = built("update");
    let store = GraphStore::new(dir.join("graph"), SERVER);
    let mut w = store.writer().unwrap();
    w.add(&[]).unwrap();
    let items = Items {
        tags: 1,
        performers: 2,
        scenes: 3,
        images: 4,
    };
    w.publish_with(
        BuildStamp::new(Default::default(), How::Update)
            .with_items(items)
            .to_json(),
    )
    .unwrap();
    let g = graph(&dir);
    assert_eq!(g["how"], "update", "{g}");
    assert_eq!(g["generation"], 2);
    assert_eq!(
        g["items"],
        json!({ "tags": 1, "performers": 2, "scenes": 3, "images": 4 })
    );

    let w = store.writer().unwrap();
    w.publish().unwrap();
    let g = graph(&dir);
    assert_eq!(g["state"], "ready", "{g}");
    assert_eq!(g["how"], Value::Null);
    assert_eq!(g["items"], Value::Null);
}

#[test]
fn not_built_and_needs_rebuild() {
    let dir = plugin_dir("empty");
    assert_eq!(
        graph(&dir),
        json!({ "state": "notBuilt", "pending": false })
    );

    let dir = built("rebuild");
    let stamp = dir.join("graph/store.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&stamp).unwrap()).unwrap();
    v["format"] = json!(1);
    std::fs::write(&stamp, v.to_string()).unwrap();
    assert_eq!(
        graph(&dir),
        json!({ "state": "needsRebuild", "reason": "otherFormat", "pending": false })
    );
}

#[test]
fn a_recorded_save_is_pending() {
    let dir = built("pending");
    let (port, _rx) = common::fake_stash(vec![r#"{"data":{"runPluginTask":"5"}}"#]);
    let hook = json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": port,
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": dir, "PluginDir": dir
        },
        "args": { "op": "recordChange", "hookContext": { "id": 3, "type": "Tag.Update.Post" } }
    });
    let (env, _) = common::run(hook.to_string().as_bytes());
    assert_eq!(env["result"]["graph"]["recorded"], true, "{env}");
    assert_eq!(graph(&dir)["pending"], true);
}

#[test]
fn status_does_not_open_the_store() {
    let dir = built("held");
    let store = GraphStore::new(dir.join("graph"), SERVER);
    let _held = store.writer().unwrap();
    // The generation's database made unreadable: opening it would fail, describing it does not.
    let generation = dir.join("graph/snapshots/1");
    for e in std::fs::read_dir(&generation).unwrap() {
        std::fs::write(e.unwrap().path(), b"not a database").unwrap();
    }
    assert!(store.reader().is_err(), "opening the store fails now");
    let config = Box::leak(
        json!({"data": {"configuration": {"plugins": {"semantic-engine": {"serverId": SERVER}}}}})
            .to_string()
            .into_boxed_str(),
    );
    let (port, _rx) = common::fake_stash(vec![
        r#"{"data":{"version":{"version":"v0.31.1"}}}"#,
        config,
    ]);
    // With the writer lock held and the database unreadable, status still describes the
    // generation: it reads only the small files.
    let (env, _) = common::run(&status_input(port, &dir));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["result"]["serverId"], SERVER);
    assert_eq!(env["result"]["graph"]["state"], "ready", "{env}");
}

#[test]
fn a_store_of_another_server_needs_a_rebuild() {
    let dir = built("other-server");
    let config = Box::leak(
        json!({"data": {"configuration": {"plugins": {"semantic-engine": {"serverId": common::VALID_ID}}}}})
            .to_string()
            .into_boxed_str(),
    );
    let (port, _rx) = common::fake_stash(vec![
        r#"{"data":{"version":{"version":"v0.31.1"}}}"#,
        config,
    ]);
    let (env, _) = common::run(&status_input(port, &dir));
    assert_eq!(
        env["result"]["graph"],
        json!({ "state": "needsRebuild", "reason": "otherServer", "pending": false }),
        "{env}"
    );
}
