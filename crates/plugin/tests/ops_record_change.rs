//! The save hook's operation, `recordChange` (spec 004 T025, research R6; spec 009 T021,
//! research R8).

mod common;
use serde_json::{json, Value};

/// Every trigger of the hook entry (contracts/operations-1.4.md).
const TRIGGERS: &[&str] = &[
    "Tag.Create.Post",
    "Tag.Update.Post",
    "Tag.Destroy.Post",
    "Tag.Merge.Post",
    "Performer.Create.Post",
    "Performer.Update.Post",
    "Performer.Destroy.Post",
    "Scene.Create.Post",
    "Scene.Update.Post",
    "Scene.Destroy.Post",
    "Image.Create.Post",
    "Image.Update.Post",
    "Image.Destroy.Post",
];

fn hook_input(port: u16, dir: &std::path::Path, hook_type: &str, id: Value) -> Vec<u8> {
    json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": port,
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": "/tmp", "PluginDir": dir
        },
        "args": {
            "op": "recordChange",
            "hookContext": { "id": id, "type": hook_type, "inputFields": ["custom_fields"] }
        }
    })
    .to_string()
    .into_bytes()
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("se-hook-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn job_started(_: &str) -> String {
    r#"{"data":{"runPluginTask":"57"}}"#.to_string()
}

/// The relations path (spec 004) handles scene and image creates and updates only.
fn applies_relations(hook_type: &str) -> bool {
    matches!(
        hook_type,
        "Scene.Create.Post" | "Scene.Update.Post" | "Image.Create.Post" | "Image.Update.Post"
    )
}

#[test]
fn every_trigger_records_the_graph_marker_and_starts_an_update() {
    let (port, rx) = common::fake_stash_fn(2 * TRIGGERS.len(), job_started);
    for t in TRIGGERS {
        let dir = tmp(&format!("trigger-{t}"));
        let (env, log) = common::run(&hook_input(port, &dir, t, json!(5)));
        assert_eq!(env["ok"], true, "{t}: {env}");
        let relations = applies_relations(t);
        assert_eq!(
            env["result"],
            json!({
                "recorded": relations, "started": relations,
                "graph": { "recorded": true, "started": true }
            }),
            "{t}"
        );
        assert!(dir.join("graph.pending").exists(), "{t}");
        assert_eq!(dir.join("journal.jsonl").exists(), relations, "{t}");
        assert!(!log.contains("cookie-secret"));
        let mut bodies: Vec<String> = (0..if relations { 2 } else { 1 })
            .map(|_| rx.recv().unwrap())
            .collect();
        bodies.sort();
        assert!(
            bodies.last().unwrap().contains("Update semantic graph"),
            "{t}: {bodies:?}"
        );
        if relations {
            assert!(bodies[0].contains("Apply relations"), "{t}: {bodies:?}");
        }
    }
}

#[test]
fn the_first_save_of_a_burst_starts_each_job_once_and_every_save_is_journaled() {
    let dir = tmp("burst");
    let (port, rx) = common::fake_stash(vec![
        r#"{"data":{"runPluginTask":"57"}}"#,
        r#"{"data":{"runPluginTask":"58"}}"#,
    ]);
    let (env, _) = common::run(&hook_input(port, &dir, "Scene.Update.Post", json!(101)));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(
        env["result"],
        json!({ "recorded": true, "started": true, "graph": { "recorded": true, "started": true } })
    );
    let apply = rx.recv().unwrap();
    assert!(apply.contains("runPluginTask") && apply.contains("Apply relations"));
    let update = rx.recv().unwrap();
    assert!(update.contains("runPluginTask") && update.contains("Update semantic graph"));

    for (t, id) in [
        ("Scene.Update.Post", json!(102)),
        ("Image.Create.Post", json!("7")),
    ] {
        let (env, _) = common::run(&hook_input(common::dead_port(), &dir, t, id));
        assert_eq!(
            env["result"],
            json!({ "recorded": true, "started": false, "graph": { "recorded": true, "started": false } })
        );
    }
    let (env, _) = common::run(&hook_input(
        common::dead_port(),
        &dir,
        "Tag.Update.Post",
        json!(3),
    ));
    assert_eq!(
        env["result"],
        json!({ "recorded": false, "started": false, "graph": { "recorded": true, "started": false } })
    );
    let journal = std::fs::read_to_string(dir.join("journal.jsonl")).unwrap();
    let lines: Vec<Value> = journal
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        lines,
        vec![
            json!({ "type": "scene", "id": "101" }),
            json!({ "type": "scene", "id": "102" }),
            json!({ "type": "image", "id": "7" }),
        ]
    );
}

#[test]
fn tag_and_performer_saves_leave_the_relations_journal_alone() {
    for t in ["Tag.Update.Post", "Performer.Destroy.Post"] {
        let dir = tmp(&format!("no-journal-{t}"));
        let (env, _) = common::run(&hook_input(common::dead_port(), &dir, t, json!(5)));
        assert_eq!(env["result"]["recorded"], false, "{t}: {env}");
        assert_eq!(env["result"]["started"], false, "{t}: {env}");
        assert_eq!(env["result"]["graph"]["recorded"], true, "{t}: {env}");
        assert!(!dir.join("journal.jsonl").exists(), "{t}");
        assert!(!dir.join("apply.pending").exists(), "{t}");
    }
}

#[test]
fn a_failed_job_start_leaves_no_marker_so_the_next_save_retries() {
    let dir = tmp("retry");
    let (env, _) = common::run(&hook_input(
        common::dead_port(),
        &dir,
        "Scene.Update.Post",
        json!(5),
    ));
    assert_eq!(env["ok"], true, "a hook never fails the save: {env}");
    assert_eq!(env["result"]["started"], false);
    assert_eq!(
        env["result"]["graph"],
        json!({ "recorded": true, "started": false })
    );
    assert!(!dir.join("apply.pending").exists());
    assert!(!dir.join("graph.pending").exists());

    let (port, rx) = common::fake_stash(vec![r#"{"data":{"runPluginTask":"9"}}"#]);
    let (env, _) = common::run(&hook_input(port, &dir, "Tag.Create.Post", json!(6)));
    assert_eq!(env["result"]["graph"]["started"], true, "{env}");
    assert!(rx.recv().unwrap().contains("Update semantic graph"));
}

#[test]
fn an_unknown_hook_type_records_nothing() {
    for t in [
        "Gallery.Update.Post",
        "Scene.Merge.Post",
        "Tag.Frobnicate.Post",
        "Scene.Update.Pre",
        "nonsense",
    ] {
        let dir = tmp(&format!("unknown-{t}"));
        let (env, _) = common::run(&hook_input(common::dead_port(), &dir, t, json!(5)));
        assert_eq!(env["ok"], true, "{t}: {env}");
        assert_eq!(
            env["result"],
            json!({ "recorded": false, "started": false, "graph": { "recorded": false, "started": false } }),
            "{t}"
        );
        assert!(!dir.join("journal.jsonl").exists(), "{t}");
        assert!(!dir.join("graph.pending").exists(), "{t}");
    }
}
