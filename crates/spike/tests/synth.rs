//! Synthetic vocabulary-shaped graphs (tasks T010).

use oxigraph::model::{Subject, Term};
use semantic_vocabulary::iri::ItemIris;
use spike::synth::{self, Spec, RDFS_LABEL, RDF_TYPE, SERVER_ID, VOCAB};
use std::collections::BTreeSet;

#[test]
fn every_vocabulary_predicate_and_class_is_a_defined_term() {
    let vocab = semantic_vocabulary::embedded().unwrap();
    let defined: BTreeSet<String> = vocab.terms().map(|t| t.iri).collect();
    let mut predicates = BTreeSet::new();
    let mut classes = BTreeSet::new();
    for q in synth::quads(Spec::new(200)) {
        let p = q.predicate.as_str().to_string();
        if p == RDF_TYPE {
            if let Term::NamedNode(c) = &q.object {
                classes.insert(c.as_str().to_string());
            }
        }
        predicates.insert(p);
    }
    for p in &predicates {
        assert!(
            p == RDF_TYPE || p == RDFS_LABEL || (p.starts_with(VOCAB) && defined.contains(p)),
            "predicate {p} is not a vocabulary 1.1.0 term"
        );
    }
    for c in &classes {
        assert!(
            defined.contains(c),
            "class {c} is not a vocabulary 1.1.0 term"
        );
    }
    assert_eq!(vocab.version().as_deref(), Some("1.1.0"));
}

#[test]
fn item_iris_come_from_the_iri_scheme() {
    let iris = ItemIris::new(SERVER_ID).unwrap();
    let scene1 = iris.scene("1");
    let subjects: BTreeSet<String> = synth::quads(Spec::new(5))
        .filter_map(|q| match q.subject {
            Subject::NamedNode(n) => Some(n.as_str().to_string()),
            _ => None,
        })
        .collect();
    assert!(subjects.contains(&scene1));
    assert!(subjects.contains(&iris.image("1")));
    assert!(subjects.contains(&iris.performer("1")));
    let prefix = format!("urn:semantic-stash:{SERVER_ID}:");
    assert!(
        subjects.iter().all(|s| s.starts_with(&prefix)),
        "every subject is an item IRI or derived from one"
    );
}

#[test]
fn the_same_spec_gives_identical_quads() {
    let a: Vec<String> = synth::quads(Spec::new(300))
        .map(|q| q.to_string())
        .collect();
    let b: Vec<String> = synth::quads(Spec::new(300))
        .map(|q| q.to_string())
        .collect();
    assert_eq!(a, b);
    let other: Vec<String> = synth::quads(Spec {
        scenes: 300,
        seed: 2,
    })
    .map(|q| q.to_string())
    .collect();
    assert_ne!(a, other);
}

#[test]
fn scales_parse_and_test_is_the_test_library() {
    assert_eq!(synth::parse_scale("test"), Some(1021));
    assert_eq!(synth::parse_scale("50k"), Some(50_000));
    assert_eq!(synth::parse_scale("100k"), Some(100_000));
    assert_eq!(synth::parse_scale("250k"), Some(250_000));
    assert_eq!(synth::parse_scale("1234"), Some(1234));
    assert_eq!(synth::parse_scale("0"), None);
    assert_eq!(synth::parse_scale("big"), None);

    let scenes = synth::quads(Spec::new(synth::parse_scale("test").unwrap()))
        .filter(|q| {
            q.predicate.as_str() == RDF_TYPE && q.object.to_string() == format!("<{VOCAB}Scene>")
        })
        .count();
    assert_eq!(scenes, 1021);
}

#[test]
fn load_streams_into_an_oxigraph_store() {
    let store = oxigraph::store::Store::new().unwrap();
    let n = synth::load(&store, Spec::new(50)).unwrap();
    assert_eq!(store.len().unwrap() as u64, n);
    assert!(n > 50 * 10);
}
