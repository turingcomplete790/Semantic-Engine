//! Definitions parsed from tags (spec 004 T010; data-model "Role", "Relation", "Auto-apply rule").

mod common;
use engine::relations::defs::{Condition, Definitions, ProblemKind};
use serde_json::json;
use stash_adapter::relations::TagRecord;

fn defs() -> Definitions {
    Definitions::from_tags(&common::all_tags())
}

fn problems_of(d: &Definitions, kind: ProblemKind) -> Vec<String> {
    d.problems
        .iter()
        .filter(|p| p.kind == kind)
        .map(|p| p.relation.clone().unwrap_or_default())
        .collect()
}

#[test]
fn tag_types_come_from_their_flags() {
    let d = defs();
    for r in ["30", "31", "32", "33", "34", "60", "61", "70", "71"] {
        assert!(d.is_role(r), "{r} is a Role");
    }
    assert!(!d.is_role("90"), "an unflagged parent is not a Role");
    assert!(d.is_label("50") && d.is_label("51"));
    assert!(d.is_action("21") && d.is_action("23"));
    assert!(d.relations.contains_key("40") && d.relations.contains_key("45"));
    assert_eq!(d.variant_of("22"), Some("21"));
    assert_eq!(d.variant_of("21"), None);
}

#[test]
fn relations_have_ordered_ends_and_rules() {
    let d = defs();
    let extra = &d.relations["41"];
    assert_eq!(extra.name, "Extra Care");
    assert_eq!(extra.ends, Some(("31".to_string(), "33".to_string())));
    assert_eq!(extra.rules.len(), 1);
    assert_eq!(extra.rules[0].id, "ec1");
    assert_eq!(extra.rules[0].actions, vec!["21".to_string()]);

    let size = &d.relations["45"];
    assert_eq!(size.ends, Some(("60".to_string(), "61".to_string())));
    let r1 = &size.rules[0];
    assert_eq!(r1.a, Condition::Label("50".into()));
    assert_eq!(r1.b, Condition::Label("51".into()));
    assert!(r1.actions.is_empty(), "no actions means any Action");
}

#[test]
fn relations_missing_an_end_or_with_a_non_role_end_are_invalid() {
    let d = defs();
    assert_eq!(problems_of(&d, ProblemKind::MissingEnd), vec!["72"]);
    assert_eq!(problems_of(&d, ProblemKind::EndNotRole), vec!["73"]);
    assert!(!d.relations["72"].valid && !d.relations["73"].valid);
    assert!(d.relations["40"].valid && d.relations["45"].valid);
}

#[test]
fn bad_rules_are_reported_and_dropped_but_good_ones_kept() {
    let d = defs();
    // Rule "both" has label and role on one end; one rule has no id; a second "dup" repeats an id.
    assert_eq!(
        problems_of(&d, ProblemKind::BadRule),
        vec!["75", "75", "75"]
    );
    let kept: Vec<&str> = d.relations["75"]
        .rules
        .iter()
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(kept, vec!["dup"]);
    assert_eq!(d.relations["75"].rules[0].a, Condition::Role("30".into()));
    assert!(
        d.relations["75"].valid,
        "a bad rule does not invalidate its relation"
    );
}

#[test]
fn flags_and_json_fields_accept_both_stored_forms() {
    let t = |id: &str, cf: serde_json::Value| TagRecord {
        id: id.into(),
        name: id.into(),
        parents: vec![],
        custom_fields: cf.as_object().unwrap().clone(),
    };
    let tags = vec![
        t("1", json!({ "isRole": "1" })),
        t("2", json!({ "isRole": 1 })),
        t("3", json!({ "isRole": 0 })),
        // Ends stored as a JSON object instead of a JSON-encoded string.
        t(
            "4",
            json!({ "isRelation": 1, "relationEnds": { "a": "1", "b": "2" } }),
        ),
    ];
    let d = Definitions::from_tags(&tags);
    assert!(d.is_role("1") && d.is_role("2") && !d.is_role("3"));
    assert_eq!(
        d.relations["4"].ends,
        Some(("1".to_string(), "2".to_string()))
    );
    assert!(d.problems.is_empty(), "{:?}", d.problems);
}

#[test]
fn problems_serialize_in_the_contract_shape() {
    let d = defs();
    let p = d
        .problems
        .iter()
        .find(|p| p.kind == ProblemKind::EndNotRole)
        .unwrap();
    let v = serde_json::to_value(p).unwrap();
    assert_eq!(v["kind"], "endNotRole");
    assert_eq!(v["relation"], "73");
    assert!(v["detail"].as_str().unwrap().contains("50"));
}
