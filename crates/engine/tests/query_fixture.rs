//! Known answers on the compatibility fixture (spec 009 T008; SC-001), and what every answer
//! carries: typed values, set semantics, a stable order, and its generation.

mod common;

use common::build::SERVER;
use common::query::{answer, expect_error, fixture_store, query_fixture_path, run_query};
use engine::graph::GraphStore;
use engine::query::Value;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Known {
    name: String,
    query: String,
    expected: Vec<BTreeMap<String, String>>,
    because: String,
}

/// Expands the short forms the file uses: `i:` items, and `^^xsd:` datatypes.
fn expand(v: &str) -> String {
    if let Some(rest) = v.strip_prefix("i:") {
        format!("<urn:semantic-stash:{SERVER}:{rest}>")
    } else if let Some(at) = v.rfind("^^xsd:") {
        format!(
            "{}^^<http://www.w3.org/2001/XMLSchema#{}>",
            &v[..at],
            &v[at + 6..]
        )
    } else {
        v.to_string()
    }
}

#[test]
fn every_known_query_gives_exactly_its_expected_bindings() {
    let store = fixture_store("fixture-known");
    let known: Vec<Known> =
        serde_json::from_str(&std::fs::read_to_string(query_fixture_path("known.json")).unwrap())
            .unwrap();
    assert!(known.len() >= 30, "SC-001 asks for at least 30 queries");
    let mut failures = Vec::new();
    for k in &known {
        let (_, got) = answer(&store, &k.query);
        let mut want: Vec<BTreeMap<String, String>> = k
            .expected
            .iter()
            .map(|r| r.iter().map(|(v, x)| (v.clone(), expand(x))).collect())
            .collect();
        want.sort();
        if got != want {
            failures.push(format!(
                "{} ({}):\n  got  {got:?}\n  want {want:?}",
                k.name, k.because
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn values_are_typed() {
    let store = fixture_store("fixture-typed");
    let page = run_query(
        &store,
        "SELECT ?scene ?i ?c ?t WHERE { ?scene :hasPerformer ?p . ?p sem:hasRole ?i . ?i a ?c ; sem:atTime ?t ; sem:inMedia ?scene }",
    )
    .unwrap();
    assert_eq!(page.variables, vec!["scene", "i", "c", "t"]);
    assert_eq!(page.bindings.len(), 2, "{:?}", page.bindings);
    let row = page
        .bindings
        .iter()
        .find(|r| matches!(&r[0], Value::Item { id, .. } if id == "100"))
        .unwrap();
    assert_eq!(
        row[0],
        Value::Item {
            iri: format!("urn:semantic-stash:{SERVER}:scene:100"),
            kind: "scene".into(),
            id: "100".into(),
        }
    );
    assert_eq!(
        row[1],
        Value::Node {
            iri: format!("urn:semantic-stash:{SERVER}:scene:100/role:8:3"),
        }
    );
    assert_eq!(
        row[2],
        Value::Term {
            iri: "urn:semantic-stash:vocab:RoleInstance".into(),
            name: "RoleInstance".into(),
        }
    );
    match &row[3] {
        Value::Literal {
            value,
            datatype,
            language,
        } => {
            assert_eq!(value, "45.5");
            assert_eq!(datatype, "http://www.w3.org/2001/XMLSchema#decimal");
            assert!(language.is_none());
        }
        v => panic!("{v:?}"),
    }
    let page = run_query(&store, "SELECT ?n WHERE { tag:12 rdfs:label ?n }").unwrap();
    assert_eq!(
        page.bindings[0][0],
        Value::Literal {
            value: "Care".into(),
            datatype: "http://www.w3.org/2001/XMLSchema#string".into(),
            language: None,
        }
    );
}

#[test]
fn answers_are_sets_in_a_stable_order() {
    let store = fixture_store("fixture-order");
    // Performer 7 holds several roles in several scenes; the answer names it once.
    let text = "SELECT ?p WHERE { ?scene :hasPerformer ?p . ?p :hasRole ?r }";
    let page = run_query(&store, text).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for r in &page.bindings {
        assert!(seen.insert(format!("{:?}", r)), "repeated binding {r:?}");
    }
    assert_eq!(seen.len(), 3, "{:?}", page.bindings);
    let again = run_query(&store, text).unwrap();
    assert_eq!(page.bindings, again.bindings);
    let text = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";
    assert_eq!(
        run_query(&store, text).unwrap().bindings,
        run_query(&store, text).unwrap().bindings
    );
}

#[test]
fn the_answer_names_its_generation() {
    let store = fixture_store("fixture-generation");
    let page = run_query(&store, "SELECT ?m WHERE { ?m a sem:Image }").unwrap();
    let current = store.reader().unwrap().generation().clone();
    assert_eq!(page.generation, current);
    assert_eq!(page.offset, 0);
    assert!(page.continuation.is_none());
}

#[test]
fn no_graph_and_a_graph_to_rebuild_are_errors() {
    let dir = std::env::temp_dir().join(format!("se-query-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let empty = GraphStore::new(dir.join("graph"), SERVER);
    let e = expect_error(
        &empty,
        "SELECT ?s WHERE { ?s ?p ?o }",
        "GRAPH_NOT_BUILT",
        None,
    );
    assert!(e.message.contains("Build semantic graph"), "{e}");

    let store = fixture_store("fixture-format");
    let stamp = store.dir().join("store.json");
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&stamp).unwrap()).unwrap();
    v["format"] = serde_json::json!(1);
    std::fs::write(&stamp, v.to_string()).unwrap();
    let e = expect_error(
        &store,
        "SELECT ?s WHERE { ?s ?p ?o }",
        "GRAPH_NEEDS_REBUILD",
        None,
    );
    assert_eq!(e.details().unwrap()["reason"], "otherFormat");
}
