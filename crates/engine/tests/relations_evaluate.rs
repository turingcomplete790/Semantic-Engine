//! Matches for one scene or image (spec 004 T016; research R3, R5).

mod common;
use engine::relations::evaluate::{matches, Match};
use engine::relations::Model;
use serde_json::json;
use stash_adapter::relations::TagRecord;

fn m(relation: &str, a: &str, b: &str, rule: &str) -> Match {
    Match {
        relation: relation.into(),
        a: a.into(),
        b: b.into(),
        rule: rule.into(),
    }
}

fn eval(model: &Model, scene: &str) -> Vec<Match> {
    let e = matches(&common::scene(scene), model, &common::labels()).unwrap();
    assert!(e.stable);
    e.matches
}

#[test]
fn example2_applies_size_difference_with_p1_smaller() {
    assert_eq!(
        eval(&common::example_model(), "101"),
        vec![m("45", "7", "8", "r1")]
    );
}

#[test]
fn direction_follows_whoever_meets_each_end() {
    let model = common::example_model();
    // Performer 13 is Short, 12 is Tall: 13 is at end a whatever the listing order.
    assert_eq!(eval(&model, "102"), vec![m("45", "13", "12", "r1")]);
    // Both are Short and Tall: one match per direction, never a performer with themselves.
    assert_eq!(
        eval(&model, "103"),
        vec![m("45", "14", "15", "r1"), m("45", "15", "14", "r1")]
    );
}

#[test]
fn every_paired_qualifying_pair_gets_the_relation() {
    assert_eq!(
        eval(&common::example_model(), "104"),
        vec![m("45", "7", "12", "r1"), m("45", "7", "8", "r1")]
    );
}

#[test]
fn the_same_action_without_a_pairing_does_not_link() {
    assert!(eval(&common::example_model(), "105").is_empty());
    assert!(eval(&common::example_model(), "112").is_empty());
}

#[test]
fn a_variant_counts_as_its_base_action() {
    assert_eq!(
        eval(&common::example_model(), "106"),
        vec![m("45", "7", "8", "r1")]
    );
}

#[test]
fn role_conditions_accept_narrower_roles_and_actions_filter_links() {
    let model = common::example_model();
    // Patient (narrower than Customer) and Doctor (narrower than Professional) paired on Hug.
    assert_eq!(eval(&model, "110"), vec![m("40", "16", "17", "fs1")]);
    // Extra Care's rule only accepts Kissing; the pair is on Hug, so only Full Service.
    assert_eq!(eval(&model, "111"), vec![m("40", "16", "18", "fs1")]);
}

#[test]
fn invalid_relations_are_never_applied() {
    let mut tags = common::example_tags();
    tags.push(TagRecord {
        id: "76".into(),
        name: "No Ends".into(),
        parents: vec![],
        custom_fields: json!({
            "isRelation": 1,
            "relationRules": "[{\"id\":\"x\",\"a\":{\"label\":\"50\"},\"b\":{\"label\":\"51\"}}]"
        })
        .as_object()
        .unwrap()
        .clone(),
    });
    let model = Model::from_tags(&tags);
    assert_eq!(eval(&model, "101"), vec![m("45", "7", "8", "r1")]);
}

#[test]
fn roles_added_automatically_feed_role_rules_until_stable() {
    // "Chain" needs Role Smaller at end a, which only Size Difference's auto-apply gives P1.
    let mut tags = common::example_tags();
    tags.push(TagRecord {
        id: "77".into(),
        name: "Chain".into(),
        parents: vec![],
        custom_fields: json!({
            "isRelation": 1,
            "relationEnds": "{\"a\":\"60\",\"b\":\"61\"}",
            "relationRules": "[{\"id\":\"c1\",\"a\":{\"role\":\"60\"},\"b\":{\"label\":\"51\"}}]"
        })
        .as_object()
        .unwrap()
        .clone(),
    });
    let model = Model::from_tags(&tags);
    let e = matches(&common::scene("101"), &model, &common::labels()).unwrap();
    assert!(e.stable);
    assert_eq!(e.passes, 2);
    assert_eq!(
        e.matches,
        vec![m("45", "7", "8", "r1"), m("77", "7", "8", "c1")]
    );
}
