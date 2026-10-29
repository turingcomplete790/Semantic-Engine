//! The query index answers exactly as the store does (spec 009 research R12; T056): every known
//! query of the compatibility fixture, the scope fixture and the recursive fixture gives the same
//! set of bindings with the index written at publish as without it. The order differs: the index
//! streams in evaluation order, the store's answer is sorted.

mod common;

use common::build::load_library;
use common::query::{fixture_store, index_agrees, known_queries, query_fixture_path, store_of};

#[test]
fn the_known_queries() {
    let queries = known_queries();
    assert!(queries.len() >= 30);
    index_agrees(&fixture_store("qindex-known"), &queries, "a full build");
}

#[test]
fn the_scope_and_recursive_libraries() {
    let scope = [
        "SELECT ?m WHERE { ?m :hasPerformer ?p . ?p :hasRole :Dom }",
        "SELECT ?m ?dom ?sub WHERE { ?m :hasPerformer ?dom . ?m :hasPerformer ?sub . ?dom :hasRole :Dom . ?sub :hasRole :Sub . FILTER (?dom != ?sub) }",
        "SELECT ?m ?p WHERE { ?m :hasPerformer ?p . ?p :hasRole :Sub . ?p :hasTrait :Blonde }",
        "SELECT ?m WHERE { ?m :hasMiseEnScene :Dungeon }",
        "SELECT ?p WHERE { ?p :hasRole :Dom }",
        "SELECT ?s ?p ?o WHERE { ?s ?p ?o }",
    ];
    let store = store_of(
        load_library(&query_fixture_path("scope.json")),
        "qindex-scope",
    );
    index_agrees(&store, &scope.map(String::from), "the scope library");

    let recursive = [
        "SELECT ?s WHERE { ?r sem:ofRelation recursive:Full%20Service ; sem:inMedia ?s }",
        "SELECT ?s WHERE { ?s :hasPerformer ?p . ?p :hasRole recursive:Customer }",
        "SELECT ?s WHERE { ?s :hasPerformer ?p . ?p :hasRole recursive:Loop%20A }",
        "SELECT ?s ?p ?r WHERE { ?s :hasPerformer ?p . ?p :hasRole ?r }",
    ];
    let store = store_of(
        load_library(&query_fixture_path("recursive.json")),
        "qindex-recursive",
    );
    index_agrees(
        &store,
        &recursive.map(String::from),
        "the recursive library",
    );
}

#[test]
fn every_fact_is_in_the_index() {
    let store = fixture_store("qindex-all");
    index_agrees(
        &store,
        &["SELECT ?s ?p ?o WHERE { ?s ?p ?o }".to_string()],
        "every fact",
    );
}

#[test]
fn a_damaged_index_is_not_used() {
    let store = fixture_store("qindex-damaged");
    let q = "SELECT ?scene WHERE { ?scene :hasPerformer ?p . ?p :hasRole :Dom }".to_string();
    let good = common::query::answer_sets(&store, std::slice::from_ref(&q));
    let n = store.reader().unwrap().generation().number;
    std::fs::write(engine::qindex::path(store.dir(), n), b"SEQI0004 not really").unwrap();
    assert_eq!(common::query::answer_sets(&store, &[q]), good);
}
