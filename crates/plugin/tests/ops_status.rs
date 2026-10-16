mod common;

const FIELDS: [&str; 7] = [
    "engineVersion",
    "contractVersion",
    "vocabularyVersion",
    "oxigraphVersion",
    "rocksdbVersion",
    "stashMinimumVersion",
    "build",
];

#[test]
fn status_reports_versions_and_the_server_version() {
    let (port, rx) = common::fake_stash(vec![r#"{"data":{"version":{"version":"v0.31.1"}}}"#]);
    let (env, log) = common::run(&common::input(port, "status"));
    assert_eq!(env["ok"], true, "{env}");
    let r = &env["result"];
    for f in FIELDS {
        assert!(r.get(f).is_some(), "missing {f}: {r}");
    }
    assert_eq!(r["stashVersion"], "v0.31.1");
    assert!(r["serverId"].is_null());
    assert_eq!(r["rocksdb"]["available"], true);
    assert_eq!(r["rocksdb"]["version"], r["rocksdbVersion"]);
    assert!(rx.recv().unwrap().contains("version"));
    assert!(!log.contains("cookie-secret"));
}

#[test]
fn unreachable_stash_still_returns_local_versions() {
    let (env, _) = common::run(&common::input(common::dead_port(), "status"));
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "STASH_UNREACHABLE");
    for f in FIELDS {
        assert!(env["error"]["details"].get(f).is_some(), "missing {f}");
    }
}

#[test]
fn older_stash_is_too_old() {
    let (port, _rx) = common::fake_stash(vec![r#"{"data":{"version":{"version":"v0.30.0"}}}"#]);
    let (env, _) = common::run(&common::input(port, "status"));
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "STASH_TOO_OLD");
    assert_eq!(env["error"]["details"]["stashVersion"], "v0.30.0");
}
