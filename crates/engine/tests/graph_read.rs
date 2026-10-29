//! Reading and querying a published generation (spec 006 T013; US2, FR-010 to FR-013).

mod common;

use common::graph::{cleared, fact, iri, lit, publish, typed};
use engine::graph::{GraphError, Term};

const XSD_INT: &str = "http://www.w3.org/2001/XMLSchema#integer";

fn library() -> Vec<engine::graph::Fact> {
    vec![
        fact("scene/1", "title", lit("Beta")),
        fact("scene/2", "title", lit("Alpha")),
        fact("scene/3", "title", lit("Gamma")),
        fact("scene/1", "rating", typed("80", XSD_INT)),
        fact(
            "scene/2",
            "label",
            Term::Literal {
                value: "bonjour".into(),
                datatype: None,
                language: Some("fr".into()),
            },
        ),
    ]
}

#[test]
fn select_answers_in_order_with_variables_and_unbound_cells() {
    let (_, store) = cleared("select");
    publish(&store, &library());
    let r = store.reader().unwrap();
    let s = r
        .select(
            "SELECT ?title ?scene ?rating WHERE { ?scene <urn:t:title> ?title \
             OPTIONAL { ?scene <urn:t:rating> ?rating } } ORDER BY ?title",
        )
        .unwrap();
    assert_eq!(s.variables, ["title", "scene", "rating"]);
    let titles: Vec<_> = s.rows.iter().map(|r| r[0].clone()).collect();
    assert_eq!(
        titles,
        [Some(lit("Alpha")), Some(lit("Beta")), Some(lit("Gamma"))]
    );
    assert_eq!(s.rows[0][1], Some(iri("scene/2")));
    assert_eq!(s.rows[0][2], None, "scene 2 has no rating");
    assert_eq!(s.rows[1][2], Some(typed("80", XSD_INT)));
    let lang = r
        .select("SELECT ?l WHERE { <urn:t:scene/2> <urn:t:label> ?l }")
        .unwrap();
    assert_eq!(
        lang.rows[0][0],
        Some(Term::Literal {
            value: "bonjour".into(),
            datatype: None,
            language: Some("fr".into())
        })
    );
}

#[test]
fn a_reader_keeps_its_generation_while_a_newer_one_is_published() {
    let (_, store) = cleared("keep");
    for i in 0..5 {
        publish(&store, &[fact(&format!("gen/{i}"), "n", lit("x"))]);
    }
    let old = store.reader().unwrap();
    assert_eq!(old.generation().number, 5);
    publish(&store, &[fact("gen/new", "n", lit("x"))]);
    let count = |r: &engine::graph::Reader| {
        r.select("SELECT ?s WHERE { ?s <urn:t:n> ?o }")
            .unwrap()
            .rows
            .len()
    };
    assert_eq!(old.generation().number, 5);
    assert_eq!(
        count(&old),
        5,
        "the old reader still answers from generation 5"
    );
    let new = store.reader().unwrap();
    assert_eq!(new.generation().number, 6);
    assert_eq!(count(&new), 6);
}

#[test]
fn a_syntax_error_has_its_position() {
    let (_, store) = cleared("syntax");
    publish(&store, &library());
    let r = store.reader().unwrap();
    match r.select("SELECT ?s WHERE { ?s ?p }") {
        Err(GraphError::InvalidQuery {
            line,
            column,
            message,
        }) => {
            assert_eq!(line, Some(1), "{message}");
            assert!(column.is_some_and(|c| c > 20), "{column:?}: {message}");
        }
        other => panic!("{other:?}"),
    }
    match r.select("SELECT ?s\nWHERE {\n  ?s ?p ?o\n  FILTER(\n}") {
        Err(GraphError::InvalidQuery { line, .. }) => assert!(line.is_some_and(|l| l >= 4)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn only_select_is_allowed() {
    let (_, store) = cleared("forms");
    publish(&store, &library());
    let r = store.reader().unwrap();
    for q in [
        "ASK { ?s ?p ?o }",
        "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
        "DESCRIBE <urn:t:scene/1>",
        "INSERT DATA { <urn:t:a> <urn:t:b> <urn:t:c> }",
    ] {
        match r.select(q) {
            Err(GraphError::InvalidQuery { message, .. }) => {
                assert!(message.contains("SELECT"), "{q}: {message}")
            }
            other => panic!("{q}: {other:?}"),
        }
    }
}

#[test]
fn not_built_is_distinct_from_an_empty_graph() {
    let (_, store) = cleared("notbuilt");
    assert!(matches!(store.reader(), Err(GraphError::NotBuilt)));
    publish(&store, &[]);
    let r = store.reader().unwrap();
    let s = r.select("SELECT ?s WHERE { ?s ?p ?o }").unwrap();
    assert_eq!(s.variables, ["s"]);
    assert!(s.rows.is_empty());
}

// ---- Opening a kept generation (spec 009 T031; research R6)

#[test]
fn reader_at_opens_kept_generations_and_reports_the_rest_gone() {
    let (dir, store) = cleared("reader-at");
    let first = publish(&store, &library()[..2]);
    let second = publish(&store, &library());
    let old = store.reader_at(first.number).unwrap();
    assert_eq!(old.generation().number, first.number);
    let count =
        |r: &engine::graph::Reader| r.select("SELECT ?s WHERE { ?s ?p ?o }").unwrap().rows.len();
    assert_eq!(count(&old), 2, "the generation before answers as it was");
    let now = store.reader_at(second.number).unwrap();
    assert_eq!(now.generation(), store.reader().unwrap().generation());
    assert_eq!(count(&now), 5);
    drop((old, now));

    // Never `work/`: a writer with unpublished changes holds the next number.
    let mut w = store.writer().unwrap();
    w.add(&library()[..1]).unwrap();
    assert_eq!(
        store.reader_at(second.number + 1).err(),
        Some(GraphError::GenerationGone {
            generation: second.number + 1
        })
    );
    drop(w);

    // A generation no longer kept.
    std::fs::remove_dir_all(dir.join("snapshots").join(first.number.to_string())).unwrap();
    assert_eq!(
        store.reader_at(first.number).err(),
        Some(GraphError::GenerationGone {
            generation: first.number
        })
    );
    assert!(matches!(
        store.reader_at(0),
        Err(GraphError::GenerationGone { generation: 0 })
    ));
}

#[test]
fn reader_at_checks_the_stamp_first() {
    let (dir, store) = cleared("reader-at-stamp");
    let g = publish(&store, &library());
    let other = engine::graph::GraphStore::new(&dir, "another-server");
    assert!(matches!(
        other.reader_at(g.number),
        Err(GraphError::NeedsRebuild(_))
    ));
}
