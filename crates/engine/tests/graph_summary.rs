//! What the store holds (spec 006 T020; US4, FR-018).

mod common;

use common::graph::{cleared, publish, sample_facts, SERVER};
use engine::graph::{Health, STORE_FORMAT};
use std::time::Instant;

fn folder_size(p: &std::path::Path) -> u64 {
    if p.is_dir() {
        std::fs::read_dir(p)
            .unwrap()
            .map(|e| folder_size(&e.unwrap().path()))
            .sum()
    } else {
        std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
    }
}

#[test]
fn a_published_store_reports_every_field() {
    let (dir, store) = cleared("summary");
    publish(&store, &sample_facts(1));
    publish(&store, &sample_facts(2));
    let last = publish(&store, &sample_facts(3));
    let s = store.summary().unwrap();
    assert_eq!(s.health, Health::Ready { generation: 3 });
    let g = s.generation.unwrap();
    assert_eq!((g.number, g.facts), (3, 9));
    assert_eq!(g.published_at, last.published_at);
    assert!(
        g.published_at.ends_with('Z') && g.published_at.len() == 20,
        "{}",
        g.published_at
    );
    assert_eq!(s.format, STORE_FORMAT);
    assert_eq!(s.server_id.as_deref(), Some(SERVER));
    assert!(s.disk_bytes > 0);
    assert_eq!(s.disk_bytes, folder_size(&dir));
    assert!(!s.writing);
}

#[test]
fn a_write_in_progress_is_reported_without_waiting() {
    let (_, store) = cleared("writing");
    publish(&store, &sample_facts(1));
    let w = store.writer().unwrap();
    let t = Instant::now();
    let s = store.summary().unwrap();
    assert!(t.elapsed().as_millis() < 50, "took {:?}", t.elapsed());
    assert!(s.writing);
    drop(w);
    assert!(!store.summary().unwrap().writing);
}

#[test]
fn a_store_not_built_yet_has_no_generation() {
    let (_, store) = cleared("summary-notbuilt");
    let s = store.summary().unwrap();
    assert_eq!(s.health, Health::NotBuilt);
    assert!(s.generation.is_none());
    assert_eq!(s.server_id.as_deref(), Some(SERVER));
}
