//! Scene-scoped facts join only within one scene (spec 009 T010; research R3; SC-002).
//!
//! The scope library (`fixtures/query/scope.json`): performer 10 (A) is Dom in scene 1 and Sub
//! in scene 2; performer 11 (B) is Sub in scene 1 and Dom in scene 3; performer 12 (C) is in
//! scene 2 with no Role. A carries the Trait Blonde. Scene 3 has the Mise-en-scène Dungeon.
//! Image 1 repeats scene 1's Roles.

mod common;

use common::build::load_library;
use common::query::{answer, expect_error, item, query_fixture_path, store_of};
use engine::graph::GraphStore;
use engine::query::ErrorKind;
use std::collections::BTreeMap;

fn scope_store(name: &str) -> GraphStore {
    store_of(load_library(&query_fixture_path("scope.json")), name)
}

type Rows = Vec<BTreeMap<String, String>>;

fn rows(pairs: &[&[(&str, &str)]]) -> Rows {
    let mut out: Rows = pairs
        .iter()
        .map(|r| r.iter().map(|(v, x)| (v.to_string(), item(x))).collect())
        .collect();
    out.sort();
    out
}

/// Each short-form query, with the same question written in the graph's own shape.
const PAIRS: &[(&str, &str)] = &[
    (
        "SELECT ?m WHERE { ?m :hasPerformer ?p . ?p :hasRole :Dom }",
        "SELECT ?m WHERE { ?m :hasPerformer ?p . ?p sem:hasRole ?i . ?i sem:ofTag :Dom ; sem:inMedia ?m }",
    ),
    (
        "SELECT ?m ?dom ?sub WHERE {
           ?m :hasPerformer ?dom . ?m :hasPerformer ?sub .
           ?dom :hasRole :Dom . ?sub :hasRole :Sub .
           FILTER (?dom != ?sub) }",
        "SELECT ?m ?dom ?sub WHERE {
           ?m :hasPerformer ?dom . ?m :hasPerformer ?sub .
           ?dom sem:hasRole ?x . ?x sem:ofTag :Dom ; sem:inMedia ?m .
           ?sub sem:hasRole ?y . ?y sem:ofTag :Sub ; sem:inMedia ?m .
           FILTER (?dom != ?sub) }",
    ),
    (
        "SELECT ?m ?p WHERE { ?m :hasPerformer ?p . ?p :hasRole :Sub . ?p :hasTrait :Blonde }",
        "SELECT ?m ?p WHERE { ?m :hasPerformer ?p . ?p sem:hasRole ?i . ?i sem:ofTag :Sub ; sem:inMedia ?m . ?p sem:hasTrait :Blonde }",
    ),
    (
        "SELECT ?m WHERE { ?m :hasMiseEnScene :Dungeon }",
        "SELECT ?m WHERE { ?m sem:hasMiseEnScene ?i . ?i sem:ofTag :Dungeon }",
    ),
    (
        "SELECT ?p WHERE { ?p :hasRole :Dom }",
        "SELECT ?p WHERE { ?p sem:hasRole ?i . ?i sem:ofTag :Dom ; sem:inMedia ?m }",
    ),
    (
        "SELECT ?m ?r WHERE { ?m :hasPerformer performer:10 . performer:10 :hasRole ?r }",
        "SELECT ?m ?r WHERE { ?m :hasPerformer performer:10 . performer:10 sem:hasRole ?i . ?i sem:ofTag ?r ; sem:inMedia ?m }",
    ),
];

#[test]
fn a_role_matches_only_in_the_scene_where_it_is_held() {
    let store = scope_store("scope-dom");
    let (_, got) = answer(&store, PAIRS[0].0);
    assert_eq!(
        got,
        rows(&[
            &[("m", "scene:1")],
            &[("m", "scene:3")],
            &[("m", "image:1")]
        ])
    );
}

#[test]
fn the_documents_multiple_bound_performers() {
    let store = scope_store("scope-pair");
    let (_, got) = answer(&store, PAIRS[1].0);
    assert_eq!(
        got,
        rows(&[
            &[
                ("m", "scene:1"),
                ("dom", "performer:10"),
                ("sub", "performer:11")
            ],
            &[
                ("m", "image:1"),
                ("dom", "performer:10"),
                ("sub", "performer:11")
            ],
        ])
    );
}

#[test]
fn a_global_trait_holds_in_every_scene_and_the_role_in_its_own() {
    let store = scope_store("scope-trait");
    let (_, got) = answer(&store, PAIRS[2].0);
    assert_eq!(got, rows(&[&[("m", "scene:2"), ("p", "performer:10")]]));
}

#[test]
fn mise_en_scene_and_facts_with_no_scene() {
    let store = scope_store("scope-mise");
    let (_, got) = answer(&store, PAIRS[3].0);
    assert_eq!(got, rows(&[&[("m", "scene:3")]]));
    let (_, got) = answer(&store, PAIRS[4].0);
    assert_eq!(
        got,
        rows(&[&[("p", "performer:10")], &[("p", "performer:11")]])
    );
    let (_, got) = answer(&store, PAIRS[5].0);
    assert_eq!(
        got,
        rows(&[
            &[("m", "scene:1"), ("r", "tag:1")],
            &[("m", "scene:2"), ("r", "tag:2")],
            &[("m", "image:1"), ("r", "tag:1")],
        ])
    );
}

#[test]
fn the_short_form_equals_the_graph_form() {
    let store = scope_store("scope-equal");
    for (short, graph) in PAIRS {
        let (_, a) = answer(&store, short);
        let (_, b) = answer(&store, graph);
        assert_eq!(a, b, "short:\n{short}\ngraph:\n{graph}");
        assert!(!a.is_empty(), "{short}");
    }
}

#[test]
fn a_performer_linked_to_two_scenes_makes_the_scope_ambiguous() {
    let store = scope_store("scope-ambiguous");
    let e = expect_error(
        &store,
        "SELECT ?a ?b WHERE {\n  ?a :hasPerformer ?p .\n  ?b :hasPerformer ?p .\n  ?p :hasRole :Dom .\n}",
        "QUERY_SCOPE_AMBIGUOUS",
        Some((4, 3)),
    );
    assert!(matches!(&e.kind, ErrorKind::ScopeAmbiguous { performer } if performer == "?p"));
    // The same performer and scene written twice is one scope, not two.
    let (_, got) = answer(
        &store,
        "SELECT ?m WHERE { ?m :hasPerformer ?p . ?m :hasPerformer ?p . ?p :hasRole :Sub }",
    );
    assert_eq!(
        got,
        rows(&[
            &[("m", "scene:1")],
            &[("m", "scene:2")],
            &[("m", "image:1")]
        ])
    );
}

#[test]
fn a_variable_object_used_as_a_subject_is_the_graph_form() {
    let store = scope_store("scope-graph-form");
    let (_, got) = answer(
        &store,
        "SELECT ?i WHERE { scene:2 :hasPerformer ?p . ?p :hasRole ?i . ?i sem:inMedia scene:2 }",
    );
    assert_eq!(got, rows(&[&[("i", "scene:2/role:10:2")]]));
}
