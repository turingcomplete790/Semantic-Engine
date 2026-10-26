//! Writing and publishing (spec 006 T010; US1, FR-004 to FR-009, SC-004, SC-005).

mod common;

use common::graph::{cleared, fact, iri, lit, publish, sample_facts};
use engine::graph::{GraphError, Pattern};
use std::time::Instant;

#[test]
fn the_first_publish_is_generation_one_with_its_facts() {
    let (_, store) = cleared("first");
    let facts = sample_facts(4); // 12 facts
    let g = publish(&store, &facts[..10]);
    assert_eq!((g.number, g.facts), (1, 10));
    let r = store.reader().unwrap();
    assert_eq!(r.generation().number, 1);
    for f in &facts[..10] {
        assert!(r.contains(f).unwrap(), "{f:?}");
    }
}

#[test]
fn unpublished_changes_are_invisible() {
    let (_, store) = cleared("unpublished");
    publish(&store, &sample_facts(1));
    let extra = fact("scene/x", "title", lit("not yet"));
    let mut w = store.writer().unwrap();
    w.add(std::slice::from_ref(&extra)).unwrap();
    let r = store.reader().unwrap();
    assert_eq!(r.generation().number, 1);
    assert!(!r.contains(&extra).unwrap());
    drop(w);
}

#[test]
fn a_second_writer_is_told_busy_at_once() {
    let (_, store) = cleared("busy");
    let w = store.writer().unwrap();
    let t = Instant::now();
    let second = store.writer();
    let took = t.elapsed();
    assert!(
        matches!(second, Err(GraphError::Busy)),
        "{:?}",
        second.err()
    );
    assert!(took.as_millis() < 50, "took {took:?}");
    assert!(matches!(store.clear_for_rebuild(), Err(GraphError::Busy)));
    drop(w);
    assert!(
        store.writer().is_ok(),
        "the lock is released when the writer goes"
    );
}

#[test]
fn a_writer_dropped_before_publishing_changes_nothing() {
    let (_, store) = cleared("dropped");
    let base = sample_facts(2);
    publish(&store, &base);
    let lost = fact("scene/lost", "title", lit("lost"));
    {
        let mut w = store.writer().unwrap();
        w.add(std::slice::from_ref(&lost)).unwrap();
        w.remove(&base[..1]).unwrap();
    }
    let r = store.reader().unwrap();
    assert_eq!(r.generation().number, 1);
    assert!(r.contains(&base[0]).unwrap());
    // The next writer starts from generation 1, not from the dropped writer's changes.
    let g = publish(&store, &[]);
    assert_eq!(g.number, 2);
    let r = store.reader().unwrap();
    assert!(
        !r.contains(&lost).unwrap(),
        "the dropped change was not published"
    );
    assert!(
        r.contains(&base[0]).unwrap(),
        "the dropped removal was not published"
    );
    assert_eq!(g.facts, base.len() as u64);
}

const CHILD: &str = "SE_GRAPH_ABORT_CHILD";

/// Run as a child process: add a fact and abort without publishing.
#[test]
fn child_abort_during_write() {
    let Ok(dir) = std::env::var(CHILD) else {
        return;
    };
    let store = engine::graph::GraphStore::new(dir, common::graph::SERVER);
    let mut w = store.writer().unwrap();
    w.add(&[fact("scene/aborted", "title", lit("aborted"))])
        .unwrap();
    std::process::abort();
}

#[test]
fn a_process_that_crashes_mid_write_changes_nothing_and_frees_the_lock() {
    let (dir, store) = cleared("crash");
    publish(&store, &sample_facts(2));
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_abort_during_write", "--nocapture"])
        .env(CHILD, &dir)
        .status()
        .unwrap();
    assert!(!status.success(), "the child aborted");
    let r = store.reader().unwrap();
    assert_eq!(r.generation().number, 1);
    assert!(!r
        .contains(&fact("scene/aborted", "title", lit("aborted")))
        .unwrap());
    // The lock died with the process, and the next writer starts from generation 1.
    let g = publish(&store, &[]);
    assert_eq!((g.number, g.facts), (2, 6));
}

