//! Seed manifest (data-model "Seed manifest"; tasks T016), against a fake Stash.

#[path = "../../stash-adapter/tests/common/mod.rs"]
mod common;

use serde_json::{json, Value};
use spike::bench::seed::{
    self, Changed, CreatedTag, SeedError, SeedManifest, SeedState, StatsRecord,
};
use stash_adapter::spike::Kind;
use stash_adapter::Client;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const SERVER: &str = "http://localhost:9998";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("spike-seed-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d.join("seed-manifest.json")
}

fn client(port: u16) -> Client {
    Client::with_api_key(&format!("http://127.0.0.1:{port}"), "k").unwrap()
}

fn query(body: &str) -> (String, Value) {
    let v: Value = serde_json::from_str(body).unwrap();
    (
        v["query"].as_str().unwrap().to_string(),
        v["variables"].clone(),
    )
}

fn data(d: Value) -> String {
    json!({ "data": d }).to_string()
}

const STATS: &str =
    r#"{"stats": {"scene_count": 3, "image_count": 2, "performer_count": 2, "tag_count": 5}}"#;

static NEXT_TAG: AtomicUsize = AtomicUsize::new(100);

/// A small library: 3 scenes (one with an existing custom field), 2 images, 2 performers.
fn library(body: &str) -> String {
    let (q, _) = query(body);
    if q.contains("stats") {
        data(serde_json::from_str(STATS).unwrap())
    } else if q.contains("tagCreate") {
        let id = NEXT_TAG.fetch_add(1, Ordering::SeqCst);
        data(json!({"tagCreate": {"id": id.to_string()}}))
    } else if q.contains("findPerformers") {
        data(json!({"findPerformers": {"count": 2, "performers": [
            {"id": "7", "custom_fields": {}}, {"id": "8", "custom_fields": {}}]}}))
    } else if q.contains("findScenes") {
        data(json!({"findScenes": {"count": 2, "scenes": [
            {"id": "1", "custom_fields": {}},
            {"id": "2", "custom_fields": {"roleTagAssignments": "{\"9\": [\"1\"]}", "note": "keep"}}]}}))
    } else if q.contains("findImages") {
        data(json!({"findImages": {"count": 1, "images": [{"id": "15", "custom_fields": {}}]}}))
    } else if q.contains("sceneUpdate") {
        data(json!({"sceneUpdate": {"id": "1"}}))
    } else if q.contains("imageUpdate") {
        data(json!({"imageUpdate": {"id": "15"}}))
    } else if q.contains("tagsDestroy") {
        data(json!({"tagsDestroy": true}))
    } else {
        json!({"errors": [{"message": format!("unexpected request: {q}")}]}).to_string()
    }
}

fn failing_tag_create(body: &str) -> String {
    let (q, _) = query(body);
    if q.contains("stats") {
        data(serde_json::from_str(STATS).unwrap())
    } else {
        json!({"errors": [{"message": "tagCreate failed"}]}).to_string()
    }
}

#[test]
fn the_manifest_is_written_as_seeding_before_the_first_write() {
    let path = tmp("first");
    let (port, _rx) = common::fake_stash(10, failing_tag_create);
    assert!(seed::seed(&client(port), &path, SERVER, 2, 1).is_err());
    let m = seed::load_manifest(&path)
        .unwrap()
        .expect("manifest on disk");
    assert_eq!(m.state, SeedState::Seeding);
    assert_eq!(
        m.stats_before,
        StatsRecord {
            scenes: 3,
            images: 2,
            performers: 2,
            tags: 5
        }
    );
    assert!(m.created_tags.is_empty());
}

