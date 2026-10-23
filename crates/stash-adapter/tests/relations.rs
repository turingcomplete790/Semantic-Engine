//! Relation reads and writes against recorded responses (spec 004, tasks T009, T020, T037).

mod common;
use serde_json::Value;
use stash_adapter::relations::DEFINITION_FLAGS;
use stash_adapter::Client;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/relations")
            .join(name),
    )
    .unwrap()
}

/// Answers each flag-filtered `findTags` with that flag's recorded response.
fn tags_by_flag(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap();
    let field = v["variables"]["filter"]["custom_fields"][0]["field"]
        .as_str()
        .unwrap_or("none")
        .to_string();
    fixture(&format!("tags_{field}.json"))
}

fn client(port: u16) -> Client {
    Client::with_api_key(&format!("http://127.0.0.1:{port}"), "test-key").unwrap()
}

#[test]
fn definitions_are_read_with_one_bounded_query_per_flag() {
    let (port, rx) = common::fake_stash(DEFINITION_FLAGS.len(), tags_by_flag);
    let tags = client(port).relation_definitions().unwrap();

    let bodies: Vec<String> = rx.try_iter().collect();
    assert_eq!(bodies.len(), DEFINITION_FLAGS.len(), "one request per flag");
    for (b, flag) in bodies.iter().zip(DEFINITION_FLAGS) {
        let v: Value = serde_json::from_str(b).unwrap();
        let c = &v["variables"]["filter"]["custom_fields"][0];
        assert_eq!(c["field"], flag);
        assert_eq!(c["modifier"], "NOT_NULL", "every read is flag-filtered");
    }

    // Merged by id, each tag once, in id order.
    let ids: Vec<&str> = tags.iter().map(|t| t.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_by_key(|s| s.parse::<u64>().unwrap());
    sorted.dedup();
    assert_eq!(ids, sorted);
    for id in ["21", "22", "30", "31", "40", "41", "45", "50", "60", "75"] {
        assert!(ids.contains(&id), "tag {id} read");
    }
    // The non-semantic browse parent is not fetched as a tag of its own.
    assert!(!ids.contains(&"90"));

    let nurse = tags.iter().find(|t| t.id == "33").unwrap();
    assert_eq!(nurse.name, "Nurse");
    assert_eq!(nurse.parents, vec!["32".to_string(), "90".to_string()]);
    let extra = tags.iter().find(|t| t.id == "41").unwrap();
    assert!(extra.custom_fields["relationEnds"].is_string());
    let variant = tags.iter().find(|t| t.id == "22").unwrap();
    assert_eq!(variant.custom_fields["variantOfTagId"], "21");
}

/// Routes each request to its recorded response by operation and variables.
fn route(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap();
    let q = v["query"].as_str().unwrap();
    let vars = &v["variables"];
    let name = if q.contains("sceneUpdate") {
        "update_scene.json"
    } else if q.contains("runPluginTask") {
        "run_plugin_task.json"
    } else if q.contains("findPerformers") {
        "performers.json"
    } else if q.contains("findImages") {
        "items_images.json"
    } else if q.contains("findScenes") {
        if !vars["filter"]["updated_at"].is_null() {
            "changed_since_scenes.json"
        } else if !vars["filter"]["custom_fields"].is_null() {
            "ids_with_data_scenes.json"
        } else if !vars["ids"].is_null() {
            "items_scenes.json"
        } else {
            "latest_scene.json"
        }
    } else {
        panic!("unexpected request {q}")
    };
    fixture(name)
}

fn one(f: impl FnOnce(&Client)) -> Value {
    let (port, rx) = common::fake_stash(1, route);
    f(&client(port));
    serde_json::from_str(&rx.recv().unwrap()).unwrap()
}

#[test]
fn items_are_read_by_ids_with_their_custom_fields() {
    use stash_adapter::relations::MediaKind;
    let mut items = Vec::new();
    let sent = one(|c| {
        items = c
            .items_by_ids(MediaKind::Scene, &["101".into(), "104".into()])
            .unwrap()
    });
    assert_eq!(sent["variables"]["ids"], serde_json::json!(["101", "104"]));
    let s = items.iter().find(|i| i.id == "101").unwrap();
    assert_eq!(s.kind, MediaKind::Scene);
    assert_eq!(s.performers, vec!["7".to_string(), "8".to_string()]);
    assert_eq!(s.updated_at, "2026-10-21T10:00:00-06:00");
    assert!(s.custom_fields["actionPairings"].is_string());
}

#[test]
fn performer_tags_are_read_by_ids() {
    let mut ps = Vec::new();
    let sent = one(|c| ps = c.performer_tags(&["7".into(), "14".into()]).unwrap());
    assert_eq!(sent["variables"]["ids"], serde_json::json!(["7", "14"]));
    let p14 = ps.iter().find(|p| p.id == "14").unwrap();
    assert_eq!(p14.tags, vec!["50".to_string(), "51".to_string()]);
}

#[test]
fn changed_items_use_the_threshold_text_and_never_ids() {
    use stash_adapter::relations::MediaKind;
    let mut ids = Vec::new();
    let sent = one(|c| {
        ids = c
            .changed_since(MediaKind::Scene, "2026-10-21T09:59:59-06:00")
            .unwrap()
    });
    let f = &sent["variables"]["filter"]["updated_at"];
    assert_eq!(f["value"], "2026-10-21T09:59:59-06:00");
    assert_eq!(f["modifier"], "GREATER_THAN");
    assert!(
        sent["variables"]["ids"].is_null(),
        "the filter is ignored when ids are given"
    );
    assert_eq!(ids, vec!["101".to_string(), "110".to_string()]);
}

#[test]
fn the_latest_update_time_is_read_as_stored() {
    use stash_adapter::relations::MediaKind;
    let mut t = None;
    let sent = one(|c| t = c.latest_updated_at(MediaKind::Scene).unwrap());
    assert!(sent["query"].as_str().unwrap().contains("updated_at"));
    assert_eq!(t.as_deref(), Some("2026-10-21T10:00:00-06:00"));
}

#[test]
fn items_with_relation_data_are_found_by_flag_filters() {
    use stash_adapter::relations::MediaKind;
    let (port, rx) = common::fake_stash(2, route);
    let ids = client(port)
        .ids_with_relation_data(MediaKind::Scene)
        .unwrap();
    let fields: Vec<String> = rx
        .try_iter()
        .map(|b| {
            let v: Value = serde_json::from_str(&b).unwrap();
            let c = &v["variables"]["filter"]["custom_fields"][0];
            assert_eq!(c["modifier"], "NOT_NULL");
            c["field"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(fields, ["actionPairings", "relationAssignments"]);
    assert_eq!(
        ids,
        vec!["101".to_string(), "104".to_string(), "112".to_string()]
    );
}

#[test]
fn an_update_sends_a_partial_custom_fields_patch_only() {
    use stash_adapter::relations::MediaKind;
    let mut patch = serde_json::Map::new();
    patch.insert("relationAssignments".into(), "[]".into());
    let sent = one(|c| {
        c.update_item_custom_fields(MediaKind::Scene, "101", patch.clone())
            .unwrap()
    });
    let input = &sent["variables"]["input"];
    assert_eq!(input["id"], "101");
    assert_eq!(
        input["custom_fields"]["partial"]["relationAssignments"],
        "[]"
    );
    assert!(
        input["custom_fields"].get("full").is_none(),
        "never replaces the whole map"
    );
    assert_eq!(
        input.as_object().unwrap().len(),
        2,
        "only id and custom_fields: {input}"
    );
}

#[test]
fn a_plugin_task_is_started_by_name() {
    let mut job = String::new();
    let sent = one(|c| {
        job = c
            .run_plugin_task("semantic-engine", "Apply relations")
            .unwrap()
    });
    let vars: Vec<&Value> = sent["variables"].as_object().unwrap().values().collect();
    assert!(vars.contains(&&Value::from("semantic-engine")), "{sent}");
    assert!(vars.contains(&&Value::from("Apply relations")), "{sent}");
    assert!(sent["query"].as_str().unwrap().contains("runPluginTask"));
    assert_eq!(job, "57");
}

fn native_route(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap();
    if v["variables"]["id"] == "101" {
        fixture("native_tags_scene.json")
    } else {
        fixture("native_tags_missing.json")
    }
}

#[test]
fn an_items_native_tags_are_read_and_a_missing_item_is_none() {
    use stash_adapter::relations::MediaKind;
    let (port, rx) = common::fake_stash(2, native_route);
    let c = client(port);
    assert_eq!(
        c.item_native_tags(MediaKind::Scene, "101").unwrap(),
        Some(vec![
            "21".to_string(),
            "22".to_string(),
            "45".to_string(),
            "999".to_string()
        ])
    );
    assert_eq!(c.item_native_tags(MediaKind::Scene, "404").unwrap(), None);
    let first: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
    assert!(first["query"].as_str().unwrap().contains("findScene"));
}
