//! The change journal, the pending marker, and the catch-up watermark (spec 004 T019; R6, R7).

use plugin::journal::{one_second_earlier, Journal, STALE_AFTER};
use stash_adapter::relations::MediaKind;
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("se-journal-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn append_then_drain_returns_each_item_once_in_order() {
    let j = Journal::new(tmp("drain"));
    j.append(MediaKind::Scene, "5").unwrap();
    j.append(MediaKind::Image, "9").unwrap();
    j.append(MediaKind::Scene, "5").unwrap();
    assert_eq!(
        j.drain().unwrap(),
        vec![
            (MediaKind::Scene, "5".to_string()),
            (MediaKind::Image, "9".to_string())
        ]
    );
    assert!(j.drain().unwrap().is_empty(), "drained entries are gone");
}

#[test]
fn only_the_first_hook_of_a_burst_starts_a_job() {
    let j = Journal::new(tmp("marker"));
    assert!(j.mark_pending().unwrap(), "first: start the job");
    assert!(
        !j.mark_pending().unwrap(),
        "second: a job is already pending"
    );
}

#[test]
fn finishing_reports_entries_that_arrived_during_a_drain() {
    let j = Journal::new(tmp("finish"));
    j.mark_pending().unwrap();
    j.append(MediaKind::Scene, "1").unwrap();
    assert_eq!(j.drain().unwrap().len(), 1);
    // A hook appends while the job works; it sees the marker and does not start another job.
    j.append(MediaKind::Scene, "2").unwrap();
    assert!(!j.mark_pending().unwrap());
    assert!(j.finish().unwrap(), "more work: the job must loop");
    assert_eq!(
        j.drain().unwrap(),
        vec![(MediaKind::Scene, "2".to_string())]
    );
    assert!(!j.finish().unwrap(), "nothing left");
    assert!(
        j.mark_pending().unwrap(),
        "the marker is gone, so the next hook starts a job"
    );
}

#[test]
fn a_stale_marker_from_a_crashed_job_is_replaced() {
    let dir = tmp("stale");
    let j = Journal::new(dir.clone());
    assert!(j.mark_pending().unwrap());
    let old = SystemTime::now() - STALE_AFTER - Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(dir.join("apply.pending"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    assert!(
        j.mark_pending().unwrap(),
        "a stale marker does not block new jobs"
    );
}

#[test]
fn a_drain_interrupted_by_a_crash_is_picked_up_next_time() {
    let dir = tmp("crash");
    std::fs::write(
        dir.join("journal.draining"),
        "{\"type\":\"scene\",\"id\":\"3\"}\n",
    )
    .unwrap();
    let j = Journal::new(dir);
    j.append(MediaKind::Scene, "4").unwrap();
    assert_eq!(
        j.drain().unwrap(),
        vec![
            (MediaKind::Scene, "3".to_string()),
            (MediaKind::Scene, "4".to_string())
        ]
    );
}

#[test]
fn the_watermark_round_trips_per_kind() {
    let j = Journal::new(tmp("watermark"));
    assert!(
        j.watermark().unwrap().is_empty(),
        "first install: no watermark"
    );
    let mut w = BTreeMap::new();
    w.insert(MediaKind::Scene, "2026-10-21T10:00:00-06:00".to_string());
    j.set_watermark(&w).unwrap();
    assert_eq!(j.watermark().unwrap(), w);
}

#[test]
fn thresholds_are_one_second_early_in_the_server_format() {
    assert_eq!(
        one_second_earlier("2026-10-21T10:00:00-06:00").as_deref(),
        Some("2026-10-21T09:59:59-06:00")
    );
    assert_eq!(
        one_second_earlier("2026-01-01T00:00:00+02:00").as_deref(),
        Some("2025-12-31T23:59:59+02:00")
    );
    assert_eq!(
        one_second_earlier("2024-03-01T00:00:00Z").as_deref(),
        Some("2024-02-29T23:59:59Z")
    );
    assert_eq!(one_second_earlier("not a time"), None);
}

// ---- The graph's pending markers (spec 009 T020; research R8)

fn backdate(path: &std::path::Path) {
    let old = SystemTime::now() - STALE_AFTER - Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(old)
        .unwrap();
}

#[test]
fn only_the_first_save_of_a_burst_asks_for_a_graph_update() {
    let dir = tmp("graph-mark");
    let j = Journal::new(dir.clone());
    assert!(!j.graph_pending(), "nothing recorded yet");
    assert!(j.mark_graph_pending().unwrap(), "first: start the update");
    assert!(
        !j.mark_graph_pending().unwrap(),
        "second: one is wanted already"
    );
    assert!(j.graph_pending());
    backdate(&dir.join("graph.pending"));
    assert!(
        j.mark_graph_pending().unwrap(),
        "a stale marker does not block a new update"
    );
    assert!(
        !dir.join("apply.pending").exists(),
        "the relations marker is separate"
    );
}

#[test]
fn an_update_takes_the_marker_and_finishes_it() {
    let dir = tmp("graph-take");
    let j = Journal::new(dir.clone());
    j.mark_graph_pending().unwrap();
    j.take_graph_pending().unwrap();
    assert!(!dir.join("graph.pending").exists());
    assert!(dir.join("graph.applying").exists());
    assert!(j.graph_pending(), "an update is running");
    assert!(
        j.mark_graph_pending().unwrap(),
        "a save after the update took the marker starts another"
    );
    j.finish_graph().unwrap();
    assert!(!dir.join("graph.applying").exists());
    assert!(
        dir.join("graph.pending").exists(),
        "the later save's marker stays"
    );
}

#[test]
fn a_stale_applying_marker_is_not_pending() {
    let dir = tmp("graph-applying-stale");
    let j = Journal::new(dir.clone());
    j.mark_graph_pending().unwrap();
    j.take_graph_pending().unwrap();
    backdate(&dir.join("graph.applying"));
    assert!(!j.graph_pending(), "an update that crashed long ago");
    j.finish_graph().unwrap();
    assert!(!j.graph_pending());
    j.finish_graph().unwrap(); // nothing to remove: no error
}

#[test]
fn a_failed_update_restores_a_stale_marker() {
    let dir = tmp("graph-restore");
    let j = Journal::new(dir.clone());
    j.mark_graph_pending().unwrap();
    j.take_graph_pending().unwrap();
    j.restore_graph_pending().unwrap();
    assert!(!dir.join("graph.applying").exists());
    assert!(j.graph_pending(), "the update is still wanted");
    let age = SystemTime::now()
        .duration_since(
            std::fs::metadata(dir.join("graph.pending"))
                .unwrap()
                .modified()
                .unwrap(),
        )
        .unwrap();
    assert!(age >= STALE_AFTER, "backdated: {age:?}");
    assert!(
        j.mark_graph_pending().unwrap(),
        "so the next save starts a job"
    );
}
