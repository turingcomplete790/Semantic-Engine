//! The minimal change to write for one item (spec 004 T017; research R5; data-model).

mod common;
use engine::relations::diff::plan;
use engine::relations::evaluate::matches;
use serde_json::{json, Map, Value};
use stash_adapter::relations::ItemRecord;

/// The patch for a fixture scene, with JSON-string values parsed back for comparison.
fn patch_for(item: &ItemRecord) -> Option<Map<String, Value>> {
    let model = common::example_model();
    let e = matches(item, &model, &common::labels()).unwrap();
    plan(item, &e, &model).unwrap().map(|p| p.partial())
}

fn parsed(p: &Map<String, Value>, key: &str) -> Value {
    serde_json::from_str(p[key].as_str().expect("values are JSON-encoded strings")).unwrap()
}

#[test]
fn a_new_match_adds_the_relation_and_its_end_roles() {
    let p = patch_for(&common::scene("101")).unwrap();
    assert_eq!(
        parsed(&p, "relationAssignments"),
        json!([{ "relationTagId": "45", "a": "7", "b": "8", "auto": true, "ruleId": "r1" }])
    );
    assert_eq!(
        parsed(&p, "roleTagAssignments"),
        json!({ "7": ["60"], "8": ["61"] })
    );
    assert_eq!(
        parsed(&p, "autoRoleAssignments"),
        json!({ "7": ["60"], "8": ["61"] })
    );
    let keys: Vec<&String> = p.keys().collect();
    assert_eq!(
        keys,
        [
            "autoRoleAssignments",
            "relationAssignments",
            "roleTagAssignments"
        ]
    );
}

#[test]
fn a_hand_applied_relation_is_left_alone_and_not_duplicated() {
    assert_eq!(patch_for(&common::scene("107")), None);
}

#[test]
fn a_role_already_held_by_hand_is_not_recorded_as_automatic() {
    let p = patch_for(&common::scene("108")).unwrap();
    assert_eq!(
        parsed(&p, "roleTagAssignments"),
        json!({ "7": ["60"], "8": ["61"] })
    );
    assert_eq!(parsed(&p, "autoRoleAssignments"), json!({ "8": ["61"] }));
}

#[test]
fn an_up_to_date_item_needs_no_write() {
    assert_eq!(patch_for(&common::scene("109")), None);
}

#[test]
fn a_stale_automatic_relation_is_removed_with_its_roles() {
    let p = patch_for(&common::scene("112")).unwrap();
    assert_eq!(parsed(&p, "relationAssignments"), json!([]));
    assert_eq!(parsed(&p, "roleTagAssignments"), json!({}));
    assert_eq!(parsed(&p, "autoRoleAssignments"), json!({}));
}

#[test]
fn several_pairs_and_both_directions_get_their_roles() {
    let p = patch_for(&common::scene("104")).unwrap();
    assert_eq!(
        parsed(&p, "roleTagAssignments"),
        json!({ "7": ["60"], "8": ["61"], "12": ["61"] })
    );
    let p = patch_for(&common::scene("103")).unwrap();
    assert_eq!(
        parsed(&p, "roleTagAssignments"),
        json!({ "14": ["60", "61"], "15": ["60", "61"] })
    );
}

#[test]
fn applying_a_patch_then_planning_again_writes_nothing() {
    for s in common::scenes() {
        let Some(p) = patch_for(&s) else { continue };
        let after = common::apply_patch(&s, &p);
        assert_eq!(patch_for(&after), None, "scene {} is not idempotent", s.id);
    }
}

#[test]
fn a_hand_entry_replaces_an_automatic_one_for_the_same_pair() {
    let mut s = common::scene("109");
    s.custom_fields.insert(
        "relationAssignments".into(),
        json!(json!([
            { "relationTagId": "45", "a": "7", "b": "8", "auto": true, "ruleId": "r1" },
            { "relationTagId": "45", "a": "7", "b": "8" }
        ])
        .to_string()),
    );
    let p = patch_for(&s).unwrap();
    assert_eq!(
        parsed(&p, "relationAssignments"),
        json!([{ "relationTagId": "45", "a": "7", "b": "8" }])
    );
    // The hand-applied relation needs no automatic roles any more.
    assert_eq!(parsed(&p, "autoRoleAssignments"), json!({}));
}

#[test]
fn unreadable_stored_data_is_never_overwritten() {
    let mut s = common::scene("101");
    s.custom_fields
        .insert("roleTagAssignments".into(), json!("{not json"));
    let model = common::example_model();
    let e = matches(&s, &model, &common::labels());
    assert!(e.is_err(), "an item whose fields cannot be read is skipped");
}
