mod common;
use common::VALID_ID;

fn settings(v: &str) -> String {
    format!(r#"{{"data":{{"configuration":{{"plugins":{{"semantic-engine":{v}}}}}}}}}"#)
}

#[test]
fn get_server_id_returns_the_stored_value() {
    let (port, _rx) =
        common::fake_stash_fn(1, |_| settings(&format!(r#"{{"serverId":"{VALID_ID}"}}"#)));
    let (env, _) = common::run(&common::input(port, "getServerId"));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["result"]["serverId"], VALID_ID);
}

#[test]
fn get_server_id_reports_missing_and_invalid() {
    let (port, _rx) = common::fake_stash_fn(1, |_| settings("{}"));
    let (env, _) = common::run(&common::input(port, "getServerId"));
    assert_eq!(env["error"]["code"], "SERVER_ID_NOT_GENERATED");
    let (port, _rx) = common::fake_stash_fn(1, |_| settings(r#"{"serverId":"bad"}"#));
    let (env, _) = common::run(&common::input(port, "getServerId"));
    assert_eq!(env["error"]["code"], "SERVER_ID_INVALID");
}

#[test]
fn get_server_id_never_writes() {
    let (port, rx) = common::fake_stash_fn(1, |_| settings("{}"));
    common::run(&common::input(port, "getServerId"));
    assert!(!rx.recv().unwrap().contains("configurePlugin"));
}

#[test]
fn task_generates_then_reports_existing() {
    let (port, rx) = common::fake_stash_fn(2, |body| {
        common::echo_configure(body).unwrap_or_else(|| settings(r#"{"other":"keep"}"#))
    });
    let (env, log) = common::run(&common::input(port, "generateServerId"));
    assert_eq!(env["ok"], true, "{env}");
    let id = env["result"]["serverId"].as_str().unwrap().to_string();
    assert_eq!(env["result"]["outcome"], "generated");
    assert!(log.contains(&format!("serverId={id} (generated)")), "{log}");
    let _read = rx.recv().unwrap();
    let write: serde_json::Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
    assert_eq!(write["variables"]["input"]["other"], "keep");

    let (port, _rx) =
        common::fake_stash_fn(1, |_| settings(&format!(r#"{{"serverId":"{VALID_ID}"}}"#)));
    let (env, log) = common::run(&common::input(port, "generateServerId"));
    assert_eq!(env["result"]["outcome"], "existing");
    assert!(
        log.contains(&format!("serverId={VALID_ID} (existing)")),
        "{log}"
    );
}

#[test]
fn status_includes_the_server_id() {
    let (port, _rx) = common::fake_stash_fn(2, |body| {
        if body.contains("configuration") {
            settings(&format!(r#"{{"serverId":"{VALID_ID}"}}"#))
        } else {
            r#"{"data":{"version":{"version":"v0.31.1"}}}"#.to_string()
        }
    });
    let (env, _) = common::run(&common::input(port, "status"));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["result"]["serverId"], VALID_ID);
}
