//! Contract test for `explainRelation` (spec 004 T028; contracts/operations-1.1.md).

mod common;
use serde_json::{json, Value};

fn route(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap();
    let q = v["query"].as_str().unwrap();
    if q.contains("findTags") {
        common::tags_by_flag(body)
    } else if q.contains("findPerformers") {
        common::relation_fixture("performers.json")
    } else if q.contains("findScenes") {
        // Only scene 109 exists here; any other id reads as missing.
        if v["variables"]["ids"] == json!(["109"]) {
            let all: Value =
                serde_json::from_str(&common::relation_fixture("items_scenes.json")).unwrap();
            let s: Vec<&Value> = all["data"]["findScenes"]["scenes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["id"] == "109")
                .collect();
            json!({ "data": { "findScenes": { "scenes": s } } }).to_string()
        } else {
            r#"{"data":{"findScenes":{"scenes":[]}}}"#.to_string()
        }
    } else {
        panic!("unexpected {q}")
    }
}

fn call(args: Value, requests: usize) -> Value {
    let (port, _rx) = common::fake_stash_fn(requests, route);
    common::run(&common::input_args(port, "explainRelation", args)).0
}

#[test]
fn explains_an_automatic_instance() {
    let env = call(
        json!({ "media": "scene", "id": "109", "relationTagId": "45", "a": "7", "b": "8" }),
        common::DEFINITION_READS + 2,
    );
    assert_eq!(env["ok"], true, "{env}");
    let r = &env["result"];
    assert_eq!(r["applied"], "auto");
    assert_eq!(r["ruleId"], "r1");
    assert_eq!(r["link"]["pairingId"], "p109");
}

#[test]
fn a_missing_item_is_not_found() {
    let env = call(
        json!({ "media": "scene", "id": "999", "relationTagId": "45", "a": "7", "b": "8" }),
        common::DEFINITION_READS + 1,
    );
    assert_eq!(env["error"]["code"], "NOT_FOUND");
}

#[test]
fn every_argument_is_required() {
    for missing in ["media", "id", "relationTagId", "a", "b"] {
        let mut args =
            json!({ "media": "scene", "id": "109", "relationTagId": "45", "a": "7", "b": "8" });
        args.as_object_mut().unwrap().remove(missing);
        let (env, _) = common::run(&common::input_args(
            common::dead_port(),
            "explainRelation",
            args,
        ));
        assert_eq!(env["error"]["code"], "INVALID_INPUT", "without {missing}");
    }
    let (env, _) = common::run(&common::input_args(
        common::dead_port(),
        "explainRelation",
        json!({ "media": "gallery", "id": "1", "relationTagId": "45", "a": "7", "b": "8" }),
    ));
    assert_eq!(env["error"]["code"], "INVALID_INPUT");
}
