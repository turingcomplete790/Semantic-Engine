//! Snapshot generation layout (data-model "Snapshot generation"; tasks T008).

use oxigraph::model::{GraphNameRef, Literal, NamedNode, Quad};
use spike::store::{self, LockMode, StoreError};
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("spike-store-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn quad(i: usize) -> Quad {
    Quad::new(
        NamedNode::new(format!("urn:test:s{i}")).unwrap(),
        NamedNode::new("urn:test:p").unwrap(),
        Literal::from(i as i64),
        GraphNameRef::DefaultGraph,
    )
}

fn write_n(w: &store::Writer, from: usize, n: usize) {
    for i in from..from + n {
        w.store().insert(&quad(i)).unwrap();
    }
}

#[test]
fn publish_creates_a_new_generation_and_points_current_at_it() {
    let dir = tmp("publish");
    let w = store::open_writer(&dir, LockMode::Fail).unwrap();
    assert!(dir.join("writer.lock").exists());
    write_n(&w, 0, 10);
    assert_eq!(store::publish(&w).unwrap(), 1);
    write_n(&w, 10, 5);
    assert_eq!(store::publish(&w).unwrap(), 2);
    drop(w);

    assert_eq!(fs::read_to_string(dir.join("current")).unwrap().trim(), "2");
    assert!(dir.join("snapshots/1").is_dir());
    assert!(dir.join("snapshots/2").is_dir());
    assert!(
        !dir.join("current.tmp").exists(),
        "temporary file renamed away"
    );
    let (generation, s) = store::open_current_read_only(&dir).unwrap();
    assert_eq!(generation, 2);
    assert_eq!(s.len().unwrap(), 15);
}

#[test]
fn a_reader_during_a_write_sees_the_previous_complete_generation() {
    let dir = tmp("isolation");
    let w = store::open_writer(&dir, LockMode::Fail).unwrap();
    write_n(&w, 0, 10);
    store::publish(&w).unwrap();
    // A write in progress, not yet published.
    write_n(&w, 10, 90);
    let (generation, s) = store::open_current_read_only(&dir).unwrap();
    assert_eq!(generation, 1);
    assert_eq!(s.len().unwrap(), 10);
    drop(s);
    store::publish(&w).unwrap();
    let (generation, s) = store::open_current_read_only(&dir).unwrap();
    assert_eq!(generation, 2);
    assert_eq!(s.len().unwrap(), 100);
}

#[test]
fn a_second_writer_fails_at_once_or_waits() {
    let dir = tmp("lock");
    let w = store::open_writer(&dir, LockMode::Fail).unwrap();

    let t = Instant::now();
    assert!(matches!(
        store::open_writer(&dir, LockMode::Fail),
        Err(StoreError::Busy)
    ));
    assert!(t.elapsed() < Duration::from_millis(500));

    let t = Instant::now();
    assert!(matches!(
        store::open_writer(&dir, LockMode::Wait(Duration::from_millis(200))),
        Err(StoreError::LockTimeout)
    ));
    assert!(t.elapsed() >= Duration::from_millis(200));

    let d2 = dir.clone();
    let waiter = thread::spawn(move || {
        store::open_writer(&d2, LockMode::Wait(Duration::from_secs(10))).map(|_| ())
    });
    thread::sleep(Duration::from_millis(300));
    drop(w);
    waiter
        .join()
        .unwrap()
        .expect("succeeds once the first writer releases");
}

#[test]
fn gc_keeps_the_newest_two_and_removes_old_generations() {
    let dir = tmp("gc");
    let w = store::open_writer(&dir, LockMode::Fail).unwrap();
    for g in 0..5 {
        write_n(&w, g * 10, 10);
        store::publish(&w).unwrap();
    }
    drop(w);
    // Nothing is past a long grace period.
    assert!(store::gc(&dir, Duration::from_secs(3600))
        .unwrap()
        .is_empty());
    // With no grace period, generations 1 to 3 go; 4 and 5 (the newest two) stay.
    let removed = store::gc(&dir, Duration::ZERO).unwrap();
    assert_eq!(removed, vec![1, 2, 3]);
    let left: Vec<String> = fs::read_dir(dir.join("snapshots"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    let mut left = left;
    left.sort();
    assert_eq!(left, vec!["4", "5"]);
    assert_eq!(store::open_current_read_only(&dir).unwrap().0, 5);
}

#[test]
fn missing_or_damaged_current_is_a_distinct_error() {
    let dir = tmp("damage");
    assert!(matches!(
        store::open_current_read_only(&dir),
        Err(StoreError::NoCurrent)
    ));
    fs::write(dir.join("current"), "not-a-number").unwrap();
    assert!(matches!(
        store::open_current_read_only(&dir),
        Err(StoreError::DamagedCurrent(_))
    ));
    fs::write(dir.join("current"), "7").unwrap();
    assert!(matches!(
        store::open_current_read_only(&dir),
        Err(StoreError::DamagedCurrent(_))
    ));
}

#[test]
fn live_read_only_opens_the_working_store() {
    let dir = tmp("live");
    let w = store::open_writer(&dir, LockMode::Fail).unwrap();
    write_n(&w, 0, 3);
    w.store().flush().unwrap();
    drop(w);
    let s = store::open_live_read_only(&dir).unwrap();
    assert_eq!(s.len().unwrap(), 3);
}
