//! After any sequence of changes, an update gives exactly the graph a full rebuild gives (spec 008
//! T011; FR-007, SC-001).

mod common;

use common::build::SERVER;
use common::update::{all_facts, differences, new_store, rebuilt_facts, Library};
use engine::build::update::{update, Mode};
use engine::build::{run, BuildOptions};
use serde_json::json;
use stash_adapter::reader::field::Field;
use stash_adapter::reader::{
    Kind, MediaKind, MediaRecord, SemanticPerformer, SemanticTag, TagFlag,
};
use std::collections::BTreeMap;

type Step = fn(&mut Library);

fn scene(id: &str, performers: &[&str], roles: serde_json::Value) -> MediaRecord {
    MediaRecord {
        kind: MediaKind::Scene,
        id: id.into(),
        performers: performers.iter().map(|s| s.to_string()).collect(),
        tags: vec![],
        fields: BTreeMap::from([("roleTagAssignments".to_string(), Field::Value(roles))]),
        updated_at: String::new(),
    }
}

fn performer(id: &str, tags: &[&str]) -> SemanticPerformer {
    SemanticPerformer {
        id: id.into(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        archetype: false,
        archetype_parents: None,
        measurements: None,
        display: None,
        updated_at: String::new(),
    }
}

fn tag(id: &str, name: &str, flags: &[TagFlag]) -> SemanticTag {
    SemanticTag {
        id: id.into(),
        name: name.into(),
        flags: flags.iter().copied().collect(),
        parents: vec![],
        fields: BTreeMap::new(),
        updated_at: String::new(),
    }
}

/// The sequences of research R8: each is a list of steps, with an update after every step.
fn sequences() -> Vec<(&'static str, Vec<Step>)> {
    vec![
        (
            "US1-1 a scene's roles change",
            vec![|l| {
                l.edit_media_field(
                    MediaKind::Scene,
                    "100",
                    "roleTagAssignments",
                    json!({"7": ["2"], "8": ["3", "1"]}),
                )
            }],
        ),
        (
            "US1-2 a scene is deleted",
            vec![|l| l.delete(Kind::Scene, "100")],
        ),
        (
            "US1-3 a scene's semantic fields are cleared",
            vec![|l| l.clear_media_fields(MediaKind::Scene, "101")],
        ),
        (
            "US1-4 a new performer with a Trait",
            vec![|l| l.add_performer(performer("40", &["7"]))],
        ),
        (
            "US1-5 a tag a performer carries becomes a Trait",
            vec![|l| l.add_tag(tag("500", "Freckles", &[TagFlag::Trait]))],
        ),
        (
            "a parent Role loses its flag",
            vec![|l| l.set_tag_flag("1", TagFlag::Role, false)],
        ),
        (
            "a relation's Label condition loses its flag",
            vec![|l| l.set_tag_flag("8", TagFlag::Label, false)],
        ),
        ("US1-6 two tags merge", vec![|l| l.merge_tags("13", "8")]),
        (
            "an Archetype performer is deleted",
            vec![|l| l.delete(Kind::Performer, "20")],
        ),
        (
            "a performer stops being semantic",
            vec![|l| l.set_archetype("22", false)],
        ),
        (
            "a performer loses its semantic tags but keeps measurements",
            vec![|l| l.set_performer_tags("8", &[])],
        ),
        (
            "a new scene",
            vec![|l| l.add_media(scene("400", &["7"], json!({"7": ["2"]})))],
        ),
        (
            "a scene added, then deleted",
            vec![
                |l| l.add_media(scene("401", &["8"], json!({"8": ["3"]}))),
                |l| l.delete(Kind::Scene, "401"),
            ],
        ),
        (
            "a tag's categories change",
            vec![|l| l.edit_tag_field("7", "categories", json!(["Look"]))],
        ),
        (
            "a relation's rules change",
            vec![|l| {
                l.edit_tag_field(
                    "12",
                    "relationRules",
                    json!([{"id": "rr1", "a": {"label": "8"}, "b": {"role": "3"}}]),
                )
            }],
        ),
        (
            "an image loses its timeline",
            vec![|l| l.remove_media_field(MediaKind::Image, "200", "timeline")],
        ),
        (
            "a scene's timeline loses a Stage",
            vec![|l| {
                l.edit_media_field(MediaKind::Scene, "100", "timeline", json!([
                {"id": "s1", "type": "stage", "miseEnScenePicks": [{"tagId": "9", "variation": "sunset"}]},
                {"id": "e1", "ref": {"kind": "action-instance", "performerId": "7", "actionTagId": "4"}, "seconds": 30}
            ]))
            }],
        ),
        (
            "three changes, then one update",
            vec![|l| {
                l.edit_media_field(
                    MediaKind::Scene,
                    "101",
                    "roleTagAssignments",
                    json!({"7": ["2"]}),
                );
                l.edit_tag_field("4", "variations", json!(["peck"]));
                l.set_performer_tags("7", &["7", "11"]);
            }],
        ),
        (
            "a common variant loses its variant link",
            vec![|l| l.remove_tag_field("5", "variantOfTagId")],
        ),
        (
            "a referenced tag is deleted",
            vec![|l| l.delete(Kind::Tag, "6")],
        ),
        (
            "a scene's performers change",
            vec![|l| l.set_media_performers(MediaKind::Scene, "101", &["7"])],
        ),
        (
            "a performer with nothing semantic is added",
            vec![|l| l.add_performer(performer("41", &["999"]))],
        ),
        (
            "a Mise-en-scène tag loses its flag",
            vec![|l| l.set_tag_flag("9", TagFlag::MiseEnScene, false)],
        ),
        (
            "a tag gains a parent Role",
            vec![|l| l.set_tag_parents("3", &["1"])],
        ),
        (
            "a tag gains a flag and a scene changes",
            vec![|l| l.set_tag_flag("6", TagFlag::Variation, true), |l| {
                l.edit_media_field(
                    MediaKind::Scene,
                    "300",
                    "roleTagAssignments",
                    json!({"7": ["2"]}),
                )
            }],
        ),
    ]
}

#[test]
fn every_update_equals_a_full_rebuild() {
    let seqs = sequences();
    assert!(seqs.len() >= 20);
    for (n, (name, steps)) in seqs.into_iter().enumerate() {
        let mut lib = Library::fixture();
        let store = new_store(&format!("eq{n}"));
        run(
            &lib.reader(),
            &store,
            SERVER,
            &BuildOptions::default(),
            &mut |_| {},
            &mut |_, _| {},
        )
        .unwrap();
        for (i, step) in steps.iter().enumerate() {
            step(&mut lib);
            let report = update(
                &lib.reader(),
                &store,
                SERVER,
                &BuildOptions::default(),
                &mut |_| {},
                &mut |_, _| {},
            )
            .unwrap_or_else(|e| panic!("{name}, step {i}: {e}"));
            assert_ne!(
                report.mode,
                Mode::Rebuild,
                "{name}, step {i}: {:?}",
                report.rebuild_reason
            );
            let updated = all_facts(&store);
            let rebuilt = rebuilt_facts(&lib, &format!("eq{n}-{i}"));
            assert!(
                updated == rebuilt,
                "{name}, step {i}: the update differs from a full rebuild\n{}",
                differences(&updated, &rebuilt)
            );
        }
    }
}
