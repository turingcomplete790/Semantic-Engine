//! Shared helpers for the graph store tests (spec 006 T002).

use engine::graph::{Fact, GraphStore, Term};
use std::path::PathBuf;

pub const SERVER: &str = "00000000-0000-4000-8000-000000000006";

/// A fresh, empty folder and a store on it for `SERVER`.
pub fn temp_store(name: &str) -> (PathBuf, GraphStore) {
    let dir = std::env::temp_dir().join(format!("se-graph-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = GraphStore::new(dir.join("graph"), SERVER);
    (dir.join("graph"), store)
}

/// A stamped store, ready for a first write.
pub fn cleared(name: &str) -> (PathBuf, GraphStore) {
    let (dir, store) = temp_store(name);
    store.clear_for_rebuild().unwrap();
    (dir, store)
}

pub fn iri(s: &str) -> Term {
    Term::iri(format!("urn:t:{s}"))
}

pub fn lit(s: &str) -> Term {
    Term::string(s)
}

pub fn typed(s: &str, dt: &str) -> Term {
    Term::typed(s, dt)
}

pub fn fact(s: &str, p: &str, o: Term) -> Fact {
    Fact::new(iri(s), iri(p), o)
}

/// `n` scenes, each with 2 performers and a title.
pub fn sample_facts(n: usize) -> Vec<Fact> {
    let mut out = Vec::new();
    for i in 0..n {
        let s = format!("scene/{i}");
        out.push(fact(&s, "performer", iri(&format!("performer/{}", 2 * i))));
        out.push(fact(
            &s,
            "performer",
            iri(&format!("performer/{}", 2 * i + 1)),
        ));
        out.push(fact(&s, "title", lit(&format!("Scene {i}"))));
    }
    out
}

/// Publishes `facts` as one generation on a writer.
pub fn publish(store: &GraphStore, facts: &[Fact]) -> engine::graph::Generation {
    let mut w = store.writer().unwrap();
    w.add(facts).unwrap();
    w.publish().unwrap()
}
