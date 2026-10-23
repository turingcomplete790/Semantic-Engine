//! Role and relation hierarchies (spec 004 T011; research R4).

mod common;
use engine::relations::defs::{Definitions, ProblemKind};
use engine::relations::hierarchy::Hierarchy;
use serde_json::json;
use stash_adapter::relations::TagRecord;
use std::collections::BTreeSet;

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

fn built(tags: Vec<TagRecord>) -> (Definitions, Hierarchy) {
    let mut d = Definitions::from_tags(&tags);
    let h = Hierarchy::build(&mut d);
    (d, h)
}

fn tag(id: &str, parents: &[&str], cf: serde_json::Value) -> TagRecord {
    TagRecord {
        id: id.into(),
        name: format!("tag {id}"),
        parents: parents.iter().map(|s| s.to_string()).collect(),
        custom_fields: cf.as_object().unwrap().clone(),
    }
}

#[test]
fn example1_roles_and_relations_have_their_broader_terms() {
    let (_, h) = built(common::all_tags());
    assert_eq!(h.broader("31"), set(&["30"]));
    // Nurse's parent 90 has no Role flag, so it is not a broader role.
    assert_eq!(h.broader("33"), set(&["32"]));
    assert_eq!(h.narrower("32", false), set(&["33", "34"]));
    assert_eq!(h.broader_closure("31"), set(&["30"]));
    assert_eq!(h.broader("41"), set(&["40"]));
    assert!(h.narrower("40", true).contains("41"));
    assert!(h.is_same_or_narrower("31", "30"));
    assert!(h.is_same_or_narrower("30", "30"));
    assert!(!h.is_same_or_narrower("30", "31"));
}

#[test]
fn narrower_is_direct_or_every_depth() {
    let r = json!({ "isRole": 1 });
    let (_, h) = built(vec![
        tag("1", &[], r.clone()),
        tag("2", &["1"], r.clone()),
        tag("3", &["2"], r.clone()),
        // Two broader roles at once.
        tag("4", &["1", "5"], r.clone()),
        tag("5", &[], r),
    ]);
    assert_eq!(h.narrower("1", false), set(&["2", "4"]));
    assert_eq!(h.narrower("1", true), set(&["2", "3", "4"]));
    assert_eq!(h.broader_closure("3"), set(&["1", "2"]));
    assert_eq!(h.broader("4"), set(&["1", "5"]));
}

#[test]
fn loops_are_reported_once_and_their_links_ignored() {
    let (d, h) = built(common::all_tags());
    let loops: Vec<_> = d
        .problems
        .iter()
        .filter(|p| p.kind == ProblemKind::Loop)
        .collect();
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].tags, vec!["70".to_string(), "71".to_string()]);
    assert!(h.broader("70").is_empty() && h.broader("71").is_empty());
    // Everything else is unaffected.
    assert_eq!(h.broader("31"), set(&["30"]));
}

#[test]
fn a_narrower_relation_must_have_narrower_ends() {
    let (d, _) = built(common::all_tags());
    assert!(
        d.relations["41"].valid,
        "Extra Care: Patient under Customer, Nurse under Professional"
    );
    assert!(!d.relations["74"].valid);
    let p: Vec<_> = d
        .problems
        .iter()
        .filter(|p| p.kind == ProblemKind::InvalidEnds)
        .collect();
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].relation.as_deref(), Some("74"));
    assert!(p[0].detail.contains("60") && p[0].detail.contains("32"));
}

#[test]
fn relations_on_a_loop_are_invalid() {
    let ends = json!({ "isRelation": 1, "relationEnds": "{\"a\":\"1\",\"b\":\"1\"}" });
    let (d, h) = built(vec![
        tag("1", &[], json!({ "isRole": 1 })),
        tag("10", &["11"], ends.clone()),
        tag("11", &["10"], ends),
    ]);
    assert!(!d.relations["10"].valid && !d.relations["11"].valid);
    assert!(h.broader("10").is_empty());
}