#[test]
fn seeding_records_every_tag_and_item_then_marks_seeded() {
    let path = tmp("full");
    let (port, rx) = common::fake_stash(200, library);
    let m = seed::seed(&client(port), &path, SERVER, 2, 1).unwrap();
    assert_eq!(m.state, SeedState::Seeded);
    assert_eq!(m.created_tags.len(), 7 * 6);
    assert_eq!(m.changed.len(), 3);
    let s2 = m.changed.iter().find(|c| c.id == "2").unwrap();
    assert_eq!(s2.custom_fields_before["note"], "keep");
    assert_eq!(
        s2.keys,
        vec![
            "roleTagAssignments",
            "actionAssignments",
            "pluggableAssignments"
        ]
    );
    assert_eq!(seed::load_manifest(&path).unwrap().unwrap(), m);

    let bodies: Vec<String> = rx.try_iter().collect();
    let updates: Vec<Value> = bodies
        .iter()
        .map(|b| query(b))
        .filter(|(q, _)| q.contains("sceneUpdate"))
        .map(|(_, v)| v)
        .collect();
    assert_eq!(updates.len(), 2);
    let partial = &updates[0]["input"]["custom_fields"]["partial"];
    let roles: Value =
        serde_json::from_str(partial["roleTagAssignments"].as_str().unwrap()).unwrap();
    assert_eq!(
        roles.as_object().unwrap().len(),
        2,
        "two performers per scene: {roles}"
    );
    let actions: Value =
        serde_json::from_str(partial["actionAssignments"].as_str().unwrap()).unwrap();
    assert!(actions["7"][0]["actionTagId"].is_string());

    // A second seed is refused while this one is in place.
    assert!(matches!(
        seed::seed(&client(port), &path, SERVER, 2, 1),
        Err(SeedError::Refused(_))
    ));
}

#[test]
fn cleanup_restores_custom_fields_and_deletes_tags_even_mid_seed() {
    let path = tmp("cleanup");
    let mut before = serde_json::Map::new();
    before.insert("roleTagAssignments".into(), json!("{\"9\": [\"1\"]}"));
    before.insert("note".into(), json!("keep"));
    let m = SeedManifest {
        server: SERVER.into(),
        stats_before: StatsRecord {
            scenes: 3,
            images: 2,
            performers: 2,
            tags: 5,
        },
        created_tags: vec![
            CreatedTag {
                id: "101".into(),
                flag: "isRole".into(),
            },
            CreatedTag {
                id: "102".into(),
                flag: "isAction".into(),
            },
        ],
        changed: vec![Changed {
            kind: Kind::Scene,
            id: "2".into(),
            custom_fields_before: before,
            keys: vec!["roleTagAssignments".into(), "actionAssignments".into()],
        }],
        state: SeedState::Seeding,
        seeded_at: None,
        cleaned_at: None,
        stats_after_cleanup: None,
    };
    seed::save_manifest(&path, &m).unwrap();

    let (port, rx) = common::fake_stash(10, library);
    let done = seed::cleanup(&client(port), &path, SERVER).unwrap();
    assert_eq!(done.state, SeedState::Cleaned);
    assert_eq!(
        seed::load_manifest(&path).unwrap().unwrap().state,
        SeedState::Cleaned
    );

    let reqs: Vec<(String, Value)> = rx.try_iter().map(|b| query(&b)).collect();
    let (_, upd) = reqs
        .iter()
        .find(|(q, _)| q.contains("sceneUpdate"))
        .unwrap();
    let cf = &upd["input"]["custom_fields"];
    assert_eq!(upd["input"]["id"], "2");
    assert_eq!(
        cf["partial"],
        json!({"roleTagAssignments": "{\"9\": [\"1\"]}"})
    );
    assert_eq!(cf["remove"], json!(["actionAssignments"]));
    let (_, del) = reqs
        .iter()
        .find(|(q, _)| q.contains("tagsDestroy"))
        .unwrap();
    assert_eq!(del["ids"], json!(["101", "102"]));
}

#[test]
fn cleanup_refuses_another_server_or_no_manifest() {
    let path = tmp("refuse");
    let (port, _rx) = common::fake_stash(4, library);
    assert!(matches!(
        seed::cleanup(&client(port), &path, SERVER),
        Err(SeedError::Refused(_))
    ));
    let m = SeedManifest {
        server: "http://elsewhere:9999".into(),
        stats_before: StatsRecord {
            scenes: 0,
            images: 0,
            performers: 0,
            tags: 0,
        },
        created_tags: vec![],
        changed: vec![],
        state: SeedState::Seeded,
        seeded_at: None,
        cleaned_at: None,
        stats_after_cleanup: None,
    };
    seed::save_manifest(&path, &m).unwrap();
    assert!(matches!(
        seed::cleanup(&client(port), &path, SERVER),
        Err(SeedError::Refused(_))
    ));
}
