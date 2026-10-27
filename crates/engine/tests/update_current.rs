//! An update with nothing to change publishes nothing (spec 008 T012; US1-7, FR-008, SC-003).

mod common;

use common::build::SERVER;
use common::update::{all_facts, new_store, rebuilt_facts, Library};
use engine::build::update::{update, Mode};
use engine::build::{run, BuildOptions, LogLevel};
use serde_json::json;
use stash_adapter::reader::MediaKind;

fn build(lib: &Library, store: &engine::graph::GraphStore) {
    run(
        &lib.reader(),
        store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
}

fn update_logged(
    lib: &Library,
    store: &engine::graph::GraphStore,
) -> (engine::build::update::UpdateReport, Vec<String>) {
    let mut log = Vec::new();
    let r = update(
        &lib.reader(),
        store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |l: LogLevel, m: &str| {
            log.push(m.to_string());
            let _ = l;
        },
    )
    .unwrap();
    (r, log)
}

#[test]
fn no_change_publishes_nothing() {
    let lib = Library::fixture();
    let store = new_store("current");
    build(&lib, &store);
    let (r, log) = update_logged(&lib, &store);
    assert_eq!(r.mode, Mode::Current);
    assert_eq!(r.generation, 1);
    assert!(
        log.iter().any(|m| m.contains("the graph is current")),
        "{log:?}"
    );
    assert_eq!(store.reader().unwrap().generation().number, 1);
}

#[test]
fn a_save_that_changes_no_fact_publishes_nothing_and_a_real_change_still_applies() {
    let mut lib = Library::fixture();
    let store = new_store("noop-save");
    build(&lib, &store);
    // The same value saved again: `updated_at` moves, the facts do not.
    lib.edit_media_field(
        MediaKind::Scene,
        "101",
        "roleTagAssignments",
        json!({"7": ["3"], "9": ["2"]}),
    );
    let (r, _) = update_logged(&lib, &store);
    assert_eq!(r.mode, Mode::Current);
    assert_eq!(store.reader().unwrap().generation().number, 1);

    lib.edit_media_field(
        MediaKind::Scene,
        "101",
        "roleTagAssignments",
        json!({"7": ["2"]}),
    );
    let (r, _) = update_logged(&lib, &store);
    assert_eq!(r.mode, Mode::Update);
    assert_eq!(r.generation, 2);
    assert_eq!(all_facts(&store), rebuilt_facts(&lib, "noop-then-real"));
}
