//! Terms and facts (spec 006 T003; data-model "Term", "Fact").

use engine::graph::{round_trip, validate, Fact, GraphError, Term};

fn iri(s: &str) -> Term {
    Term::iri(s)
}

fn invalid(f: &Fact) -> bool {
    matches!(validate(f), Err(GraphError::InvalidFact { .. }))
}

#[test]
fn iris_must_be_absolute() {
    let ok = Fact::new(iri("urn:a:s"), iri("urn:a:p"), iri("http://example.com/o"));
    assert!(validate(&ok).is_ok());
    assert!(invalid(&Fact::new(
        iri("relative/s"),
        iri("urn:a:p"),
        iri("urn:a:o")
    )));
    assert!(invalid(&Fact::new(
        iri("urn:a:s"),
        iri("has space"),
        iri("urn:a:o")
    )));
}

#[test]
fn blank_node_labels_are_restricted() {
    let b = |l: &str| Fact::new(Term::Blank(l.into()), iri("urn:a:p"), iri("urn:a:o"));
    assert!(validate(&b("node_1-a")).is_ok());
    for bad in ["", "has space", "dot.ted", "é"] {
        assert!(invalid(&b(bad)), "{bad:?}");
    }
}

#[test]
fn a_literal_has_a_datatype_or_a_language_not_both() {
    let both = Term::Literal {
        value: "x".into(),
        datatype: Some("http://www.w3.org/2001/XMLSchema#string".into()),
        language: Some("en".into()),
    };
    assert!(invalid(&Fact::new(iri("urn:a:s"), iri("urn:a:p"), both)));
}

#[test]
fn subjects_and_predicates_are_restricted() {
    assert!(invalid(&Fact::new(
        Term::string("lit"),
        iri("urn:a:p"),
        iri("urn:a:o")
    )));
    assert!(invalid(&Fact::new(
        iri("urn:a:s"),
        Term::Blank("b".into()),
        iri("urn:a:o")
    )));
    assert!(invalid(&Fact::new(
        iri("urn:a:s"),
        Term::string("p"),
        iri("urn:a:o")
    )));
}

#[test]
fn every_field_survives_a_round_trip() {
    let long = "x".repeat(100 * 1024);
    let objects = [
        Term::string("Ünïcödé 名前 🎬"),
        Term::string(long),
        Term::Literal {
            value: "bonjour".into(),
            datatype: None,
            language: Some("fr".into()),
        },
        Term::typed("42", "http://www.w3.org/2001/XMLSchema#integer"),
        Term::Blank("b0".into()),
        iri("urn:semantic-stash:vocab:Role"),
    ];
    for o in objects {
        let f = Fact::new(iri("urn:a:s"), iri("urn:a:p"), o);
        assert_eq!(round_trip(&f).unwrap(), f);
    }
    let blank_subject = Fact::new(Term::Blank("s1".into()), iri("urn:a:p"), Term::string("v"));
    assert_eq!(round_trip(&blank_subject).unwrap(), blank_subject);
}
