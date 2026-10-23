//! The save hook's operation, `recordChange` (spec 004 T025; research R6).

mod common;
use serde_json::{json, Value};

fn hook_input(port: u16, dir: &std::path::Path, kind: &str, id: Value) -> Vec<u8> {
    json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": port,
            "SessionCookie": {"Name": "session", "Value": "cookie-secret"},
            "Dir": "/tmp", "PluginDir": dir
        },
        "args": {
            "op": "recordChange",
            "hookContext": { "id": id, "type": format!("{kind}.Update.Post"), "inputFields": ["custom_fields"] }
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

#[test]
fn the_first_save_of_a_burst_starts_one_job_and_every_save_is_journaled() {
    let dir = tmp("burst");
    let (port, rx) = common::fake_stash(vec![r#"{"data":{"runPluginTask":"57"}}"#]);
    let (env, _) = common::run(&hook_input(port, &dir, "Scene", json!(101)));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["result"], json!({ "recorded": true, "started": true }));
    let body = rx.recv().unwrap();
    assert!(body.contains("runPluginTask") && body.contains("Apply relations"));

    for (kind, id) in [("Scene", json!(102)), ("Image", json!("7"))] {
        let (env, _) = common::run(&hook_input(common::dead_port(), &dir, kind, id));
        assert_eq!(env["result"], json!({ "recorded": true, "started": false }));
    }
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
fn a_failed_job_start_leaves_no_marker_so_the_next_save_retries() {
    let dir = tmp("retry");
    let (env, _) = common::run(&hook_input(common::dead_port(), &dir, "Scene", json!(5)));
    assert_eq!(env["ok"], true, "a hook never fails the save: {env}");
    assert_eq!(env["result"]["started"], false);
    assert!(!dir.join("apply.pending").exists());
}

#[test]
fn other_entities_are_ignored() {
    let dir = tmp("other");
    let (env, _) = common::run(&hook_input(common::dead_port(), &dir, "Tag", json!(5)));
    assert_eq!(
        env["result"],
        json!({ "recorded": false, "started": false })
    );
    assert!(!dir.join("journal.jsonl").exists());
}
