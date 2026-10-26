//! A synthetic, fully tagged library in the reader's form, for the kill test and `build-bench`
//! (research R11). The shape follows the test instance's records: every scene has role, action
//! and pluggable assignments for two performers, an action and a role pairing, and a timeline
//! with a Stage and three slots. Every image has one performer with a role and a slot.

use serde_json::{json, Value};
use stash_adapter::reader::field::Field;
use stash_adapter::reader::measurements::Measurements;
use stash_adapter::reader::{
    Kind, LibraryRead, MediaKind, MediaRecord, SemanticPerformer, SemanticTag, TagFlag,
};
use std::collections::BTreeMap;

/// Tags per flag.
pub const TAGS_PER_FLAG: usize = 6;
const TIME: &str = "2026-10-26T10:00:00-06:00";

fn tag_id(flag: usize, n: usize) -> String {
    (1 + flag * TAGS_PER_FLAG + n).to_string()
}

fn fields(pairs: Vec<(&str, Value)>) -> BTreeMap<String, Field> {
    pairs
        .into_iter()
        .map(|(k, v)| (k.to_string(), Field::Value(v)))
        .collect()
}

/// `scenes` scenes and `images` images, with one performer per 50 scenes (at least 6).
pub fn library(scenes: usize, images: usize) -> LibraryRead {
    let flag_index = |f: TagFlag| TagFlag::ALL.iter().position(|x| *x == f).unwrap();
    let t = |f: TagFlag, n: usize| tag_id(flag_index(f), n % TAGS_PER_FLAG);
    let mut tags = Vec::new();
    for (fi, f) in TagFlag::ALL.iter().enumerate() {
        for n in 0..TAGS_PER_FLAG {
            let mut extra = Vec::new();
            match f {
                TagFlag::Action | TagFlag::Trait | TagFlag::MiseEnScene => {
                    extra.push(("variations", json!(["one", "two"])))
                }
                TagFlag::Label => extra.push((
                    "labelFormula",
                    json!({"threshold": 100, "traitRules": [
                        {"id": "r1", "tagId": tag_id(flag_index(TagFlag::Trait), n), "weight": 100}
                    ]}),
                )),
                _ => {}
            }
            tags.push(SemanticTag {
                id: tag_id(fi, n),
                name: format!("{} {n}", f.key()),
                flags: [*f].into(),
                parents: Vec::new(),
                fields: fields(extra),
                updated_at: TIME.into(),
            });
        }
    }
    let people = (scenes / 50).max(6);
    let performers = (0..people)
        .map(|i| SemanticPerformer {
            id: (1 + i).to_string(),
            tags: vec![t(TagFlag::Trait, i), t(TagFlag::Label, i)],
            archetype: false,
            archetype_parents: None,
            measurements: Some(Measurements::parse("34D-26-35")),
            display: None,
            updated_at: TIME.into(),
        })
        .collect();
    let perf = |i: usize| (1 + i % people).to_string();
    let scene_records = (0..scenes)
        .map(|i| {
            let (a, b) = (perf(2 * i), perf(2 * i + 1));
            let (role_a, role_b) = (t(TagFlag::Role, i), t(TagFlag::Role, i + 1));
            let action = t(TagFlag::Action, i);
            MediaRecord {
                kind: MediaKind::Scene,
                id: (1 + i).to_string(),
                performers: vec![a.clone(), b.clone()],
                tags: Vec::new(),
                fields: fields(vec![
                    (
                        "roleTagAssignments",
                        json!({ &a: [&role_a], &b: [&role_b] }),
                    ),
                    (
                        "actionAssignments",
                        json!({ &a: [{"actionTagId": &action, "variations": ["one"]}],
                                &b: [{"actionTagId": &action}] }),
                    ),
                    (
                        "pluggableAssignments",
                        json!({ &a: [t(TagFlag::Pluggable, i)] }),
                    ),
                    (
                        "actionPairings",
                        json!([{"a": {"performer": &a, "actionTagId": &action},
                                "b": {"performer": &b, "actionTagId": &action},
                                "direction": "give", "pairingId": "p1"}]),
                    ),
                    (
                        "roleTagPairings",
                        json!([{"a": {"performer": &a, "roleTagId": &role_a},
                                "b": {"performer": &b, "roleTagId": &role_b}, "pairingId": "p2"}]),
                    ),
                    (
                        "timeline",
                        json!([
                            {"id": "s1", "type": "stage", "miseEnScenePicks":
                                [{"tagId": t(TagFlag::MiseEnScene, i), "variation": "one"}]},
                            {"id": "e1", "ref": {"kind": "action-instance", "performerId": &a,
                                "actionTagId": &action}, "seconds": 30},
                            {"id": "e2", "ref": {"kind": "role-instance", "performerId": &b,
                                "roleTagId": &role_b}, "seconds": 60},
                            {"id": "e3", "ref": {"kind": "action-pairing", "pairingId": "p1"},
                                "seconds": 90}
                        ]),
                    ),
                ]),
                updated_at: TIME.into(),
            }
        })
        .collect();
    let image_records = (0..images)
        .map(|i| {
            let p = perf(i);
            let role = t(TagFlag::Role, i);
            MediaRecord {
                kind: MediaKind::Image,
                id: (1 + i).to_string(),
                performers: vec![p.clone()],
                tags: Vec::new(),
                fields: fields(vec![
                    ("roleTagAssignments", json!({ &p: [&role] })),
                    (
                        "timeline",
                        json!([{"id": "i1", "ref": {"kind": "role-instance", "performerId": &p,
                            "roleTagId": &role}}]),
                    ),
                ]),
                updated_at: TIME.into(),
            }
        })
        .collect();
    LibraryRead {
        start: Kind::ALL.iter().map(|k| (*k, TIME.to_string())).collect(),
        tags,
        performers,
        scenes: scene_records,
        images: image_records,
        missing: BTreeMap::new(),
        requests: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::map_library;
    use semantic_vocabulary::iri::ItemIris;

    #[test]
    fn a_synthetic_library_maps_without_notes() {
        let iris = ItemIris::new("00000000-0000-4000-8000-000000000001").unwrap();
        let m = map_library(&library(10, 5), &iris);
        assert!(m.notes.is_empty(), "{:?}", m.notes.as_slice());
        assert!(m.facts.len() > 10 * 50, "{}", m.facts.len());
    }
}
