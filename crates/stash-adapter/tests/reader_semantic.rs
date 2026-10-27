//! One "semantic" rule for full reads and records read by id, and the update's reads (spec 008
//! T002; contracts/reader-additions.md).

mod common;

use common::{client, library, library_handler, serve};
use serde_json::json;
use stash_adapter::reader::field::Field;
use stash_adapter::reader::{
    performer_tag_set, LibraryRead, LibraryReader, MediaKind, MediaRecord, MemoryReader,
    SemanticPerformer, SemanticTag, TagFlag,
};
use std::collections::{BTreeMap, BTreeSet};

const T: &str = "2026-10-26T10:00:00-06:00";

fn tag(id: &str, flags: &[TagFlag], fields: &[&str]) -> SemanticTag {
    SemanticTag {
        id: id.into(),
        name: format!("tag {id}"),
        flags: flags.iter().copied().collect(),
        parents: vec![],
        fields: fields
            .iter()
            .map(|k| (k.to_string(), Field::Value(json!("1"))))
            .collect(),
        updated_at: T.into(),
    }
}

fn performer(id: &str, tags: &[&str]) -> SemanticPerformer {
    SemanticPerformer {
        id: id.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        archetype: false,
        archetype_parents: None,
        measurements: None,
        display: None,
        updated_at: T.into(),
    }
}

#[test]
fn the_semantic_rule_matches_the_full_reads_filter() {
    assert!(tag("1", &[TagFlag::Role], &[]).is_semantic());
    assert!(tag("2", &[], &["variantOfTagId"]).is_semantic());
    assert!(!tag("3", &[], &["categories"]).is_semantic());

    let tags = vec![
        tag("7", &[TagFlag::Trait], &[]),
        tag("8", &[TagFlag::Role], &[]),
    ];
    let set = performer_tag_set(&tags);
    assert_eq!(set, BTreeSet::from(["7".to_string()]));
    let mut archetype = performer("1", &[]);
    archetype.archetype = true;
    assert!(archetype.is_semantic(&set));
    let mut parents = performer("2", &[]);
    parents.archetype_parents = Some(Field::Value(json!(["9"])));
    assert!(parents.is_semantic(&set));
    let mut measured = performer("3", &[]);
    measured.measurements = Some(stash_adapter::reader::measurements::Measurements::parse(
        "34D-26-35",
    ));
    assert!(measured.is_semantic(&set));
    assert!(performer("4", &["7"]).is_semantic(&set));
    assert!(!performer("5", &["8", "99"]).is_semantic(&set));

    let mut media = MediaRecord {
        kind: MediaKind::Scene,
        id: "1".into(),
        performers: vec!["4".into()],
        tags: vec![],
        fields: BTreeMap::new(),
        updated_at: T.into(),
    };
    assert!(!media.is_semantic());
    media
        .fields
        .insert("timeline".into(), Field::Value(json!([])));
    assert!(media.is_semantic());
}

#[test]
fn semantic_tags_are_the_full_reads_tags() {
    let (port, _rx) = serve(library_handler(library("library.json")));
    let c = client(port);
    let full = c.full_read(&|| false, &mut |_| {}).unwrap();
    assert_eq!(LibraryReader::semantic_tags(&c).unwrap(), full.tags);
    let memory = MemoryReader::new(full.clone());
    assert_eq!(memory.semantic_tags().unwrap(), full.tags);
}

#[test]
fn performers_with_tags_lists_the_carriers() {
    let lib = library("library.json");
    let (port, _rx) = serve(library_handler(lib.clone()));
    let c = client(port);
    let mut expected: Vec<String> = lib["performers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["tags"].as_array().unwrap().iter().any(|t| t == "7"))
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    expected.sort_by_key(|s| s.parse::<u64>().unwrap());
    assert!(!expected.is_empty());
    assert_eq!(
        LibraryReader::performers_with_tags(&c, &["7".into()]).unwrap(),
        expected
    );
    assert!(LibraryReader::performers_with_tags(&c, &[])
        .unwrap()
        .is_empty());
}

#[test]
fn memory_full_read_filters_by_the_semantic_rule() {
    let read = LibraryRead {
        tags: vec![
            tag("1", &[TagFlag::Trait], &[]),
            tag("2", &[], &["categories"]),
        ],
        performers: vec![performer("4", &["1"]), performer("5", &["2"])],
        scenes: vec![MediaRecord {
            kind: MediaKind::Scene,
            id: "9".into(),
            performers: vec![],
            tags: vec![],
            fields: BTreeMap::new(),
            updated_at: T.into(),
        }],
        ..LibraryRead::default()
    };
    let m = MemoryReader::new(read);
    let full = m.full_read(&|| false, &mut |_| {}).unwrap();
    let ids = |v: Vec<String>| v;
    assert_eq!(ids(full.tags.iter().map(|t| t.id.clone()).collect()), ["1"]);
    assert_eq!(
        ids(full.performers.iter().map(|p| p.id.clone()).collect()),
        ["4"]
    );
    assert!(full.scenes.is_empty());
    // Records by id stay unfiltered: callers apply the rule.
    assert_eq!(m.performer_records(&["5".into()]).unwrap().found.len(), 1);
    assert_eq!(m.performers_with_tags(&["2".into()]).unwrap(), ["5"]);
}
