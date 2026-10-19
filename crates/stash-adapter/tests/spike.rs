//! Spike-only operations against a fake Stash (specs/003-engine-design-spike, tasks T012).

mod common;

use serde_json::{json, Map, Value};
use stash_adapter::spike::{CustomFields, Kind, Page};
use stash_adapter::Client;

fn client(port: u16) -> Client {
    Client::with_api_key(&format!("http://127.0.0.1:{port}"), "test-key").unwrap()
}

fn reply(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap();
    let q = v["query"].as_str().unwrap();
    let data = if q.contains("tagCreate") {
        json!({"tagCreate": {"id": "101"}})
    } else if q.contains("sceneUpdate") {
        json!({"sceneUpdate": {"id": "5"}})
    } else if q.contains("findScenes") {
        json!({"findScenes": {"count": 2, "scenes": [
            {"id": "1", "custom_fields": {}},
            {"id": "2", "custom_fields": {"note": "x"}}]}})
    } else if q.contains("stats") {
        json!({"stats": {"scene_count": 10, "image_count": 11, "performer_count": 3, "tag_count": 4}})
    } else {
        return json!({"errors": [{"message": "unexpected"}]}).to_string();
    };
    json!({ "data": data }).to_string()
}

#[test]
fn tag_create_sends_the_name_and_custom_fields() {
    let (port, rx) = common::fake_stash(1, reply);
    let mut cf = Map::new();
    cf.insert("isRole".into(), json!(1));
    assert_eq!(
        client(port).tag_create("SPIKE isRole 0", cf).unwrap(),
        "101"
    );
    let body: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
    assert!(body["query"].as_str().unwrap().contains("tagCreate"));
    assert_eq!(body["variables"]["input"]["name"], "SPIKE isRole 0");
    assert_eq!(
        body["variables"]["input"]["custom_fields"],
        json!({"isRole": 1})
    );
}

#[test]
fn scene_update_sends_only_the_custom_field_parts_given() {
    let (port, rx) = common::fake_stash(1, reply);
    let mut partial = Map::new();
    partial.insert("roleTagAssignments".into(), json!("{\"7\": [\"19\"]}"));
    let cf = CustomFields {
        remove: Some(vec!["old".into()]),
        ..CustomFields::partial(partial)
    };
    client(port)
        .update_custom_fields(Kind::Scene, "5", cf)
        .unwrap();
    let body: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
    assert!(body["query"].as_str().unwrap().contains("sceneUpdate"));
    let sent = &body["variables"]["input"]["custom_fields"];
    assert_eq!(sent["partial"]["roleTagAssignments"], "{\"7\": [\"19\"]}");
    assert_eq!(sent["remove"], json!(["old"]));
    assert!(
        sent.get("full").is_none(),
        "unset parts are not sent: {sent}"
    );
}

#[test]
fn find_and_stats_parse() {
    let (port, _rx) = common::fake_stash(2, reply);
    let c = client(port);
    let (count, rows) = c
        .find_custom_fields(
            Kind::Scene,
            Some("2026-10-16T00:00:00Z"),
            None,
            Page::first(2),
        )
        .unwrap();
    assert_eq!(count, 2);
    assert_eq!(rows[1].0, "2");
    assert_eq!(rows[1].1["note"], "x");
    let s = c.stats().unwrap();
    assert_eq!(
        (s.scene_count, s.image_count, s.performer_count, s.tag_count),
        (10, 11, 3, 4)
    );
}
