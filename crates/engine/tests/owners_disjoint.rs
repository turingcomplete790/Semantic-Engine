//! No fact belongs to two owners (spec 008 T008; research R1), and the mapping version moves with
//! the mapping rules (research R7).

mod common;

use common::build::{fixture_library, fixture_path, iris};
use engine::build::map::{map_media, map_performer, map_tag, owner_iri, Ctx};
use engine::build::stamp::MAPPING_VERSION;
use engine::build::{synth, vocabulary_facts};
use engine::graph::Fact;
use stash_adapter::reader::{Kind, LibraryRead};
use std::collections::BTreeMap;

/// Every fact, with the owner whose record produced it; panics on a fact produced twice.
fn owners_of(read: &LibraryRead) -> BTreeMap<Fact, String> {
    let i = iris();
    let ctx = Ctx::new(&read.tags, &i);
    let mut seen: BTreeMap<Fact, String> = BTreeMap::new();
    let mut put = |owner: Option<String>, facts: std::collections::BTreeSet<Fact>| {
        let owner = owner.expect("valid ids");
        for f in facts {
            if let Some(other) = seen.insert(f.clone(), owner.clone()) {
                panic!(
                    "{} is produced by both {other} and {owner}",
                    f.to_ntriples()
                );
            }
        }
    };
    for t in &read.tags {
        put(owner_iri(&i, Kind::Tag, &t.id), map_tag(&ctx, t).facts);
    }
    for p in &read.performers {
        put(
            owner_iri(&i, Kind::Performer, &p.id),
            map_performer(&ctx, p).facts,
        );
    }
    for m in read.scenes.iter().chain(&read.images) {
        put(
            owner_iri(&i, m.kind.into(), &m.id),
            map_media(&ctx, m).facts,
        );
    }
    put(
        Some("urn:semantic-stash:vocab:".into()),
        vocabulary_facts().into_iter().collect(),
    );
    seen
}

#[test]
fn no_fact_belongs_to_two_owners() {
    assert!(owners_of(&fixture_library()).len() > 300);
    assert!(owners_of(&synth::library(200, 80)).len() > 10_000);
}

/// FNV-1a, so the pin does not depend on the standard library's hasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The expected graph that `MAPPING_VERSION` was set for. Changing the mapping rules changes the
/// expected graph; bump `MAPPING_VERSION` with it, and update this pin.
const PINNED: (u32, u64) = (1, 0xf3f8_94fe_edb8_88be);

#[test]
fn the_mapping_version_moves_with_the_expected_graph() {
    let hash = fnv1a(&std::fs::read(fixture_path("expected.nt")).unwrap());
    assert_eq!(
        (MAPPING_VERSION, hash),
        PINNED,
        "expected.nt changed (hash {hash:#x}) without a matching MAPPING_VERSION bump: bump \
         MAPPING_VERSION in crates/engine/src/build/stamp.rs and update PINNED"
    );
}