#[test]
fn generations_are_numbered_without_gaps_across_a_dropped_writer() {
    let (_, store) = cleared("numbers");
    let mut seen = vec![publish(&store, &sample_facts(1)).number];
    for i in 0..3 {
        if i == 1 {
            let mut w = store.writer().unwrap();
            w.add(&sample_facts(3)).unwrap();
        }
        seen.push(publish(&store, &[fact(&format!("s{i}"), "p", lit("v"))]).number);
    }
    assert_eq!(seen, [1, 2, 3, 4]);
}

#[test]
fn a_change_with_removals_and_additions_is_seen_whole() {
    let (_, store) = cleared("whole");
    let base = sample_facts(4);
    publish(&store, &base[..10]);
    let added: Vec<_> = (0..5)
        .map(|i| fact(&format!("new/{i}"), "p", lit("v")))
        .collect();
    let mut w = store.writer().unwrap();
    w.remove(&base[..5]).unwrap();
    w.add(&added).unwrap();
    let g = w.publish().unwrap();
    assert_eq!(g.facts, 10);
    let r = store.reader().unwrap();
    assert!(base[..5].iter().all(|f| !r.contains(f).unwrap()));
    assert!(base[5..10].iter().all(|f| r.contains(f).unwrap()));
    assert!(added.iter().all(|f| r.contains(f).unwrap()));
}

#[test]
fn pattern_removal_counts_and_clear_empties() {
    let (_, store) = cleared("pattern");
    publish(&store, &sample_facts(3));
    let mut w = store.writer().unwrap();
    let n = w
        .remove_matching(&Pattern {
            subject: Some(iri("scene/1")),
            ..Pattern::default()
        })
        .unwrap();
    assert_eq!(n, 3);
    let g = w.publish().unwrap();
    assert_eq!(g.facts, 6);
    let mut w = store.writer().unwrap();
    w.clear().unwrap();
    assert_eq!(w.publish().unwrap().facts, 0);
}

#[test]
fn adding_a_present_fact_or_removing_an_absent_one_does_nothing() {
    let (_, store) = cleared("noop");
    let base = sample_facts(1);
    publish(&store, &base);
    let mut w = store.writer().unwrap();
    w.add(&base).unwrap();
    w.remove(&[fact("absent", "p", lit("v"))]).unwrap();
    assert_eq!(w.publish().unwrap().facts, 3);
}

#[test]
fn invalid_facts_are_refused_before_any_change() {
    let (_, store) = cleared("invalid");
    publish(&store, &sample_facts(1));
    let mut w = store.writer().unwrap();
    let bad = engine::graph::Fact::new(lit("not a subject"), iri("p"), lit("v"));
    assert!(matches!(w.add(&[bad]), Err(GraphError::InvalidFact { .. })));
}

// spec 007 T008: a full rebuild starts from an empty working store, and the published
// generations stay current until it publishes (research R3).
#[test]
fn rebuild_writer_starts_empty_and_keeps_the_current_generation() {
    let (dir, store) = cleared("rebuild-writer");
    let old = sample_facts(3);
    publish(&store, &old);
    let new = fact("scene/new", "title", lit("rebuilt"));
    let mut w = store.rebuild_writer().unwrap();
    {
        let r = store.reader().unwrap();
        assert_eq!(r.generation().number, 1, "generation 1 stays current");
        assert!(r.contains(&old[0]).unwrap());
    }
    assert!(matches!(store.rebuild_writer(), Err(GraphError::Busy)));
    assert!(matches!(store.writer(), Err(GraphError::Busy)));
    w.add(std::slice::from_ref(&new)).unwrap();
    let g = w.publish().unwrap();
    assert_eq!((g.number, g.facts), (2, 1), "only the new facts");
    let r = store.reader().unwrap();
    assert!(r.contains(&new).unwrap());
    assert!(!r.contains(&old[0]).unwrap());
    drop(r);

    std::fs::write(dir.join("store.json"), "{not json").unwrap();
    assert!(matches!(
        store.rebuild_writer(),
        Err(GraphError::NeedsRebuild(_))
    ));
}

#[test]
fn a_dropped_rebuild_writer_leaves_the_previous_generation() {
    let (_, store) = cleared("rebuild-dropped");
    let old = sample_facts(2);
    publish(&store, &old);
    let w = store.rebuild_writer().unwrap();
    drop(w);
    let mut w = store.writer().unwrap();
    w.add(&[fact("scene/z", "title", lit("z"))]).unwrap();
    let g = w.publish().unwrap();
    assert_eq!(g.facts, old.len() as u64 + 1, "reset from generation 1");
}
