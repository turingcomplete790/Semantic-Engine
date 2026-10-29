//! Contract test for the `query` operation (spec 009 T018; contracts/operations-1.4.md).

mod common;

use engine::build::{run, BuildOptions};
use engine::graph::GraphStore;
use serde_json::{json, Value};
use stash_adapter::reader::{LibraryRead, MemoryReader};
use std::path::{Path, PathBuf};

/// The example server id of the compatibility fixture (spec 007).
const SERVER: &str = "00000000-0000-4000-8000-000000000001";

fn fixture_library() -> LibraryRead {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../engine/tests/fixtures/build/library.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn plugin_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("se-ops-query-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A plugin folder whose store holds the compatibility fixture.
fn built(name: &str) -> PathBuf {
    let dir = plugin_dir(name);
    let store = GraphStore::new(dir.join("graph"), SERVER);
    run(
        &MemoryReader::new(fixture_library()),
        &store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
    dir
}

/// The plugin input for `query`, pointing at a closed port: the operation must not call Stash.
fn input(dir: &Path, args: Value) -> Vec<u8> {
    let mut args = args;
    args["op"] = json!("query");
    json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": common::dead_port(),
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": dir, "PluginDir": dir
        },
        "args": args
    })
    .to_string()
    .into_bytes()
}

fn query(dir: &Path, args: Value) -> Value {
    let (env, log) = common::run(&input(dir, args));
    assert!(!log.contains("cookie-secret"));
    env
}

fn error(env: &Value, code: &str) -> Value {
    assert_eq!(env["ok"], false, "{env}");
    assert_eq!(env["error"]["code"], code, "{env}");
    env["error"].clone()
}

#[test]
fn an_answer_has_the_contracts_envelope() {
    let dir = built("envelope");
    let env = query(
        &dir,
        json!({ "query": "SELECT ?scene ?p WHERE { ?scene :hasPerformer ?p . ?p :hasRole :Dom }" }),
    );
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["op"], "query");
    assert_eq!(env["contractVersion"], engine::CONTRACT_VERSION);
    let r = &env["result"];
    assert_eq!(r["variables"], json!(["scene", "p"]));
    let b = r["bindings"].as_array().unwrap();
    assert_eq!(b.len(), 3);
    let scene_100 = b
        .iter()
        .find(|x| x["scene"]["id"] == "100")
        .unwrap_or_else(|| panic!("{r}"));
    assert_eq!(
        scene_100["scene"],
        json!({ "type": "item", "kind": "scene", "id": "100",
                "iri": format!("urn:semantic-stash:{SERVER}:scene:100") })
    );
    assert_eq!(scene_100["p"]["kind"], "performer");
    assert_eq!(
        r["page"],
        json!({ "count": 3, "offset": 0, "continuation": null })
    );
    assert_eq!(r["graph"]["generation"], 1);
    assert!(r["graph"]["publishedAt"].as_str().unwrap().ends_with('Z'));
    assert_eq!(r["graph"]["pending"], false);
    assert!(r["seconds"].as_f64().unwrap() >= 0.0);

    // A save recorded since the last update (spec 009 research R8).
    std::fs::write(dir.join("graph.pending"), "").unwrap();
    let env = query(
        &dir,
        json!({ "query": "SELECT ?m WHERE { ?m a sem:Image }" }),
    );
    assert_eq!(env["result"]["graph"]["pending"], true, "{env}");
}

#[test]
fn bad_arguments_are_invalid_input() {
    let dir = built("arguments");
    let q = "SELECT ?s WHERE { ?s ?p ?o }";
    for args in [
        json!({}),
        json!({ "query": 5 }),
        json!({ "query": "  " }),
        json!({ "query": q, "pageSize": 0 }),
        json!({ "query": q, "pageSize": 20001 }),
        json!({ "query": q, "pageSize": "10" }),
        json!({ "query": q, "limit": 0 }),
        json!({ "query": q, "continuation": 7 }),
        json!({ "query": q, "contract": "one" }),
    ] {
        let env = query(&dir, args.clone());
        error(&env, "INVALID_INPUT");
        let _ = args;
    }
}

#[test]
fn query_errors_carry_their_position() {
    let dir = built("errors");
    for (text, code, line, column) in [
        ("SELECT ?s WHERE { ?s ?p ?o", "QUERY_SYNTAX", 1, 27),
        (
            "SELECT ?s WHERE { ?s ?p ?o OPTIONAL { ?s ?q ?r } }",
            "QUERY_UNSUPPORTED",
            1,
            28,
        ),
        (
            "SELECT ?s WHERE { ?s sem:ofTag :Nobody }",
            "QUERY_UNKNOWN",
            1,
            32,
        ),
        (
            "SELECT ?a WHERE { ?a :hasPerformer ?p . ?b :hasPerformer ?p . ?p :hasRole :Dom }",
            "QUERY_SCOPE_AMBIGUOUS",
            1,
            63,
        ),
        (
            "SELECT ?p WHERE { ?p :hasTrait recursive:Hair%20Colour }",
            "QUERY_NOT_RECURSIVE",
            1,
            32,
        ),
    ] {
        let env = query(&dir, json!({ "query": text }));
        let e = error(&env, code);
        assert_eq!(e["details"]["line"], line, "{text}: {e}");
        assert_eq!(e["details"]["column"], column, "{text}: {e}");
        assert!(!e["message"].as_str().unwrap().is_empty());
    }
    let env = query(&dir, json!({ "query": "SELECT ?s WHERE { ?s :a/:b ?o }" }));
    assert_eq!(
        error(&env, "QUERY_UNSUPPORTED")["details"]["construct"],
        "property paths"
    );
}

