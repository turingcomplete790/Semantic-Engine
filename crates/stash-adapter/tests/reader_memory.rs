//! `MemoryReader` keeps rules B1 to B9 of contracts/reader-interface.md with no network
//! (spec 005 T027; FR-023).

mod common;

use common::{client, library, library_handler, serve};
use stash_adapter::reader::{
    Kind, LibraryRead, LibraryReader, MediaKind, MemoryReader, Phase, RECORD_BATCH,
};
use stash_adapter::Error;
use std::cell::Cell;

/// The fixture library, read once through the network reader.
fn recorded() -> LibraryRead {
    let (port, _) = serve(library_handler(library("library.json")));
    client(port).full_read(&|| false, &mut |_| {}).unwrap()
}

#[test]
fn a_full_read_returns_the_records_it_holds() {
    let lib = recorded();
    let m = MemoryReader::new(lib.clone());
    let mut last = None;
    let read = m.full_read(&|| false, &mut |p| last = Some(p)).unwrap();
    assert_eq!(read.tags, lib.tags);
    assert_eq!(read.performers, lib.performers);
    assert_eq!(read.scenes, lib.scenes);
    assert_eq!(read.images, lib.images);
    assert_eq!(read.start, lib.start);
    assert_eq!(last.unwrap().phase, Phase::Done);
}

#[test]
fn it_stops_between_batches() {
    let mut lib = LibraryRead::default();
    let one = recorded().scenes[0].clone();
    for i in 0..(RECORD_BATCH * 2 + 10) {
        let mut s = one.clone();
        s.id = (1000 + i).to_string();
        lib.scenes.push(s);
    }
    let m = MemoryReader::new(lib);
    let batches = Cell::new(0);
    let r = m.full_read(&|| batches.get() >= 1, &mut |p| {
        if p.phase == Phase::Records {
            batches.set(p.done)
        }
    });
    assert!(matches!(r, Err(Error::Stopped)));
    assert_eq!(batches.get(), 1);
    assert!(matches!(
        m.full_read(&|| true, &mut |_| {}),
        Err(Error::Stopped)
    ));
}

#[test]
fn progress_never_goes_back() {
    let m = MemoryReader::new(recorded());
    let mut shares = Vec::new();
    m.full_read(&|| false, &mut |p| shares.push(p.share()))
        .unwrap();
    assert!(shares.windows(2).all(|w| w[1] >= w[0]), "{shares:?}");
}

#[test]
fn records_of_named_ids_are_sorted_with_missing_ones_listed() {
    let m = MemoryReader::new(recorded());
    let b = m
        .media_records(
            MediaKind::Scene,
            &["23".into(), "99".into(), "20".into(), "20".into()],
        )
        .unwrap();
    let ids: Vec<&str> = b.found.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["20", "23"]);
    assert_eq!(b.missing, ["99"]);
    assert_eq!(
        m.tag_records(&["3".into()]).unwrap().found[0].name,
        "Blonde"
    );
    assert_eq!(m.performer_records(&["13".into()]).unwrap().missing, ["13"]);
}

#[test]
fn change_reads_compare_the_updated_at_text() {
    let m = MemoryReader::new(recorded());
    assert_eq!(
        m.changed_since(Kind::Scene, "2026-10-22T10:00:00-06:00")
            .unwrap(),
        ["21", "23"]
    );
    assert_eq!(
        m.changed_since(Kind::Performer, "2026-10-22T00:00:00-06:00")
            .unwrap(),
        ["14"]
    );
    assert!(m
        .changed_since(Kind::Tag, "2030-01-01T00:00:00-06:00")
        .unwrap()
        .is_empty());
}

#[test]
fn every_id_includes_the_ids_without_semantic_data() {
    let m =
        MemoryReader::new(recorded()).with_ids(Kind::Scene, ["22".to_string(), "20".to_string()]);
    assert_eq!(m.all_ids(Kind::Scene).unwrap(), ["20", "21", "22", "23"]);
    assert_eq!(m.all_ids(Kind::Image).unwrap(), ["30", "31"]);
}

#[test]
fn the_newest_updated_at_is_the_start_of_the_recorded_read() {
    let lib = recorded();
    let m = MemoryReader::new(lib.clone());
    for kind in Kind::ALL {
        assert_eq!(
            m.latest_updated_at(kind).unwrap(),
            lib.start.get(&kind).cloned()
        );
    }
}

#[test]
fn a_failing_reader_fails_every_call() {
    let m = MemoryReader::new(recorded()).fail_with(Error::TimedOut("test".into()));
    assert!(matches!(
        m.full_read(&|| false, &mut |_| {}),
        Err(Error::TimedOut(_))
    ));
    assert!(matches!(m.all_ids(Kind::Tag), Err(Error::TimedOut(_))));
    assert!(matches!(m.tag_records(&[]), Err(Error::TimedOut(_))));
    assert!(matches!(
        m.changed_since(Kind::Tag, "x"),
        Err(Error::TimedOut(_))
    ));
    assert!(matches!(
        m.latest_updated_at(Kind::Tag),
        Err(Error::TimedOut(_))
    ));
}

#[test]
fn a_recorded_read_loads_from_a_file() {
    let lib = recorded();
    let path = std::env::temp_dir().join(format!("se-library-read-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_string(&lib).unwrap()).unwrap();
    let m = MemoryReader::from_file(&path).unwrap();
    assert_eq!(
        m.full_read(&|| false, &mut |_| {}).unwrap().scenes,
        lib.scenes
    );
    let _ = std::fs::remove_file(path);
    assert!(matches!(
        MemoryReader::from_file(std::path::Path::new("/no/such/file.json")),
        Err(Error::Setup(_))
    ));
}

#[test]
fn it_never_writes() {
    // MemoryReader has no write method at all; the trait has none either (B9). This test
    // documents that by using only trait reads on a shared reference.
    let m: &dyn LibraryReader = &MemoryReader::new(LibraryRead::default());
    assert!(m.all_ids(Kind::Tag).unwrap().is_empty());
}

#[test]
fn the_captured_test_instance_library_loads() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test-instance/library-read.json");
    let m = MemoryReader::from_file(&path).unwrap();
    let read = m.full_read(&|| false, &mut |_| {}).unwrap();
    assert_eq!(read.tags.len(), 17);
    assert_eq!(read.performers.len(), 886);
    assert_eq!(read.scenes.len(), 1);
    assert_eq!(read.images.len(), 1);
    let measured = read
        .performers
        .iter()
        .filter(|p| p.measurements.is_some())
        .count();
    assert_eq!(measured, 791);
    assert!(
        read.performers
            .iter()
            .filter_map(|p| p.measurements.as_ref())
            .all(|m| m.bust_size.is_some()),
        "every test-instance measurement parses"
    );
}
