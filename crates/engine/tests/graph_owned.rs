//! Facts owned by items, in format 2 (spec 008 T005; contracts/graph-store-format-2.md, G10–G13).

mod common;

use common::graph::{cleared, fact, iri, lit, temp_store, SERVER};
use engine::graph::{Fact, GraphError, GraphStore, Owned, RebuildReason, Replaced, Term};
use serde_json::json;

fn owner(name: &str) -> String {
    format!("urn:t:{name}")
}

fn facts_of(name: &str, n: usize) -> Vec<Fact> {
    (0..n)
        .map(|i| fact(name, "p", lit(&format!("{name} {i}"))))
        .collect()
}

/// Every fact the published generation answers, through the union of graphs.
fn count(store: &GraphStore) -> u64 {
    let rows = store
        .reader()
        .unwrap()
        .select("SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }")
        .unwrap()
        .rows;
    match &rows[0][0] {
        Some(Term::Literal { value, .. }) => value.parse().unwrap(),
        other => panic!("{other:?}"),
    }
}

fn published_facts(store: &GraphStore) -> u64 {
    store.reader().unwrap().generation().facts
}

#[test]
fn owned_facts_are_added_listed_and_counted() {
    let (_, store) = cleared("owned-add");
    let mut w = store.rebuild_writer().unwrap();
    w.add_owned(&[
        Owned {
            owner: owner("a"),
            facts: facts_of("a", 3),
        },
        Owned {
            owner: owner("b"),
            facts: facts_of("b", 2),
        },
    ])
    .unwrap();
    assert_eq!(w.owners().unwrap(), [owner("a"), owner("b")]);
    let g = w.publish().unwrap();
    assert_eq!(g.facts, 5);
    assert_eq!(
        count(&store),
        5,
        "queries see the union of the graphs (G10)"
    );
    let r = store.reader().unwrap();
    assert!(r.contains(&facts_of("a", 1)[0]).unwrap());
}

#[test]
fn replacing_with_the_same_facts_writes_nothing() {
    let (dir, store) = cleared("owned-same");
    let mut w = store.rebuild_writer().unwrap();
    w.add_owned(&[Owned {
        owner: owner("a"),
        facts: facts_of("a", 3),
    }])
    .unwrap();
    w.publish().unwrap();

    let mut w = store.writer().unwrap();
    assert_eq!(
        w.replace_owned(&owner("a"), &facts_of("a", 3)).unwrap(),
        Replaced::Unchanged
    );
    assert!(
        !dir.join("work.dirty").exists(),
        "an unchanged replace leaves the store clean (G11)"
    );
    let changed = w
        .replace_owned(
            &owner("a"),
            &[facts_of("a", 2), vec![fact("a", "q", lit("new"))]].concat(),
        )
        .unwrap();
    assert_eq!(
        changed,
        Replaced::Changed {
            removed: 1,
            added: 1
        }
    );
    let g = w.publish().unwrap();
    assert_eq!(g.facts, 3);
    assert_eq!(count(&store), 3);
}

#[test]
fn removing_an_owner_removes_only_its_graph() {
    let (_, store) = cleared("owned-remove");
    let mut w = store.rebuild_writer().unwrap();
    w.add_owned(&[
        Owned {
            owner: owner("a"),
            facts: facts_of("a", 3),
        },
        Owned {
            owner: owner("b"),
            facts: facts_of("b", 2),
        },
    ])
    .unwrap();
    w.publish().unwrap();
    let mut w = store.writer().unwrap();
    assert_eq!(w.remove_owned(&owner("a")).unwrap(), 3);
    assert_eq!(w.remove_owned(&owner("nobody")).unwrap(), 0);
    assert_eq!(w.owners().unwrap(), [owner("b")]);
    assert_eq!(w.publish().unwrap().facts, 2);
    assert_eq!(count(&store), 2, "G12");
}

#[test]
fn owners_referencing_finds_the_graphs_that_point_at_an_iri() {
    let (_, store) = cleared("owned-ref");
    let mut w = store.rebuild_writer().unwrap();
    let target = iri("tag/7");
    w.add_owned(&[
        Owned {
            owner: owner("p1"),
            facts: vec![fact("p1", "hasTrait", target.clone())],
        },
        Owned {
            owner: owner("p2"),
            facts: vec![fact("p2", "hasTrait", iri("tag/8"))],
        },
        Owned {
            owner: owner("p3"),
            facts: vec![fact("p3", "hasLabel", target.clone())],
        },
    ])
    .unwrap();
    let Term::Iri(t) = &target else {
        unreachable!()
    };
    assert_eq!(w.owners_referencing(t).unwrap(), [owner("p1"), owner("p3")]);
}

#[test]
fn the_kept_count_matches_after_mixed_changes() {
    let (_, store) = cleared("owned-count");
    let mut w = store.rebuild_writer().unwrap();
    w.add_owned(&[Owned {
        owner: owner("a"),
        facts: facts_of("a", 4),
    }])
    .unwrap();
    w.publish().unwrap();
    let mut w = store.writer().unwrap();
    w.replace_owned(&owner("b"), &facts_of("b", 5)).unwrap();
    w.replace_owned(&owner("a"), &facts_of("a", 2)).unwrap();
    w.remove_owned(&owner("b")).unwrap();
    w.replace_owned(&owner("c"), &facts_of("c", 1)).unwrap();
    w.publish().unwrap();
    assert_eq!(published_facts(&store), count(&store));
    // A default-graph write makes the count unknown, so publish counts the store.
    let mut w = store.writer().unwrap();
    w.add(&[fact("x", "p", lit("x"))]).unwrap();
    w.publish().unwrap();
    assert_eq!(published_facts(&store), count(&store));
}

#[test]
fn publish_with_records_the_build_stamp() {
    let (_, store) = cleared("owned-stamp");
    let mut w = store.rebuild_writer().unwrap();
    w.add_owned(&[Owned {
        owner: owner("a"),
        facts: facts_of("a", 1),
    }])
    .unwrap();
    let g = w.publish_with(json!({ "mappingVersion": 1 })).unwrap();
    assert_eq!(g.build, Some(json!({ "mappingVersion": 1 })));
    let r = store.reader().unwrap();
    assert_eq!(r.generation().build, Some(json!({ "mappingVersion": 1 })));
    // A plain publish has no stamp.
    let w = store.writer().unwrap();
    assert_eq!(w.publish().unwrap().build, None);
}

#[test]
fn a_format_1_store_needs_a_rebuild() {
    let (dir, store) = temp_store("owned-format1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("store.json"),
        json!({ "format": 1, "serverId": SERVER, "createdAt": "2026-10-25T00:00:00Z" }).to_string(),
    )
    .unwrap();
    match store.writer() {
        Err(GraphError::NeedsRebuild(RebuildReason::OtherFormat { found, expected })) => {
            assert_eq!((found, expected), (1, 2))
        }
        other => panic!("{:?}", other.err()),
    }
}
