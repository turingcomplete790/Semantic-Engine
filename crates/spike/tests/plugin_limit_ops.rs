//! `payload`, `exportSnapshot`, `snapshotInfo`, and `applyEdit` ops on a local plugin folder
//! (tasks T035, T037, T050).

use base64::Engine as _;
use serde_json::{json, Value};
use spike::plugin_ops;
use std::io::Read;

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
    let d = std::env::temp_dir().join(format!("spike-limit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d.to_string_lossy().into_owned()
}

fn gunzip_lines(data: &[u8]) -> usize {
    let mut text = String::new();
    flate2::read::GzDecoder::new(data)
        .read_to_string(&mut text)
        .unwrap();
    text.lines().filter(|l| !l.is_empty()).count()
}

#[test]
fn payload_is_about_the_requested_size() {
    let dir = tmp("payload");
    for bytes in [1024u64, 64 * 1024, 1024 * 1024] {
        let r = call(&dir, json!({"op": "payload", "bytes": bytes}));
        assert_eq!(r["ok"], true, "{r}");
        let size = r["result"]["bindings"].to_string().len() as u64;
        assert!(size >= bytes, "{size} >= {bytes}");
        assert!(
            size < bytes + 200,
            "{size} is within one binding of {bytes}"
        );
        let first = &r["result"]["bindings"][0];
        assert!(first["item"]
            .as_str()
            .unwrap()
            .starts_with("urn:semantic-stash:"));
        assert!(first["label"].is_string());
    }
    let r = call(&dir, json!({"op": "payload"}));
    assert_eq!(r["error"]["code"], "INVALID_INPUT");
    let r = call(&dir, json!({"op": "payload", "bytes": 1u64 << 40}));
    assert_eq!(r["error"]["code"], "INVALID_INPUT");
}

#[test]
fn export_to_output_returns_every_quad_of_the_current_generation() {
    let dir = tmp("output");
    let w = call(
        &dir,
        json!({"op": "write", "option": "B", "store": "s", "scale": "200"}),
    );
    assert_eq!(w["ok"], true, "{w}");
    let triples = w["result"]["triples"].as_u64().unwrap();

    let r = call(&dir, json!({"op": "exportSnapshot", "store": "s"}));
    assert_eq!(r["ok"], true, "{r}");
    let m = &r["result"]["manifest"];
    assert_eq!(m["generation"], 1);
    assert_eq!(m["store"], "s");
    assert!(m["builtAt"].is_string());
    assert!(m["vocabularyVersion"].is_string() && m["engineVersion"].is_string());
    let data = base64::engine::general_purpose::STANDARD
        .decode(r["result"]["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(m["bytes"], data.len() as u64);
    let lines = gunzip_lines(&data) as u64;
    assert_eq!(lines, m["quads"].as_u64().unwrap());
    // The write adds its start and end markers on top of the synthetic triples.
    assert!(lines >= triples, "{lines} >= {triples}");

    // With no asset exported, a client of the op path still learns the current generation.
    let i = call(&dir, json!({"op": "snapshotInfo", "store": "s"}));
    assert_eq!(i["ok"], true, "{i}");
    assert_eq!(i["result"]["manifest"], Value::Null);
    assert_eq!(i["result"]["currentGeneration"], 1);
    assert_eq!(i["result"]["assetIsStale"], false);
}

#[test]
fn export_to_asset_writes_the_served_files_and_info_tracks_staleness() {
    let dir = tmp("asset");
    let w = |n: u64| {
        let w = call(
            &dir,
            json!({"op": "write", "option": "B", "store": "s", "scale": "50", "seed": n}),
        );
        assert_eq!(w["ok"], true, "{w}");
    };
    w(1);
    let r = call(
        &dir,
        json!({"op": "exportSnapshot", "store": "s", "to": "asset"}),
    );
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["result"]["path"], "snapshot.nq.gz");
    assert!(r["result"].get("data").is_none());
    let assets = std::path::Path::new(&dir).join("store").join("assets");
    let data = std::fs::read(assets.join("snapshot.nq.gz")).unwrap();
    assert_eq!(
        gunzip_lines(&data) as u64,
        r["result"]["manifest"]["quads"].as_u64().unwrap()
    );
    let on_disk: Value =
        serde_json::from_str(&std::fs::read_to_string(assets.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(on_disk, r["result"]["manifest"]);

    let i = call(&dir, json!({"op": "snapshotInfo"}));
    assert_eq!(i["ok"], true, "{i}");
    assert_eq!(i["result"]["manifest"]["generation"], 1);
    assert_eq!(i["result"]["assetIsStale"], false);

    // A new generation makes the exported file stale until it is exported again.
    w(2);
    let i = call(&dir, json!({"op": "snapshotInfo"}));
    assert_eq!(i["result"]["currentGeneration"], 2);
    assert_eq!(i["result"]["assetIsStale"], true);
}

#[test]
fn snapshots_of_missing_stores_are_errors() {
    let dir = tmp("none");
    let r = call(&dir, json!({"op": "exportSnapshot", "store": "nothing"}));
    assert_eq!(r["error"]["code"], "INTERNAL");
    let r = call(&dir, json!({"op": "exportSnapshot", "to": "elsewhere"}));
    assert_eq!(r["error"]["code"], "INVALID_INPUT");
    let r = call(&dir, json!({"op": "snapshotInfo"}));
    assert_eq!(r["error"]["code"], "INTERNAL");
}

#[test]
fn a_delta_chain_turns_a_copy_of_generation_n_into_generation_n_plus_k() {
    use oxigraph::store::Store;
    use spike::plugin_ops::delta::{apply_delta, delta_path, item_quads};
    use spike::store;

    let dir = tmp("delta");
    let w = call(
        &dir,
        json!({"op": "write", "option": "B", "store": "s", "scale": "200"}),
    );
    assert_eq!(w["ok"], true, "{w}");
    let sdir = std::path::Path::new(&dir).join("store").join("s");
    let copy = std::path::Path::new(&dir).join("client-copy");
    store::copy_generation(&sdir, 1, &copy).unwrap();
    let gen1_len = store::open_current_read_only(&sdir)
        .unwrap()
        .1
        .len()
        .unwrap();

    let mut touched = Vec::new();
    for (items, seed) in [(3, 1), (5, 2)] {
        let e = call(
            &dir,
            json!({"op": "applyEdit", "store": "s", "items": items, "seed": seed}),
        );
        assert_eq!(e["ok"], true, "{e}");
        let r = &e["result"];
        assert_eq!(r["to"].as_u64(), Some(r["from"].as_u64().unwrap() + 1));
        assert!(r["removed"].as_u64().unwrap() > 0 && r["added"].as_u64().unwrap() > 0);
        assert_eq!(r["scenes"].as_array().unwrap().len(), items);
        touched.extend(
            r["scenes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_u64().unwrap()),
        );
    }

    let client = Store::open(&copy).unwrap();
    for to in [2, 3] {
        apply_delta(&client, &delta_path(&sdir, to)).unwrap();
    }
    let (g, latest) = store::open_current_read_only(&sdir).unwrap();
    assert_eq!(g, 3);
    assert_eq!(client.len().unwrap(), latest.len().unwrap());
    let iris = semantic_vocabulary::iri::ItemIris::new(spike::synth::SERVER_ID).unwrap();
    for s in touched {
        let scene = iris.scene(&s.to_string());
        let (a, b) = (
            item_quads(&client, &scene).unwrap(),
            item_quads(&latest, &scene).unwrap(),
        );
        assert!(!a.is_empty());
        assert_eq!(a, b, "scene {s}");
    }

    // Writing the client's copy never changes the published generation it came from.
    drop(client);
    let gen1 = Store::open_read_only(store::snapshots_dir(&sdir).join("1")).unwrap();
    assert_eq!(gen1.len().unwrap(), gen1_len);
}

#[test]
fn bad_edits_and_levels_are_refused() {
    let dir = tmp("edit-bad");
    let r = call(&dir, json!({"op": "applyEdit", "store": "nothing"}));
    assert_eq!(r["error"]["code"], "INTERNAL");
    let w = call(
        &dir,
        json!({"op": "write", "option": "B", "store": "s", "scale": "20"}),
    );
    assert_eq!(w["ok"], true, "{w}");
    let r = call(&dir, json!({"op": "applyEdit", "store": "s", "items": 0}));
    assert_eq!(r["error"]["code"], "INVALID_INPUT");
    let r = call(
        &dir,
        json!({"op": "exportSnapshot", "store": "s", "level": 10}),
    );
    assert_eq!(r["error"]["code"], "INVALID_INPUT");
    let r = call(
        &dir,
        json!({"op": "exportSnapshot", "store": "s", "level": 1}),
    );
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["result"]["gzipLevel"], 1);
}
