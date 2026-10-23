//! Contract test for `recommendations` (spec 004 T039; contracts/operations-1.1.md).

mod common;
use serde_json::{json, Value};

fn route(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap();
    let q = v["query"].as_str().unwrap();
    if q.contains("findTags") {
        common::tags_by_flag(body)
    } else if q.contains("findScene") {
        if v["variables"]["id"] == "101" {
            common::relation_fixture("native_tags_scene.json")
        } else {
            common::relation_fixture("native_tags_missing.json")
        }
    } else {
        panic!("unexpected {q}: recommendations must not write")
    }
}

fn call(args: Value) -> Value {
    let (port, _rx) = common::fake_stash_fn(common::DEFINITION_READS + 1, route);
    common::run(&common::input_args(port, "recommendations", args)).0
}

#[test]
fn recommends_by_category_with_the_suggesting_native_tag() {
    let env = call(json!({ "media": "scene", "id": "101" }));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(
        env["result"],
        json!({
            "actions": [{ "tag": "21", "from": "21" }, { "tag": "21", "from": "22" }],
            "roles": [], "pluggables": [], "miseEnScene": [],
            "relations": [{ "tag": "45", "from": "45" }],
        })
    );
}

#[test]
fn a_missing_item_is_not_found() {
    assert_eq!(
        call(json!({ "media": "scene", "id": "404" }))["error"]["code"],
        "NOT_FOUND"
    );
}

#[test]
fn media_and_id_are_required() {
    for args in [
        json!({ "id": "101" }),
        json!({ "media": "scene" }),
        json!({ "media": "group", "id": "1" }),
    ] {
        let (env, _) = common::run(&common::input_args(
            common::dead_port(),
            "recommendations",
            args.clone(),
        ));
        assert_eq!(env["error"]["code"], "INVALID_INPUT", "{args}");
    }
}
