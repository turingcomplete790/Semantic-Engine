//! A store that repairs itself (spec 006 T017; US3, FR-014 to FR-017, SC-006).

mod common;

use common::graph::{cleared, publish, sample_facts, temp_store, SERVER};
use engine::graph::{GraphError, GraphStore, Health, RebuildReason, GENERATION_GRACE};
use std::path::Path;
use std::time::{Duration, SystemTime};

fn needs(store: &GraphStore) -> RebuildReason {
    match store.check().unwrap() {
        Health::NeedsRebuild(r) => r,
        other => panic!("expected a rebuild, got {other:?}"),
    }
}

/// Every entry point refuses with the same reason, and clearing repairs it.
fn refused_then_repaired(store: &GraphStore, reason: &RebuildReason) {
    for (what, r) in [
        ("reader", store.reader().err()),
        ("writer", store.writer().err()),
        ("summary", store.summary().err()),
    ] {
        match r {
            Some(GraphError::NeedsRebuild(got)) => assert_eq!(&got, reason, "{what}"),
            other if what == "summary" => {
                // summary reports the health instead of failing
                let s = store.summary().unwrap();
                assert_eq!(s.health, Health::NeedsRebuild(reason.clone()), "{other:?}");
            }
            other => panic!("{what}: {other:?}"),
        }
    }
    store.clear_for_rebuild().unwrap();
    assert_eq!(store.check().unwrap(), Health::NotBuilt);
    assert_eq!(publish(store, &sample_facts(1)).number, 1);
    assert!(matches!(
        store.check().unwrap(),
        Health::Ready { generation: 1 }
    ));
}

fn damaged(store: &GraphStore) {
    let r = needs(store);
    assert!(matches!(r, RebuildReason::Damaged { .. }), "{r:?}");
    refused_then_repaired(store, &r);
}

#[test]
fn a_missing_store_or_stamp_is_missing() {
    let (dir, store) = temp_store("missing");
    assert_eq!(needs(&store), RebuildReason::Missing, "no folder");
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(
        needs(&store),
        RebuildReason::Missing,
        "folder without store.json"
    );
    refused_then_repaired(&store, &RebuildReason::Missing);
}

#[test]
fn a_truncated_stamp_is_damaged() {
    let (dir, store) = cleared("truncated");
    publish(&store, &sample_facts(1));
    std::fs::write(dir.join("store.json"), "{\"format\": 1, \"serv").unwrap();
    damaged(&store);
}

#[test]
fn a_garbled_current_is_damaged() {
    let (dir, store) = cleared("garbled");
    publish(&store, &sample_facts(1));
    std::fs::write(dir.join("current"), "abc\n").unwrap();
    damaged(&store);
}

#[test]
fn a_current_naming_a_missing_generation_is_damaged() {
    let (dir, store) = cleared("gone");
    publish(&store, &sample_facts(1));
    std::fs::remove_dir_all(dir.join("snapshots/1")).unwrap();
    damaged(&store);
}

#[test]
fn a_generation_without_its_rocksdb_current_file_is_damaged() {
    let (dir, store) = cleared("rocks");
    publish(&store, &sample_facts(1));
    std::fs::remove_file(dir.join("snapshots/1/CURRENT")).unwrap();
    damaged(&store);
}

fn set_format(dir: &Path, format: u32) {
    let path = dir.join("store.json");
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    v["format"] = format.into();
    std::fs::write(path, v.to_string()).unwrap();
}

#[test]
fn another_format_needs_a_rebuild_naming_both() {
    for found in [0, 2] {
        let (dir, store) = cleared(&format!("format{found}"));
        publish(&store, &sample_facts(1));
        set_format(&dir, found);
        let r = needs(&store);
        assert_eq!(r, RebuildReason::OtherFormat { found, expected: 1 });
        refused_then_repaired(&store, &r);
    }
}

#[test]
fn another_servers_store_is_never_read() {
    let (dir, store) = cleared("server");
    publish(&store, &sample_facts(1));
    let other = GraphStore::new(&dir, "11111111-2222-4333-8444-555555555555");
    assert_eq!(needs(&other), RebuildReason::OtherServer);
    let text = format!("{}", GraphError::NeedsRebuild(RebuildReason::OtherServer));
    assert!(!text.contains(SERVER), "{text}");
    refused_then_repaired(&other, &RebuildReason::OtherServer);
}

#[test]
fn no_server_id_is_its_own_error() {
    let (dir, _) = cleared("noid");
    let store = GraphStore::new(&dir, "");
    assert!(matches!(store.check(), Err(GraphError::NoServerId)));
    assert!(matches!(store.reader(), Err(GraphError::NoServerId)));
    assert!(matches!(store.writer(), Err(GraphError::NoServerId)));
    assert!(matches!(
        store.clear_for_rebuild(),
        Err(GraphError::NoServerId)
    ));
}

fn age(dir: &Path, generation: u64, by: Duration) {
    let t = SystemTime::now() - by;
    let f = std::fs::File::open(dir.join(format!("snapshots/{generation}"))).unwrap();
    f.set_modified(t).unwrap();
}

fn present(dir: &Path) -> Vec<u64> {
    let mut v: Vec<u64> = std::fs::read_dir(dir.join("snapshots"))
        .unwrap()
        .filter_map(|e| {
            let e = e.unwrap();
            e.file_type()
                .unwrap()
                .is_dir()
                .then(|| e.file_name().to_str()?.parse().ok())?
        })
        .collect();
    v.sort_unstable();
    for g in &v {
        assert!(
            dir.join(format!("snapshots/{g}.json")).exists(),
            "record of {g}"
        );
    }
    v
}

#[test]
fn cleanup_keeps_current_previous_and_young_generations() {
    let (dir, store) = cleared("cleanup");
    for _ in 0..6 {
        publish(&store, &sample_facts(1));
    }
    for g in 1..=4 {
        age(&dir, g, Duration::from_secs(3600));
    }
    let g = publish(&store, &sample_facts(1));
    assert_eq!(g.number, 7);
    assert!(g.notes.is_empty(), "{:?}", g.notes);
    assert_eq!(present(&dir), [5, 6, 7]);
    let gone = !dir.join("snapshots/1.json").exists();
    assert!(gone, "an old generation's record goes with it");
}

#[test]
fn cleanup_keeps_every_generation_within_the_grace_period() {
    let (dir, store) = cleared("grace");
    for _ in 0..5 {
        publish(&store, &sample_facts(1));
    }
    age(&dir, 1, GENERATION_GRACE - Duration::from_secs(60));
    publish(&store, &sample_facts(1));
    assert_eq!(present(&dir), [1, 2, 3, 4, 5, 6]);
}

#[test]
fn one_publish_clears_a_backlog() {
    let (dir, store) = cleared("backlog");
    for _ in 0..20 {
        publish(&store, &sample_facts(1));
    }
    for g in 1..=18 {
        age(&dir, g, Duration::from_secs(7200));
    }
    publish(&store, &sample_facts(1));
    assert_eq!(present(&dir), [19, 20, 21]);
}
