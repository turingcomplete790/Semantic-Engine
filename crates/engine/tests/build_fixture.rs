//! The hand-written fixture library maps to exactly its expected graph (spec 007 T014; US1,
//! SC-001).

mod common;

use common::build::{assert_graph, fixture_library, iris, SERVER};
use engine::build::{map_library, vocabulary_facts, Mapped};
use engine::graph::{Fact, GraphStore, Term};

const SEM: &str = "urn:semantic-stash:vocab:";

fn mapped() -> Mapped {
    map_library(&fixture_library(), &iris())
}

fn item(rest: &str) -> Term {
    Term::iri(format!("urn:semantic-stash:{SERVER}:{rest}"))
}

fn has(m: &Mapped, s: &str, p: &str, o: Term) -> bool {
    m.facts
        .contains(&Fact::new(item(s), Term::iri(format!("{SEM}{p}")), o))
}

/// Publishes the mapped facts plus the vocabulary into a fresh store, for query checks.
fn published(name: &str, m: &Mapped) -> GraphStore {
    let dir = std::env::temp_dir().join(format!("se-build-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = GraphStore::new(dir.join("graph"), SERVER);
    store.clear_for_rebuild().unwrap();
    let mut w = store.rebuild_writer().unwrap();
    let mut facts: Vec<Fact> = m.facts.iter().cloned().collect();
    facts.extend(vocabulary_facts());
    w.add(&facts).unwrap();
    w.publish().unwrap();
    store
}

#[test]
fn the_fixture_graph_equals_the_expected_graph() {
    let m = mapped();
    assert_graph("expected.nt", &m);
    // Only the records with bad fields (user story 3: tags 15 and 16, performers 30 and 31,
    // scenes 300 to 302) give notes.
    let bad = ["15", "16", "30", "31", "300", "301", "302"];
    for n in m.notes.as_slice() {
        assert!(bad.contains(&n.id.as_str()), "unexpected note {n:?}");
    }
}

#[test]
fn us1_1_a_role_instance_has_its_media_performer_and_tag() {
    let m = mapped();
    let i = "scene:100/role:7:2";
    assert!(has(&m, i, "inMedia", item("scene:100")));
    assert!(has(&m, i, "forPerformer", item("performer:7")));
    assert!(has(&m, i, "ofTag", item("tag:2")));
    assert!(has(&m, "performer:7", "hasRole", item(i)));
    assert!(has(&m, "scene:100", "hasPerformer", item("performer:7")));
}

#[test]
fn us1_2_roles_never_cross_scenes() {
    let store = published("cross", &mapped());
    let r = store.reader().unwrap();
    // A performer who is Dom (tag 2) and Sub (tag 3) within one media item: none.
    let q = format!(
        "PREFIX sem: <{SEM}>
         SELECT ?m WHERE {{
           ?x sem:inMedia ?m ; sem:forPerformer ?p ; sem:ofTag <urn:semantic-stash:{SERVER}:tag:2> .
           ?y sem:inMedia ?m ; sem:forPerformer ?p ; sem:ofTag <urn:semantic-stash:{SERVER}:tag:3> .
         }}"
    );
    assert!(r.select(&q).unwrap().rows.is_empty());
    // Performer 7 has no direct link to a Role tag.
    let q = format!(
        "SELECT ?o WHERE {{ <urn:semantic-stash:{SERVER}:performer:7> ?p <urn:semantic-stash:{SERVER}:tag:2> }}"
    );
    assert!(r.select(&q).unwrap().rows.is_empty());
}

#[test]
fn us1_3_an_action_instance_carries_its_variations() {
    let m = mapped();
    for v in ["making out", "peck"] {
        assert!(has(
            &m,
            "scene:100/action:7:4",
            "hasVariation",
            Term::string(v)
        ));
    }
}

#[test]
fn us1_4_a_role_under_a_role_is_a_broader_role() {
    let m = mapped();
    let rdf_type = Fact::new(
        item("tag:2"),
        Term::iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
        Term::iri(format!("{SEM}Role")),
    );
    assert!(m.facts.contains(&rdf_type));
    assert!(has(&m, "tag:2", "parentTag", item("tag:1")));
    assert!(has(&m, "tag:2", "broaderRole", item("tag:1")));
}

#[test]
fn us1_5_label_rules_by_id_and_derived_fields_resolve() {
    let m = mapped();
    for id in ["r1", "r2", "r3", "r4", "r5", "r6"] {
        assert!(
            has(&m, "tag:8", "hasRule", item(&format!("tag:8/rule:{id}"))),
            "{id}"
        );
    }
    let store = published("derived", &m);
    let q = format!(
        "PREFIX sem: <{SEM}>
         SELECT ?property WHERE {{
           <urn:semantic-stash:{SERVER}:tag:8/rule:r2> sem:hasField ?f .
           ?property sem:pluginFieldName ?f .
         }}"
    );
    let rows = store.reader().unwrap().select(&q).unwrap().rows;
    assert_eq!(rows, vec![vec![Some(Term::iri(format!("{SEM}bustSize")))]]);
}

#[test]
fn us1_6_a_performer_has_traits_labels_measurements_and_archetype_parents() {
    let m = mapped();
    assert!(has(&m, "performer:7", "hasTrait", item("tag:7")));
    assert!(has(&m, "performer:7", "hasLabel", item("tag:8")));
    for p in ["bustSize", "waist", "hip", "waistToHipRatio"] {
        assert!(
            m.facts.iter().any(|f| f.subject == item("performer:7")
                && f.predicate == Term::iri(format!("{SEM}{p}"))),
            "{p}"
        );
    }
    assert!(has(
        &m,
        "performer:20",
        "parentArchetype",
        item("performer:21")
    ));
    assert!(has(
        &m,
        "performer:20",
        "parentArchetype",
        item("performer:22")
    ));
}

#[test]
fn us1_7_every_item_iri_holds_the_server_id() {
    let m = mapped();
    for f in &m.facts {
        for t in [&f.subject, &f.object] {
            if let Term::Iri(i) = t {
                if !i.starts_with(SEM) && !i.starts_with("http://www.w3.org/") {
                    assert!(
                        i.starts_with(&format!("urn:semantic-stash:{SERVER}:")),
                        "{i}"
                    );
                }
            }
        }
    }
}
