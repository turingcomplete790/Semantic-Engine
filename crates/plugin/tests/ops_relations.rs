//! Contract test for the `relations` operation (spec 004 T012; contracts/operations-1.1.md).

mod common;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn call(args: Value) -> Value {
    let (port, _rx) = common::fake_stash_fn(common::DEFINITION_READS, common::tags_by_flag);
    common::run(&common::input_args(port, "relations", args)).0
}

fn ids(list: &Value) -> BTreeSet<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect()
}

fn set(xs: &[&str]) -> BTreeSet<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

fn find<'a>(list: &'a Value, id: &str) -> &'a Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == id)
        .unwrap_or_else(|| panic!("{id} missing from {list}"))
}

#[test]
fn reports_roles_relations_and_every_problem() {
    let env = call(json!({}));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["op"], "relations");
    assert_eq!(env["contractVersion"], engine::CONTRACT_VERSION);
    let r = &env["result"];

    let patient = find(&r["roles"], "31");
    assert_eq!(patient["name"], "Patient");
    assert_eq!(patient["broader"], json!(["30"]));
    assert_eq!(find(&r["roles"], "32")["narrower"], json!(["33", "34"]));

    let extra = find(&r["relations"], "41");
    assert_eq!(extra["ends"], json!({ "a": "31", "b": "33" }));
    assert_eq!(extra["broader"], json!(["40"]));
    assert_eq!(extra["valid"], true);
    assert_eq!(
        extra["rules"],
        json!([{ "id": "ec1", "a": { "role": "31" }, "b": { "role": "33" }, "actions": ["21"] }])
    );
    assert_eq!(find(&r["relations"], "72")["ends"], Value::Null);
    assert_eq!(find(&r["relations"], "74")["valid"], false);

    let kinds: BTreeSet<String> = r["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        kinds,
        set(&["loop", "missingEnd", "endNotRole", "invalidEnds", "badRule"])
    );
}

#[test]
fn a_tag_narrows_the_result_to_its_broader_and_narrower_terms() {
    let r = &call(json!({ "tag": "32" }))["result"];
    assert_eq!(ids(&r["roles"]), set(&["32", "33", "34"]));
    assert!(r["relations"].as_array().unwrap().is_empty());

    let r = &call(json!({ "tag": "31" }))["result"];
    assert_eq!(ids(&r["roles"]), set(&["30", "31"]));

    let r = &call(json!({ "tag": "40", "recursive": true }))["result"];
    assert_eq!(ids(&r["relations"]), set(&["40", "41", "74"]));
}

#[test]
fn narrower_lists_direct_children_by_default() {
    // Direct only by default: Full Service's direct narrower relations.
    let r = &call(json!({ "tag": "40" }))["result"];
    assert_eq!(find(&r["relations"], "40")["narrower"], json!(["41", "74"]));
}

#[test]
fn an_unknown_or_non_semantic_tag_is_invalid_input() {
    for tag in ["999", "90"] {
        let env = call(json!({ "tag": tag }));
        assert_eq!(env["ok"], false);
        assert_eq!(env["error"]["code"], "INVALID_INPUT", "{tag}");
    }
}

#[test]
fn unreachable_stash_is_reported() {
    let (env, _) = common::run(&common::input_args(
        common::dead_port(),
        "relations",
        json!({}),
    ));
    assert_eq!(env["error"]["code"], "STASH_UNREACHABLE");
}
