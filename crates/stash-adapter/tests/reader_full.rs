//! A full read of the semantic library (spec 005 T012; FR-001 to FR-008, FR-002a, FR-002b,
//! FR-013, SC-001), against a fake Stash serving `fixtures/reader/library.json`.

mod common;

use common::{client, library, library_handler, root_field, serve};
use serde_json::{json, Value};
use stash_adapter::reader::field::{Field, GraphPosition};
use stash_adapter::reader::keys::NOT_READ;
use stash_adapter::reader::{Kind, LibraryRead, LibraryReader, TagFlag};
use std::sync::mpsc::Receiver;

fn read_with(
    handler: impl Fn(&str) -> (u16, String) + Send + Sync + 'static,
) -> (LibraryRead, Vec<String>) {
    let (port, rx) = serve(handler);
    let read = client(port)
        .full_read(&|| false, &mut |_| {})
        .expect("full read");
    (read, drain(&rx))
}

fn drain(rx: &Receiver<String>) -> Vec<String> {
    rx.try_iter().collect()
}

fn ids<T>(list: &[T], id: impl Fn(&T) -> &str) -> Vec<String> {
    list.iter().map(|x| id(x).to_string()).collect()
}

fn full() -> (LibraryRead, Vec<String>) {
    read_with(library_handler(library("library.json")))
}

#[test]
fn every_semantic_tag_and_no_other() {
    let (read, _) = full();
    assert_eq!(ids(&read.tags, |t| &t.id), ["1", "2", "3", "4", "5", "6"]);
    let by = |id: &str| read.tags.iter().find(|t| t.id == id).unwrap();
    assert_eq!(by("1").flags, [TagFlag::Action].into());
    assert!(by("2").flags.is_empty(), "a variant may carry no flag");
    assert_eq!(by("2").fields["variantOfTagId"], Field::Value(json!("1")));
    assert_eq!(by("3").parents, ["8"]);
    assert_eq!(
        by("3").fields["fieldMapping"],
        Field::Value(json!({"base": {"field": "hair_color", "priority": 1}}))
    );
    assert_eq!(by("4").flags, [TagFlag::Label].into());
    assert_eq!(by("5").flags, [TagFlag::Role].into());
    assert_eq!(by("6").flags, [TagFlag::Variation].into());
    assert_eq!(by("1").name, "Kissing");
    assert_eq!(by("1").updated_at, "2026-10-20T09:00:00-06:00");
}

#[test]
fn every_semantic_performer_with_display_data_and_measurements() {
    let (read, _) = full();
    // 13 is only referenced by a scene; 15 has isArchetype "0" and nothing else.
    assert_eq!(ids(&read.performers, |p| &p.id), ["10", "11", "12", "14"]);
    let by = |id: &str| read.performers.iter().find(|p| p.id == id).unwrap();

    let arch = by("10");
    assert!(arch.archetype);
    assert_eq!(arch.archetype_parents, Some(Field::Value(json!(["12"]))));
    assert_eq!(
        arch.display.as_ref().unwrap().graph_position,
        Some(GraphPosition { x: 10.0, y: -5.5 })
    );

    assert_eq!(by("11").tags, ["3", "7"]);
    assert_eq!(
        by("11").measurements,
        None,
        "an empty measurements field is absent"
    );

    let m = by("12").measurements.as_ref().unwrap();
    assert_eq!(m.raw, "34D-26-35");
    assert_eq!(
        (m.bust_size, m.waist, m.hip),
        (Some(55.5), Some(26), Some(35))
    );
    assert_eq!(m.waist_to_hip_ratio, Some(0.74));

    let labelled = by("14");
    assert_eq!(labelled.display.as_ref().unwrap().graph_position, None);
    let m = labelled.measurements.as_ref().unwrap();
    assert_eq!((m.raw.as_str(), m.bust_size), ("natural", None));
}

#[test]
fn keys_that_are_never_read_are_never_returned() {
    let (read, _) = full();
    let text = serde_json::to_string(&read).unwrap();
    for key in NOT_READ {
        assert!(!text.contains(key), "{key} leaked into the read");
    }
    assert!(
        !text.contains("traitPicks"),
        "archetypeRecipe content leaked"
    );
}

