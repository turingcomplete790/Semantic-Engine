//! An update that cannot be trusted rebuilds in full and names why (spec 008 T016; US2, FR-010,
//! SC-005).

mod common;

use common::build::SERVER;
use common::update::{all_facts, new_store, rebuilt_facts, Library};
use engine::build::synth;
use engine::build::update::{update, Mode, RebuildCause, UpdateReport};
use engine::build::{run, BuildOptions};
use engine::graph::GraphStore;
use serde_json::{json, Value};
use stash_adapter::reader::MediaKind;

fn build(lib: &Library, store: &GraphStore) {
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

fn go(lib: &Library, store: &GraphStore) -> UpdateReport {
    update(
        &lib.reader(),
        store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap()
}

fn rebuilt_because(lib: &Library, store: &GraphStore, name: &str) -> RebuildCause {
    let r = go(lib, store);
    assert_eq!(r.mode, Mode::Rebuild, "{name}");
    assert_eq!(
        all_facts(store),
        rebuilt_facts(lib, name),
        "{name}: the rebuild equals a full build"
    );
    r.rebuild_reason.expect("a reason")
}

/// Rewrites the current generation file's `build` stamp.
fn edit_stamp(store: &GraphStore, edit: impl FnOnce(&mut Value)) {
    let n = store.reader().unwrap().generation().number;
    let path = store.dir().join(format!("snapshots/{n}.json"));
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    edit(&mut v);
    std::fs::write(&path, v.to_string()).unwrap();
}

#[test]
fn no_store_is_a_first_build() {
    let lib = Library::fixture();
    let store = new_store("fb-first");
    assert_eq!(
        rebuilt_because(&lib, &store, "first"),
        RebuildCause::FirstBuild
    );
}

#[test]
fn a_damaged_store_is_cleared_and_rebuilt() {
    let lib = Library::fixture();
    let store = new_store("fb-damaged");
    build(&lib, &store);
    std::fs::write(store.dir().join("store.json"), "{nope").unwrap();
    assert_eq!(
        rebuilt_because(&lib, &store, "damaged"),
        RebuildCause::StoreNeedsRebuild { cleared: "damaged" }
    );
}

#[test]
fn a_format_1_store_is_rebuilt() {
    let lib = Library::fixture();
    let store = new_store("fb-format");
    build(&lib, &store);
    let path = store.dir().join("store.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    v["format"] = json!(1);
    std::fs::write(&path, v.to_string()).unwrap();
    assert_eq!(
        rebuilt_because(&lib, &store, "format"),
        RebuildCause::StoreNeedsRebuild {
            cleared: "otherFormat"
        }
    );
}

#[test]
fn another_mapping_or_vocabulary_version_is_rebuilt() {
    let lib = Library::fixture();
    let store = new_store("fb-mapping");
    build(&lib, &store);
    edit_stamp(&store, |v| v["build"]["mappingVersion"] = json!(0));
    assert_eq!(
        rebuilt_because(&lib, &store, "mapping"),
        RebuildCause::MappingVersion { from: 0, to: 1 }
    );
    edit_stamp(&store, |v| v["build"]["vocabularyVersion"] = json!("1.3.0"));
    match rebuilt_because(&lib, &store, "vocabulary") {
        RebuildCause::VocabularyVersion { from, .. } => assert_eq!(from, "1.3.0"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_or_broken_stamp_is_rebuilt() {
    let lib = Library::fixture();
    let store = new_store("fb-stamp");
    build(&lib, &store);
    edit_stamp(&store, |v| {
        v.as_object_mut().unwrap().remove("build");
    });
    assert_eq!(
        rebuilt_because(&lib, &store, "no stamp"),
        RebuildCause::NoCatchUpPoint
    );
    edit_stamp(&store, |v| v["build"] = json!({ "mappingVersion": 1 }));
    assert_eq!(
        rebuilt_because(&lib, &store, "partial stamp"),
        RebuildCause::NoCatchUpPoint
    );
}

#[test]
fn too_many_changes_rebuild() {
    let mut lib = Library::new(synth::library(2000, 400));
    let store = new_store("fb-many");
    build(&lib, &store);
    for i in 1..=900 {
        lib.edit_media_field(
            MediaKind::Scene,
            &i.to_string(),
            "pluggableAssignments",
            json!({}),
        );
    }
    match rebuilt_because(&lib, &store, "many") {
        RebuildCause::TooManyChanges { items, limit } => {
            // Only changes after the catch-up second count (research R13).
            assert_eq!(items, 900);
            assert!(limit < 900, "{limit}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_crowded_catch_up_second_does_not_force_a_rebuild() {
    // A bulk save: 700 scenes share the newest second, so every update re-reads them.
    let mut lib = Library::new(synth::library(1000, 0));
    let crowded = lib
        .read
        .scenes
        .iter()
        .map(|m| m.updated_at.clone())
        .max()
        .unwrap();
    for m in lib.read.scenes.iter_mut().skip(300) {
        m.updated_at = crowded.clone();
    }
    let store = new_store("fb-crowded");
    build(&lib, &store);
    lib.edit_media_field(MediaKind::Scene, "1", "pluggableAssignments", json!({}));
    let r = go(&lib, &store);
    assert_eq!(r.mode, Mode::Update, "{:?}", r.rebuild_reason);
    assert_eq!(all_facts(&store), rebuilt_facts(&lib, "crowded"));
}
