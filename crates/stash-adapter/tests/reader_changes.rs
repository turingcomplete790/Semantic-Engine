//! Change reads, record reads, id listings, and the newest `updated_at` (spec 005 T019;
//! FR-009 to FR-012).

mod common;

use common::{client, library, library_handler, root_field, serve};
use serde_json::{json, Value};
use stash_adapter::reader::field::Field;
use stash_adapter::reader::{Kind, LibraryReader, MediaKind};

fn reader() -> (stash_adapter::Client, std::sync::mpsc::Receiver<String>) {
    let (port, rx) = serve(library_handler(library("library.json")));
    (client(port), rx)
}

fn variables(body: &str) -> serde_json::Map<String, Value> {
    let v: Value = serde_json::from_str(body).unwrap();
    v["variables"].as_object().cloned().unwrap_or_default()
}

const THRESHOLD: &str = "2026-10-22T00:00:00-06:00";

#[test]
fn changes_since_a_threshold_for_every_kind() {
    let (c, rx) = reader();
    assert_eq!(
        LibraryReader::changed_since(&c, Kind::Tag, THRESHOLD).unwrap(),
        ["9"]
    );
    assert_eq!(
        LibraryReader::changed_since(&c, Kind::Performer, THRESHOLD).unwrap(),
        ["14"]
    );
    assert_eq!(
        LibraryReader::changed_since(&c, Kind::Scene, THRESHOLD).unwrap(),
        ["20", "21", "22", "23"]
    );
    assert_eq!(
        LibraryReader::changed_since(&c, Kind::Image, THRESHOLD).unwrap(),
        ["30", "31", "32"]
    );
    let bodies: Vec<String> = rx.try_iter().collect();
    let change_reads: Vec<&String> = bodies
        .iter()
        .filter(|b| root_field(b) != "version")
        .collect();
    assert_eq!(change_reads.len(), 4);
    for b in change_reads {
        let vars = variables(b);
        assert!(vars.values().all(|v| !v.is_array()), "never with ids: {b}");
        let filter = vars.values().find(|v| v.is_object()).unwrap();
        assert_eq!(
            filter["updated_at"],
            json!({"value": THRESHOLD, "modifier": "GREATER_THAN"}),
            "the threshold text exactly as given"
        );
    }
}

#[test]
fn a_change_read_is_sorted_and_without_duplicates() {
    let (c, _) = reader();
    let ids = LibraryReader::changed_since(&c, Kind::Scene, "2000-01-01T00:00:00-06:00").unwrap();
    let mut sorted = ids.clone();
    sorted.sort_by_key(|s| s.parse::<u64>().unwrap());
    sorted.dedup();
    assert_eq!(ids, sorted);
}

#[test]
fn records_of_named_ids_list_the_gone_ones_as_missing() {
    let (c, _) = reader();
    let b = c
        .media_records(MediaKind::Scene, &["21".into(), "99".into(), "20".into()])
        .unwrap();
    let found: Vec<&str> = b.found.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(found, ["20", "21"]);
    assert_eq!(b.missing, ["99"]);
    assert_eq!(
        b.found[0].fields["roleTagAssignments"],
        Field::Value(json!({"11": ["5"]}))
    );
}

#[test]
fn records_of_named_ids_apply_no_membership_rule() {
    // A tag that stopped being semantic is still returned, so a change applier can see that.
    let (c, _) = reader();
    let b = c.tag_records(&["9".into(), "7".into()]).unwrap();
    let found: Vec<&str> = b.found.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(found, ["7", "9"]);
    assert!(b.found.iter().all(|t| t.flags.is_empty()));
    let p = c.performer_records(&["13".into()]).unwrap();
    assert_eq!(p.found.len(), 1);
    assert!(p.missing.is_empty());
}

#[test]
fn every_id_of_every_kind() {
    let (c, rx) = reader();
    assert_eq!(
        LibraryReader::all_ids(&c, Kind::Tag).unwrap(),
        ["1", "2", "3", "4", "5", "6", "7", "8", "9"],
        "all tags, not only semantic ones"
    );
    assert_eq!(
        LibraryReader::all_ids(&c, Kind::Performer).unwrap(),
        ["10", "11", "12", "13", "14", "15"]
    );
    assert_eq!(
        LibraryReader::all_ids(&c, Kind::Scene).unwrap(),
        ["20", "21", "22", "23"]
    );
    assert_eq!(
        LibraryReader::all_ids(&c, Kind::Image).unwrap(),
        ["30", "31", "32"]
    );
    for b in rx.try_iter().filter(|b| root_field(b) != "version") {
        assert!(
            variables(&b).values().all(Value::is_null),
            "an id listing has no filter: {b}"
        );
        let q: Value = serde_json::from_str(&b).unwrap();
        let q = q["query"].as_str().unwrap();
        assert!(!q.contains("custom_fields"), "ids only: {q}");
    }
}

#[test]
fn the_newest_updated_at_of_every_kind() {
    let (c, _) = reader();
    assert_eq!(
        LibraryReader::latest_updated_at(&c, Kind::Tag)
            .unwrap()
            .as_deref(),
        Some("2026-10-23T12:00:00-06:00")
    );
    assert_eq!(
        LibraryReader::latest_updated_at(&c, Kind::Performer)
            .unwrap()
            .as_deref(),
        Some("2026-10-22T08:00:04-06:00")
    );
    assert_eq!(
        LibraryReader::latest_updated_at(&c, Kind::Scene)
            .unwrap()
            .as_deref(),
        Some("2026-10-24T10:27:55-06:00")
    );
    assert_eq!(
        LibraryReader::latest_updated_at(&c, Kind::Image)
            .unwrap()
            .as_deref(),
        Some("2026-10-22T11:00:02-06:00")
    );
}

#[test]
fn an_empty_kind_has_no_newest_time() {
    let (port, _) = serve(library_handler(json!({"version": "v0.31.1"})));
    assert_eq!(
        LibraryReader::latest_updated_at(&client(port), Kind::Image).unwrap(),
        None
    );
}
