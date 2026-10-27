//! Updates follow the build's safety rules (spec 008 T019; US3, FR-013 to FR-015).

mod common;

use common::build::SERVER;
use common::update::{all_facts, new_store, rebuilt_facts, Library};
use engine::build::update::{update, Mode};
use engine::build::{run, BuildError, BuildOptions, NoteReason};
use serde_json::json;
use stash_adapter::reader::{MediaKind, MemoryReader};

fn built(name: &str) -> (Library, engine::graph::GraphStore) {
    let lib = Library::fixture();
    let store = new_store(name);
    run(
        &lib.reader(),
        &store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
    (lib, store)
}

fn go(
    reader: &MemoryReader,
    store: &engine::graph::GraphStore,
    server: &str,
) -> Result<engine::build::update::UpdateReport, BuildError> {
    update(
        reader,
        store,
        server,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
}

#[test]
fn a_held_writer_is_busy() {
    let (lib, store) = built("fail-busy");
    let _held = store.writer().unwrap();
    assert!(matches!(
        go(&lib.reader(), &store, SERVER),
        Err(BuildError::Busy)
    ));
}

#[test]
fn no_server_id_ends_at_once() {
    let (lib, store) = built("fail-noid");
    assert!(matches!(
        go(&lib.reader(), &store, ""),
        Err(BuildError::NoServerId)
    ));
}

#[test]
fn a_reader_failure_publishes_nothing_and_the_next_update_catches_up() {
    let (mut lib, store) = built("fail-reader");
    lib.edit_media_field(
        MediaKind::Scene,
        "101",
        "roleTagAssignments",
        json!({"7": ["2"]}),
    );
    let failing = lib.reader().fail_with(stash_adapter::Error::Unreachable(
        "connection refused".into(),
    ));
    assert!(matches!(
        go(&failing, &store, SERVER),
        Err(BuildError::Stash(_))
    ));
    assert_eq!(store.reader().unwrap().generation().number, 1);
    let r = go(&lib.reader(), &store, SERVER).unwrap();
    assert_eq!(r.mode, Mode::Update);
    assert_eq!(all_facts(&store), rebuilt_facts(&lib, "fail-reader"));
}

#[test]
fn bad_fields_of_re_read_items_are_noted() {
    let (mut lib, store) = built("fail-notes");
    lib.edit_media_field(
        MediaKind::Scene,
        "101",
        "roleTagAssignments",
        json!({"x7": ["2"]}),
    );
    let r = go(&lib.reader(), &store, SERVER).unwrap();
    let invalid = r
        .notes
        .skipped
        .get(&NoteReason::InvalidId)
        .copied()
        .unwrap_or(0);
    assert!(invalid >= 1, "{:?}", r.notes);
}
