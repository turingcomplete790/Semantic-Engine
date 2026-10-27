//! An update reads in proportion to what changed (spec 008 T022; US4, FR-019, SC-006).

mod common;

use common::build::SERVER;
use common::update::{new_store, CountingReader, Library};
use engine::build::update::{update, Mode};
use engine::build::{run, synth, BuildOptions};
use serde_json::json;
use stash_adapter::reader::{MediaKind, TagFlag};
use std::collections::BTreeSet;

fn built(name: &str) -> (Library, engine::graph::GraphStore) {
    let lib = Library::new(synth::library(2000, 500));
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

fn counted(
    lib: &Library,
    store: &engine::graph::GraphStore,
    check: impl FnOnce(&CountingReader, Mode),
) {
    let inner = lib.reader();
    let reader = CountingReader::new(&inner);
    let r = update(
        &reader,
        store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(
        reader.count("full_read"),
        0,
        "an update never reads the whole library"
    );
    check(&reader, r.mode);
}

/// The newest item of each kind sits in the catch-up second, so an update re-reads it.
fn newest(lib: &Library, kind: MediaKind) -> String {
    let list = match kind {
        MediaKind::Scene => &lib.read.scenes,
        MediaKind::Image => &lib.read.images,
    };
    list.iter()
        .max_by_key(|m| m.updated_at.clone())
        .unwrap()
        .id
        .clone()
}

#[test]
fn no_change_reads_no_records_beyond_the_catch_up_second() {
    let (lib, store) = built("reads-none");
    counted(&lib, &store, |r, mode| {
        assert_eq!(mode, Mode::Current);
        assert_eq!(r.count("semantic_tags"), 1);
        assert_eq!(
            r.count("changed_since"),
            6,
            "performers, scenes, images: from one second early, and after the point (R13)"
        );
        assert_eq!(r.count("all_ids"), 3);
        assert!(
            r.asked("scene_records").len() <= 1,
            "{:?}",
            r.asked("scene_records")
        );
        assert!(r.asked("image_records").len() <= 1);
        assert!(r.asked("performer_records").len() <= 1);
    });
}

#[test]
fn changed_scenes_are_the_only_scenes_read() {
    for n in [1usize, 100] {
        let (mut lib, store) = built(&format!("reads-{n}"));
        let latest = newest(&lib, MediaKind::Scene);
        let mut expected: BTreeSet<String> = BTreeSet::from([latest]);
        for i in 1..=n {
            lib.edit_media_field(
                MediaKind::Scene,
                &i.to_string(),
                "pluggableAssignments",
                json!({}),
            );
            expected.insert(i.to_string());
        }
        counted(&lib, &store, |r, mode| {
            assert_eq!(mode, Mode::Update);
            assert_eq!(r.asked("scene_records"), expected, "{n} changes");
            assert!(
                r.asked("performer_records").len() <= 1,
                "no performer depends on a scene"
            );
        });
    }
}

#[test]
fn a_tag_flag_change_reads_only_its_carriers() {
    let (mut lib, store) = built("reads-tag");
    // Synthetic tag 1 is the first Pluggable; a performer carries it once it is a Trait.
    lib.set_performer_tags("3", &["1"]);
    lib.set_tag_flag("1", TagFlag::Trait, true);
    counted(&lib, &store, |r, mode| {
        assert_eq!(mode, Mode::Update);
        let asked = r.asked("performer_records");
        assert!(asked.contains("3"), "{asked:?}");
        assert!(
            asked.len() <= 3,
            "the carrier, the changed performer and the newest: {asked:?}"
        );
        assert_eq!(
            r.asked("performers_with_tags"),
            BTreeSet::from(["1".to_string()])
        );
    });
}
