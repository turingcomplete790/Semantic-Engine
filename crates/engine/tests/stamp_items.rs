//! The build stamp's item counts (spec 009 T036; research R9).

mod common;

use common::build::SERVER;
use common::update::{new_store, owner_counts, stamp_items, Library};
use engine::build::stamp::BuildStamp;
use engine::build::{run, BuildOptions};

#[test]
fn a_full_build_counts_its_owners_per_kind() {
    let lib = Library::fixture();
    let store = new_store("items-build");
    run(
        &lib.reader(),
        &store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
    let items = stamp_items(&store).expect("a build records items");
    assert_eq!(items, owner_counts(&store));
    assert!(items.tags > 0 && items.performers > 0 && items.scenes > 0 && items.images > 0);
}

#[test]
fn a_stamp_without_items_reads_back_as_absent() {
    let mut v = BuildStamp::new(Default::default(), engine::build::stamp::How::Build).to_json();
    v.as_object_mut().unwrap().remove("items");
    let generation = engine::graph::Generation {
        number: 1,
        published_at: "2026-10-28T00:00:00Z".into(),
        facts: 0,
        format: engine::graph::STORE_FORMAT,
        server_id: SERVER.into(),
        notes: Vec::new(),
        build: Some(v),
    };
    let stamp = BuildStamp::read(&generation).expect("still a stamp");
    assert_eq!(stamp.items, None);
}
