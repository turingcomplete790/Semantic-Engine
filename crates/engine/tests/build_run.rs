//! A build through a stand-in reader into a temporary store (spec 007 T022; US2).

mod common;

use common::build::{fixture_library, iris, SERVER};
use engine::build::{
    map_library, run, vocabulary_facts, BuildError, BuildOptions, BuildReport, LogLevel,
};
use engine::graph::{GraphStore, Health, Term};
use stash_adapter::reader::{LibraryRead, MemoryReader};
use std::collections::BTreeSet;

fn store(name: &str) -> GraphStore {
    let dir = std::env::temp_dir().join(format!("se-build-run-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    GraphStore::new(dir.join("graph"), SERVER)
}

struct Outcome {
    result: Result<BuildReport, BuildError>,
    progress: Vec<f64>,
    log: Vec<(LogLevel, String)>,
}

fn build_with(
    store: &GraphStore,
    read: LibraryRead,
    server: &str,
    options: &BuildOptions,
) -> Outcome {
    build_reader(store, &MemoryReader::new(read), server, options)
}

fn build_reader(
    store: &GraphStore,
    reader: &MemoryReader,
    server: &str,
    options: &BuildOptions,
) -> Outcome {
    let mut progress = Vec::new();
    let mut log = Vec::new();
    let result = run(
        reader,
        store,
        server,
        options,
        &mut |p| progress.push(p),
        &mut |l, m| log.push((l, m.to_string())),
    );
    Outcome {
        result,
        progress,
        log,
    }
}

fn build(store: &GraphStore, read: LibraryRead) -> Outcome {
    build_with(store, read, SERVER, &BuildOptions::default())
}

/// The fact count a build of `read` publishes: its facts plus the vocabulary's.
fn expected_facts(read: &LibraryRead) -> u64 {
    let mut all: BTreeSet<_> = map_library(read, &iris()).facts;
    all.extend(vocabulary_facts());
    all.len() as u64
}

fn scene_facts(store: &GraphStore, id: &str) -> usize {
    let q = format!("SELECT ?p ?o WHERE {{ <urn:semantic-stash:{SERVER}:scene:{id}> ?p ?o }}");
    store.reader().unwrap().select(&q).unwrap().rows.len()
}

#[test]
fn a_first_build_publishes_generation_one_with_the_whole_library() {
    let s = store("first");
    let read = fixture_library();
    let out = build(&s, read.clone());
    let r = out.result.unwrap();
    assert_eq!(r.generation, 1);
    assert_eq!(r.facts, expected_facts(&read));
    assert_eq!(
        (
            r.records.tags,
            r.records.performers,
            r.records.scenes,
            r.records.images
        ),
        (16, 7, 5, 1)
    );
    assert_eq!(r.cleared, None, "a first build is not a clearing");
    assert!(out
        .log
        .iter()
        .any(|(_, m)| m.starts_with("build: generation 1 published")));

    // Progress: whole percents, never decreasing, from at most 30% to 100%.
    assert!(out.progress.len() <= 101, "{}", out.progress.len());
    assert!(out.progress.windows(2).all(|w| w[0] <= w[1]));
    assert!(out.progress[0] <= 0.30);
    assert_eq!(*out.progress.last().unwrap(), 1.0);

    // The vocabulary is in the published generation.
    let q = "SELECT ?f WHERE { <urn:semantic-stash:vocab:bustSize> <urn:semantic-stash:vocab:pluginFieldName> ?f }";
    let rows = s.reader().unwrap().select(q).unwrap().rows;
    assert_eq!(rows, vec![vec![Some(Term::string("__derived_bust"))]]);
}

#[test]
fn a_rebuild_holds_exactly_the_new_library() {
    let s = store("rebuild");
    let read = fixture_library();
    build(&s, read.clone()).result.unwrap();
    assert!(scene_facts(&s, "101") > 0);
    let mut changed = read;
    changed.scenes.retain(|m| m.id != "101");
    let r = build(&s, changed.clone()).result.unwrap();
    assert_eq!(r.generation, 2);
    assert_eq!(r.facts, expected_facts(&changed));
    assert_eq!(
        scene_facts(&s, "101"),
        0,
        "a deleted scene's facts are gone"
    );
}

#[test]
fn a_damaged_store_is_cleared_first_and_says_why() {
    let s = store("damaged");
    build(&s, fixture_library()).result.unwrap();
    std::fs::write(s.dir().join("store.json"), "{not json").unwrap();
    let out = build(&s, fixture_library());
    let r = out.result.unwrap();
    assert_eq!(r.cleared, Some("damaged"));
    assert_eq!(r.generation, 1, "clearing starts the numbering again");
    assert!(out.log.iter().any(|(_, m)| m.contains("cleared first")));
}

#[test]
fn a_build_while_another_writer_works_ends_at_once_changing_nothing() {
    let s = store("busy");
    build(&s, fixture_library()).result.unwrap();
    let held = s.writer().unwrap();
    let out = build(&s, fixture_library());
    assert!(
        matches!(out.result, Err(BuildError::Busy)),
        "{:?}",
        out.result.err()
    );
    drop(held);
    assert!(matches!(
        s.check().unwrap(),
        Health::Ready { generation: 1 }
    ));
}

#[test]
fn no_server_id_ends_before_any_read() {
    let dir = std::env::temp_dir().join(format!("se-build-run-noid-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let s = GraphStore::new(dir.join("graph"), "");
    let reader = MemoryReader::new(fixture_library())
        .fail_with(stash_adapter::Error::Unreachable("must not be read".into()));
    let out = build_reader(&s, &reader, "", &BuildOptions::default());
    let e = out.result.unwrap_err();
    assert!(matches!(e, BuildError::NoServerId), "{e:?}");
    assert!(e.to_string().contains("Generate server identifier"));
    assert!(!dir.join("graph").exists(), "nothing was written");
}

#[test]
fn a_reader_failure_publishes_nothing_and_keeps_the_previous_generation() {
    let s = store("unreachable");
    build(&s, fixture_library()).result.unwrap();
    let reader = MemoryReader::new(fixture_library()).fail_with(stash_adapter::Error::Unreachable(
        "connection refused".into(),
    ));
    let out = build_reader(&s, &reader, SERVER, &BuildOptions::default());
    assert!(matches!(out.result, Err(BuildError::Stash(_))));
    assert!(matches!(
        s.check().unwrap(),
        Health::Ready { generation: 1 }
    ));
    assert!(scene_facts(&s, "100") > 0);
    // The next build starts from the published state and succeeds.
    assert_eq!(build(&s, fixture_library()).result.unwrap().generation, 2);
}

#[test]
fn an_empty_library_publishes_an_empty_but_built_graph() {
    let s = store("empty");
    let r = build(&s, LibraryRead::default()).result.unwrap();
    assert_eq!(r.generation, 1);
    assert_eq!(
        r.facts,
        vocabulary_facts()
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len() as u64
    );
    assert!(matches!(
        s.check().unwrap(),
        Health::Ready { generation: 1 }
    ));
}

#[test]
fn small_batches_give_the_same_graph() {
    let s = store("batches");
    let read = fixture_library();
    let options = BuildOptions {
        batch_facts: 7,
        ..BuildOptions::default()
    };
    let r = build_with(&s, read.clone(), SERVER, &options)
        .result
        .unwrap();
    assert_eq!(r.facts, expected_facts(&read));
}

#[test]
fn notes_are_logged_up_to_the_cap_and_counted_in_the_summary() {
    let s = store("cap");
    let options = BuildOptions {
        note_log_cap: 2,
        ..BuildOptions::default()
    };
    let out = build_with(&s, fixture_library(), SERVER, &options);
    let r = out.result.unwrap();
    let total: usize =
        r.notes.skipped.values().sum::<usize>() + r.notes.mismatches.values().sum::<usize>();
    let noted: Vec<&String> = out
        .log
        .iter()
        .filter(|(l, m)| *l == LogLevel::Warning && m.starts_with("note: "))
        .map(|(_, m)| m)
        .collect();
    assert_eq!(noted.len(), 2);
    assert!(out
        .log
        .iter()
        .any(|(_, m)| m == &format!("notes: {} more not written", total - 2)));
    let summary = &out.log.last().unwrap().1;
    assert!(summary.contains("skipped 16 ("), "{summary}");
    assert!(summary.contains("mismatches 6 ("), "{summary}");
    assert!(summary.contains("unreadable 5"), "{summary}");
}