#[test]
fn every_semantic_scene_and_image() {
    let (read, _) = full();
    assert_eq!(ids(&read.scenes, |s| &s.id), ["20", "21", "23"]);
    assert_eq!(ids(&read.images, |i| &i.id), ["30", "31"]);
    let s20 = &read.scenes[0];
    assert_eq!(s20.performers, ["11", "13"]);
    assert_eq!(s20.tags, ["7"]);
    assert_eq!(s20.updated_at, "2026-10-22T10:00:00-06:00");
    assert_eq!(
        s20.fields["roleTagAssignments"],
        Field::Value(json!({"11": ["5"]}))
    );
    assert_eq!(s20.fields["timeline"], Field::Value(json!([])));

    let s21 = &read.scenes[1];
    assert!(matches!(
        s21.fields["actionAssignments"],
        Field::Unreadable { .. }
    ));
    assert_eq!(
        s21.fields["roleTagAssignments"],
        Field::Value(json!({"13": ["5"]}))
    );
    let s23 = &read.scenes[2];
    match &s23.fields["relationAssignments"] {
        Field::Unreadable { raw, .. } => assert_eq!(raw, &json!("{\"not\":\"a list\"}")),
        other => panic!("{other:?}"),
    }
    assert!(!s20.fields.contains_key("note"));
}

#[test]
fn the_start_is_taken_before_discovery() {
    let (read, bodies) = full();
    assert_eq!(read.start[&Kind::Tag], "2026-10-23T12:00:00-06:00");
    assert_eq!(read.start[&Kind::Performer], "2026-10-22T08:00:04-06:00");
    assert_eq!(read.start[&Kind::Scene], "2026-10-24T10:27:55-06:00");
    assert_eq!(read.start[&Kind::Image], "2026-10-22T11:00:02-06:00");
    let first_filtered = bodies
        .iter()
        .position(|b| variables(b).values().any(|v| !v.is_null()))
        .unwrap();
    assert_eq!(
        first_filtered, 5,
        "the version, then four latest-updated_at requests"
    );
}

fn variables(body: &str) -> serde_json::Map<String, Value> {
    let v: Value = serde_json::from_str(body).unwrap();
    v["variables"].as_object().cloned().unwrap_or_default()
}

#[test]
fn the_request_count_is_fixed_by_the_key_table() {
    let (read, bodies) = full();
    // The version check, then 4 starts + 9 tag keys + 2 performer keys + measurements + 1 tag
    // chunk + 16 media keys, then one record batch per kind.
    assert_eq!(root_field(&bodies[0]), "version");
    assert_eq!(bodies.len(), 1 + 4 + 9 + 2 + 1 + 1 + 16 + 4);
    assert_eq!(
        read.requests,
        bodies.len() - 1,
        "requests counts the read's own requests"
    );
}

#[test]
fn every_discovery_request_is_filtered() {
    let (_, bodies) = full();
    for b in bodies.iter().skip(5) {
        let vars = variables(b);
        let filter = vars.values().find(|v| v.is_object());
        let ids = vars.values().find(|v| v.is_array());
        match (filter, ids) {
            (Some(f), None) => {
                let ok = f["custom_fields"][0]["modifier"] == "NOT_NULL"
                    || f["tags"]["modifier"] == "INCLUDES"
                    || f["measurements"]["modifier"] == "NOT_NULL";
                assert!(ok, "unbounded discovery: {b}");
            }
            (None, Some(ids)) => assert!(!ids.as_array().unwrap().is_empty()),
            _ => panic!("neither filtered nor by ids: {b}"),
        }
    }
}

#[test]
fn an_item_found_twice_is_returned_once_and_a_vanished_one_is_missing() {
    // Scene 20 matches three keys; scene 21 disappears between discovery and its batch.
    let inner = library_handler(library("library.json"));
    let (read, _) = read_with(move |body| {
        let (status, reply) = inner(body);
        if root_field(body) == "findScenes" && variables(body).values().any(|v| v.is_array()) {
            let mut v: Value = serde_json::from_str(&reply).unwrap();
            v["data"]["findScenes"]["scenes"]
                .as_array_mut()
                .unwrap()
                .retain(|s| s["id"] != "21");
            return (status, v.to_string());
        }
        (status, reply)
    });
    assert_eq!(ids(&read.scenes, |s| &s.id), ["20", "23"]);
    assert_eq!(read.missing.get(&Kind::Scene), Some(&1));
    assert_eq!(read.missing.get(&Kind::Image), None);
}

#[test]
fn an_empty_library_is_an_empty_success() {
    let (read, _) = read_with(library_handler(json!({"version": "v0.31.1"})));
    assert!(read.tags.is_empty() && read.performers.is_empty());
    assert!(read.scenes.is_empty() && read.images.is_empty());
    assert!(read.start.is_empty());
}
