#![allow(dead_code)]
//! Loads the offline relation fixtures (`tests/fixtures/relations/`, spec 004 T002).

use stash_adapter::relations::TagRecord;
use std::path::Path;

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/relations")
            .join(name),
    )
    .unwrap()
}

pub fn tags(files: &[&str]) -> Vec<TagRecord> {
    files
        .iter()
        .flat_map(|f| serde_json::from_str::<Vec<TagRecord>>(&fixture(f)).unwrap())
        .collect()
}

/// Both examples and the invalid definitions, as tests load them together.
pub fn all_tags() -> Vec<TagRecord> {
    tags(&["example1.json", "example2.json", "invalid.json"])
}

pub fn example_tags() -> Vec<TagRecord> {
    tags(&["example1.json", "example2.json"])
}

use stash_adapter::relations::{ItemRecord, PerformerRecord};
use std::collections::{BTreeMap, BTreeSet};

pub fn scenes() -> Vec<ItemRecord> {
    serde_json::from_str(&fixture("scenes.json")).unwrap()
}

pub fn scene(id: &str) -> ItemRecord {
    scenes().into_iter().find(|s| s.id == id).unwrap()
}

/// Performer id to tag ids, from `performers.json`.
pub fn labels() -> BTreeMap<String, BTreeSet<String>> {
    let ps: Vec<PerformerRecord> = serde_json::from_str(&fixture("performers.json")).unwrap();
    ps.into_iter()
        .map(|p| (p.id, p.tags.into_iter().collect()))
        .collect()
}

pub fn example_model() -> engine::relations::Model {
    engine::relations::Model::from_tags(&example_tags())
}

/// Applies a partial `custom_fields` patch to an item, as Stash would.
pub fn apply_patch(
    item: &ItemRecord,
    partial: &serde_json::Map<String, serde_json::Value>,
) -> ItemRecord {
    let mut out = item.clone();
    for (k, v) in partial {
        out.custom_fields.insert(k.clone(), v.clone());
    }
    out
}

pub mod build;
pub mod graph;
pub mod query;
pub mod update;
