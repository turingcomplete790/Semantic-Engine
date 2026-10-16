mod common;

#[test]
fn ping_answers_without_calling_stash() {
    let (env, _) = common::run(&common::input(common::dead_port(), "ping"));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["op"], "ping");
    assert_eq!(env["result"], serde_json::json!({"pong": true}));
}

#[test]
fn unknown_operation_lists_known_names() {
    let (env, _) = common::run(&common::input(common::dead_port(), "nope"));
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "UNKNOWN_OPERATION");
    let known: Vec<String> =
        serde_json::from_value(env["error"]["details"]["known"].clone()).unwrap();
    for name in ["ping", "status"] {
        assert!(known.contains(&name.to_string()), "{known:?}");
    }
}

#[test]
fn missing_op_is_unknown_operation() {
    let mut v: serde_json::Value = serde_json::from_slice(&common::input(1, "x")).unwrap();
    v["args"] = serde_json::json!({});
    let (env, _) = common::run(v.to_string().as_bytes());
    assert_eq!(env["error"]["code"], "UNKNOWN_OPERATION");
}
