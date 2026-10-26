//! The captured test-instance library maps completely (spec 007 T034; FR-021, SC-003).

mod common;

use common::build::{iris, load_library, test_instance_path, SERVER};
use engine::build::{map_library, Mapped};
use engine::graph::{Fact, Term};
use stash_adapter::reader::LibraryRead;
use std::collections::{BTreeMap, BTreeSet};

const SEM: &str = "urn:semantic-stash:vocab:";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";

fn mapped() -> (LibraryRead, Mapped) {
    let read = load_library(&test_instance_path());
    let m = map_library(&read, &iris());
    (read, m)
}

fn item(rest: &str) -> Term {
    Term::iri(format!("urn:semantic-stash:{SERVER}:{rest}"))
}

fn sem(local: &str) -> Term {
    Term::iri(format!("{SEM}{local}"))
}

fn typed(m: &Mapped, class: &str) -> BTreeSet<Term> {
    m.facts
        .iter()
        .filter(|f| f.predicate == Term::iri(RDF_TYPE) && f.object == sem(class))
        .map(|f| f.subject.clone())
        .collect()
}

#[test]
fn counts_match_the_library() {
    let (read, m) = mapped();
    assert_eq!(typed(&m, "Scene").len(), read.scenes.len());
    assert_eq!(typed(&m, "Image").len(), read.images.len());
    assert_eq!(typed(&m, "Performer").len(), read.performers.len());
    let tags = m
        .facts
        .iter()
        .filter(|f| f.predicate == Term::iri(RDFS_LABEL))
        .count();
    assert_eq!(tags, read.tags.len());
}

#[test]
fn every_instance_is_complete() {
    let (_, m) = mapped();
    let mut checked = 0;
    for class in ["RoleInstance", "ActionInstance", "PluggableInstance"] {
        for i in typed(&m, class) {
            let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
            for f in m.facts.iter().filter(|f| f.subject == i) {
                for p in ["inMedia", "forPerformer", "ofTag"] {
                    if f.predicate == sem(p) {
                        *seen.entry(p).or_default() += 1;
                    }
                }
            }
            for p in ["inMedia", "forPerformer", "ofTag"] {
                assert_eq!(seen.get(p), Some(&1), "{i:?} {p}");
            }
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn every_fact_uses_a_defined_term() {
    let (_, m) = mapped();
    let v = semantic_vocabulary::embedded().unwrap();
    let ours = format!("urn:semantic-stash:{SERVER}:");
    for f in &m.facts {
        let Term::Iri(s) = &f.subject else { continue };
        if !s.starts_with(&ours) {
            continue;
        }
        let Term::Iri(p) = &f.predicate else {
            unreachable!()
        };
        assert!(
            p == RDF_TYPE || p == RDFS_LABEL || v.is_defined(p),
            "undefined predicate in {}",
            f.to_ntriples()
        );
        if p == RDF_TYPE {
            let Term::Iri(o) = &f.object else {
                panic!("{f:?}")
            };
            assert!(v.is_defined(o), "undefined class in {}", f.to_ntriples());
        }
    }
}

#[test]
fn facts_from_the_documents_examples_are_present() {
    let (_, m) = mapped();
    let has = |s: &str, p: &str, o: Term| m.facts.contains(&Fact::new(item(s), sem(p), o));
    // Scene 2: performer 7 is Dom (tag 19) and makes out (action 21).
    assert!(has("scene:2/role:7:19", "ofTag", item("tag:19")));
    assert!(has("performer:7", "hasRole", item("scene:2/role:7:19")));
    assert!(has(
        "scene:2/action:7:21",
        "hasVariation",
        Term::string("making out")
    ));
    // The Beach (tag 24) is picked at sunset and at night across two Stages.
    for v in ["sunset", "night"] {
        assert!(
            has("scene:2/mise:24", "hasVariation", Term::string(v)),
            "{v}"
        );
    }
    // Label 26's rules, by id.
    for r in ["r1", "r2", "r3"] {
        assert!(
            has("tag:26", "hasRule", item(&format!("tag:26/rule:{r}"))),
            "{r}"
        );
    }
}

/// The captured library has two real problems, both on image 15: its role assignments name
/// performer 8, who is not on the image, and a timeline slot places an action instance the image
/// does not have. The build notes them and maps the rest; the test pins them so a change in the
/// captured data shows up.
#[test]
fn the_captured_library_gives_exactly_its_known_notes() {
    let (_, m) = mapped();
    let lines: Vec<String> = m.notes.as_slice().iter().map(|n| n.log_line()).collect();
    assert_eq!(
        lines,
        [
            "note: image 15 roleTagAssignments [8]: performerNotInMedia",
            "note: image 15 timeline [se-slot-1]: danglingRef",
        ]
    );
}
