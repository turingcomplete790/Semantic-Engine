//! Names: built-in prefixes, tags by name, vocabulary checks and item ids (spec 009 T006;
//! research R2; contracts/query-language.md "Names").

mod common;

use common::build::fixture_library;
use common::query::{answer, expect_error, fixture_store, item, store_of};
use engine::query::ErrorKind;
use std::collections::BTreeMap;

fn column(rows: &[BTreeMap<String, String>], var: &str) -> Vec<String> {
    let mut v: Vec<String> = rows.iter().map(|r| r[var].clone()).collect();
    v.sort();
    v
}

#[test]
fn built_in_prefixes_resolve() {
    let store = fixture_store("names-prefixes");
    let (_, rows) = answer(
        &store,
        "SELECT ?s WHERE { ?s sem:hasPerformer performer:8 }",
    );
    assert_eq!(
        column(&rows, "s"),
        vec![
            item("image:200"),
            item("scene:100"),
            item("scene:301"),
            item("scene:302")
        ]
    );
    let (_, rows) = answer(&store, "SELECT ?p WHERE { scene:100 :hasPerformer ?p }");
    assert_eq!(
        column(&rows, "p"),
        vec![item("performer:7"), item("performer:8")]
    );
    let (_, rows) = answer(&store, "SELECT ?p WHERE { image:200 :hasPerformer ?p }");
    assert_eq!(column(&rows, "p"), vec![item("performer:8")]);
    let (_, rows) = answer(&store, "SELECT ?t WHERE { performer:7 :hasTrait ?t }");
    assert_eq!(column(&rows, "t"), vec![item("tag:7")]);
    let (_, rows) = answer(&store, "SELECT ?p WHERE { ?p :hasTrait tag:7 }");
    assert_eq!(column(&rows, "p"), vec![item("performer:7")]);
    // A full item IRI is checked like its prefixed form.
    let full = item("scene:101");
    let (_, rows) = answer(
        &store,
        &format!("SELECT ?p WHERE {{ {full} :hasPerformer ?p }}"),
    );
    assert_eq!(
        column(&rows, "p"),
        vec![item("performer:7"), item("performer:9")]
    );
    // rdf:, rdfs: and a declared prefix.
    let (_, rows) = answer(
        &store,
        "PREFIX v: <urn:semantic-stash:vocab:>\nSELECT ?t WHERE { ?t rdf:type v:Relation ; rdfs:label ?n }",
    );
    assert_eq!(column(&rows, "t"), vec![item("tag:12"), item("tag:14")]);
}

#[test]
fn tags_by_their_exact_stash_name() {
    let store = fixture_store("names-tags");
    let (_, rows) = answer(&store, "SELECT ?i WHERE { ?i sem:ofTag :Dom }");
    assert_eq!(
        column(&rows, "i"),
        vec![
            item("image:200/role:8:2"),
            item("scene:100/role:7:2"),
            item("scene:101/role:9:2"),
        ]
    );
    let (_, rows) = answer(&store, "SELECT ?p WHERE { ?p :hasTrait :Hair%20Colour }");
    assert_eq!(column(&rows, "p"), vec![item("performer:7")]);
    // An escaped percent is a percent sign, not an escape: no tag is named "Hair%20Colour".
    expect_error(
        &store,
        "SELECT ?p WHERE { ?p :hasTrait :Hair\\%20Colour }",
        "QUERY_UNKNOWN",
        Some((1, 32)),
    );
}

#[test]
fn unknown_names_are_errors_at_their_position() {
    let store = fixture_store("names-unknown");
    let e = expect_error(
        &store,
        "SELECT ?i WHERE { ?i sem:ofTag :Nobody }",
        "QUERY_UNKNOWN",
        Some((1, 32)),
    );
    assert!(
        matches!(&e.kind, ErrorKind::Unknown { name, .. } if name == ":Nobody"),
        "{e:?}"
    );
    assert!(e.message.contains("no tag"), "{e}");
    // "Dominant" is a category value of tag 2 and the name of tag 1: only names count.
    let (_, rows) = answer(&store, "SELECT ?i WHERE { ?i sem:ofTag :Dominant }");
    assert_eq!(column(&rows, "i"), vec![item("scene:301/role:8:1")]);

    let e = expect_error(
        &store,
        "SELECT ?s WHERE { ?s ex:p ?o }",
        "QUERY_UNKNOWN",
        Some((1, 22)),
    );
    assert!(e.message.contains("prefix"), "{e}");
    let e = expect_error(
        &store,
        "SELECT ?s WHERE { ?s sem:hasTag ?o }",
        "QUERY_UNKNOWN",
        Some((1, 22)),
    );
    assert!(e.message.contains("Planned"), "{e}");
    let e = expect_error(
        &store,
        "SELECT ?s WHERE {\n ?s sem:inStage ?o }",
        "QUERY_UNKNOWN",
        Some((2, 5)),
    );
    assert!(e.message.contains("Derived"), "{e}");
    let e = expect_error(
        &store,
        "SELECT ?s WHERE { ?s sem:noSuchTerm ?o }",
        "QUERY_UNKNOWN",
        Some((1, 22)),
    );
    assert!(e.message.contains("not a vocabulary term"), "{e}");
    expect_error(
        &store,
        "SELECT ?s WHERE { ?s <urn:semantic-stash:vocab:nope> ?o }",
        "QUERY_UNKNOWN",
        Some((1, 22)),
    );
    let e = expect_error(
        &store,
        "SELECT ?p WHERE { scene:999999 :hasPerformer ?p }",
        "QUERY_UNKNOWN",
        Some((1, 19)),
    );
    assert!(e.message.contains("no scene"), "{e}");
    expect_error(
        &store,
        "SELECT ?p WHERE { scene:abc :hasPerformer ?p }",
        "QUERY_UNKNOWN",
        Some((1, 19)),
    );
    expect_error(
        &store,
        &format!(
            "SELECT ?p WHERE {{ {} :hasTrait ?p }}",
            item("performer:4242")
        ),
        "QUERY_UNKNOWN",
        Some((1, 19)),
    );
}

#[test]
fn a_name_two_tags_share_is_ambiguous() {
    let mut lib = fixture_library();
    lib.tags.iter_mut().find(|t| t.id == "3").unwrap().name = "Dom".into();
    let store = store_of(lib, "names-twice");
    let e = expect_error(
        &store,
        "SELECT ?i WHERE { ?i sem:ofTag :Dom }",
        "QUERY_UNKNOWN",
        Some((1, 32)),
    );
    assert!(e.message.contains("2 tags"), "{e}");
    // By id it is still reachable.
    let (_, rows) = answer(&store, "SELECT ?i WHERE { ?i sem:ofTag tag:3 }");
    assert!(!rows.is_empty());
}

#[test]
fn a_vocabulary_term_wins_over_a_tag_of_the_same_name() {
    let mut lib = fixture_library();
    lib.tags.iter_mut().find(|t| t.id == "3").unwrap().name = "hasTrait".into();
    let store = store_of(lib, "names-term-wins");
    let (_, rows) = answer(&store, "SELECT ?p ?t WHERE { ?p :hasTrait ?t }");
    assert_eq!(
        column(&rows, "p"),
        vec![item("performer:7"), item("performer:8")]
    );
    let (_, rows) = answer(&store, "SELECT ?i WHERE { ?i sem:ofTag tag:3 }");
    assert!(!rows.is_empty());
}
