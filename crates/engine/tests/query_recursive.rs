//! Recursive Role and Relation criteria (spec 009 T028; research R5; SC-003).
//!
//! The recursive library (`fixtures/query/recursive.json`), the 004 SC-004 example: Full Service
//! (40) with narrower Extra Care (41), Customer (30) with narrower Patient (31), and a Role loop,
//! Loop A (70) and Loop B (71), with Loop Child (72) under Loop A. Scene 1 holds Full Service
//! and Customer, scene 2 Extra Care and Patient, scene 3 Loop B, scene 4 Loop Child, scene 5
//! Loop A, and scene 6 nothing.

mod common;

use common::build::load_library;
use common::query::{answer, expect_error, item, query_fixture_path, store_of};
use engine::graph::GraphStore;
use engine::relations::hierarchy::Hierarchy;
use std::collections::BTreeSet;

fn store(name: &str) -> GraphStore {
    store_of(load_library(&query_fixture_path("recursive.json")), name)
}

/// The scenes bound to `?s`.
fn scenes(store: &GraphStore, text: &str) -> BTreeSet<String> {
    answer(store, text)
        .1
        .into_iter()
        .map(|r| r["s"].clone())
        .collect()
}

fn expect(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|i| item(&format!("scene:{i}"))).collect()
}

#[test]
fn a_recursive_relation_matches_its_narrower_relations() {
    let store = store("rec-relation");
    let q = |c: &str| format!("SELECT ?s WHERE {{ ?r sem:ofRelation {c} ; sem:inMedia ?s }}");
    assert_eq!(
        scenes(&store, &q("recursive:Full%20Service")),
        expect(&["1", "2"])
    );
    assert_eq!(scenes(&store, &q(":Full%20Service")), expect(&["1"]));
    assert_eq!(
        scenes(&store, &q("recursive-tag:40")),
        scenes(&store, &q("recursive:Full%20Service"))
    );
    assert_eq!(scenes(&store, &q("recursive:Extra%20Care")), expect(&["2"]));
}

#[test]
fn a_recursive_role_matches_its_narrower_roles_in_each_scene() {
    let store = store("rec-role");
    let q = |c: &str| format!("SELECT ?s WHERE {{ ?s :hasPerformer ?p . ?p :hasRole {c} }}");
    assert_eq!(
        scenes(&store, &q("recursive:Customer")),
        expect(&["1", "2"])
    );
    assert_eq!(scenes(&store, &q(":Customer")), expect(&["1"]));
    assert_eq!(scenes(&store, &q("recursive-tag:30")), expect(&["1", "2"]));
}

#[test]
fn a_role_on_a_loop_answers_like_the_hierarchy() {
    let store = store("rec-loop");
    let h = Hierarchy::from_links([
        ("31".to_string(), "30".to_string()),
        ("41".to_string(), "40".to_string()),
        ("70".to_string(), "71".to_string()),
        ("71".to_string(), "70".to_string()),
        ("72".to_string(), "70".to_string()),
    ]);
    assert_eq!(
        h.expand_criterion("70", true),
        ["70", "72"].iter().map(|s| s.to_string()).collect()
    );
    let q = "SELECT ?s WHERE { ?s :hasPerformer ?p . ?p :hasRole recursive:Loop%20A }";
    assert_eq!(scenes(&store, q), expect(&["4", "5"]));
    let q = "SELECT ?s WHERE { ?s :hasPerformer ?p . ?p :hasRole recursive:Loop%20B }";
    assert_eq!(scenes(&store, q), expect(&["3"]));
}

#[test]
fn a_recursive_mark_on_a_trait_is_refused_at_its_position() {
    let store = store("rec-trait");
    expect_error(
        &store,
        "SELECT ?p WHERE { ?p :hasTrait recursive:Blonde }",
        "QUERY_NOT_RECURSIVE",
        Some((1, 32)),
    );
}