#[test]
fn no_graph_and_a_graph_to_rebuild() {
    let dir = plugin_dir("not-built");
    let env = query(&dir, json!({ "query": "SELECT ?s WHERE { ?s ?p ?o }" }));
    let e = error(&env, "GRAPH_NOT_BUILT");
    assert!(e["message"]
        .as_str()
        .unwrap()
        .contains("Build semantic graph"));

    let dir = built("rebuild");
    let stamp = dir.join("graph/store.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&stamp).unwrap()).unwrap();
    v["format"] = json!(1);
    std::fs::write(&stamp, v.to_string()).unwrap();
    let env = query(&dir, json!({ "query": "SELECT ?s WHERE { ?s ?p ?o }" }));
    assert_eq!(
        error(&env, "GRAPH_NEEDS_REBUILD")["details"]["reason"],
        "otherFormat"
    );
}

#[test]
fn the_contract_version_a_client_states() {
    let dir = built("contract");
    let q = "SELECT ?m WHERE { ?m a sem:Image }";
    let env = query(&dir, json!({ "query": q, "contract": "2.0.0" }));
    let e = error(&env, "CONTRACT_MISMATCH");
    assert_eq!(e["details"]["engine"], engine::CONTRACT_VERSION);
    assert_eq!(e["details"]["client"], "2.0.0");
    for ok in ["1.0.0", "1.9.0", engine::CONTRACT_VERSION] {
        let env = query(&dir, json!({ "query": q, "contract": ok }));
        assert_eq!(env["ok"], true, "{ok}: {env}");
    }
}

#[test]
fn a_query_past_the_time_limit_is_stopped() {
    let dir = built("timeout");
    // Every pair of facts: slow enough to pass a 1 ms limit.
    let stdin = input(
        &dir,
        json!({ "query": "SELECT ?a ?b ?c ?d WHERE { ?a ?p ?b . ?c ?q ?d }" }),
    );
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_semantic-engine"))
        .env("SEMANTIC_ENGINE_QUERY_TIME_LIMIT_MS", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(&stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    assert!(v["error"].is_null());
    let e = error(&v["output"], "QUERY_TIMEOUT");
    assert!(e["message"].as_str().unwrap().contains("limit"), "{e}");
}

// ---- Paging through the operation (spec 009 T035)

#[test]
fn a_two_page_answer_and_its_continuation_errors() {
    let dir = built("paging");
    let q = "SELECT ?scene ?p WHERE { ?scene :hasPerformer ?p . ?p :hasRole :Dom }";
    let size = |env: &Value| env.to_string().len();
    let first = query(&dir, json!({ "query": q, "pageSize": 2 }));
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(first["result"]["page"]["count"], 2);
    assert_eq!(first["result"]["page"]["offset"], 0);
    let c = first["result"]["page"]["continuation"]
        .as_str()
        .unwrap_or_else(|| panic!("{first}"))
        .to_string();
    let second = query(
        &dir,
        json!({ "query": q, "pageSize": 2, "continuation": c }),
    );
    assert_eq!(second["ok"], true, "{second}");
    assert_eq!(second["result"]["page"]["count"], 1);
    assert_eq!(second["result"]["page"]["offset"], 2);
    assert_eq!(second["result"]["page"]["continuation"], Value::Null);
    assert_eq!(
        second["result"]["graph"]["generation"],
        first["result"]["graph"]["generation"]
    );
    let mut all: Vec<Value> = first["result"]["bindings"].as_array().unwrap().clone();
    all.extend(
        second["result"]["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    let whole = query(&dir, json!({ "query": q }));
    assert_eq!(Value::Array(all), whole["result"]["bindings"]);
    for env in [&first, &second, &whole] {
        assert!(size(env) < 2 * 1_048_576);
    }

    for bad in ["garbage", "", &c[..c.len() - 2]] {
        let env = query(&dir, json!({ "query": q, "continuation": bad }));
        error(&env, "PAGE_INVALID");
    }
    let env = query(
        &dir,
        json!({ "query": "SELECT ?m WHERE { ?m a sem:Image }", "continuation": c }),
    );
    error(&env, "PAGE_INVALID");

    let generation = first["result"]["graph"]["generation"].as_u64().unwrap();
    std::fs::remove_dir_all(dir.join("graph/snapshots").join(generation.to_string())).unwrap();
    let env = query(&dir, json!({ "query": q, "continuation": c }));
    assert_eq!(
        error(&env, "PAGE_EXPIRED")["details"]["generation"],
        generation
    );
}
