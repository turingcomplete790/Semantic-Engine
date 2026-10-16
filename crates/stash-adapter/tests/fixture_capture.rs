//! T038 / FR-015 / SC-006: the fixture tool only reads, writes the expected files, reports empty
//! sections, and never writes a credential.

mod common;
use std::process::Command;

const KEY: &str = "secret-api-key-123";

fn reply(body: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(body).unwrap();
    let q = v["query"].as_str().unwrap();
    if q.contains("findTags") {
        r#"{"data":{"findTags":{"count":1,"tags":[{"id":"19","name":"Role Dom","parents":[],"custom_fields":{"isRole":1}}]}}}"#.into()
    } else if q.contains("findScenes") {
        r#"{"data":{"findScenes":{"count":1,"scenes":[{"id":"2","title":"S","performers":[{"id":"7"}],"custom_fields":{"timeline":"[]"}}]}}}"#.into()
    } else if q.contains("findImages") {
        r#"{"data":{"findImages":{"count":0,"images":[]}}}"#.into()
    } else if q.contains("findPerformers") {
        r#"{"data":{"findPerformers":{"count":1,"performers":[{"id":"7","name":"P","tags":[],"custom_fields":{}}]}}}"#.into()
    } else {
        r#"{"errors":[{"message":"unexpected query"}]}"#.into()
    }
}

#[test]
fn captures_read_only_without_credentials() {
    let (port, rx) = common::fake_stash(200, reply);
    let out_dir = std::env::temp_dir().join(format!("se-fixtures-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    let out = Command::new(env!("CARGO_BIN_EXE_fixture-capture"))
        .args(["--out", out_dir.to_str().unwrap()])
        .env("STASH_URL", format!("http://127.0.0.1:{port}"))
        .env("STASH_API_KEY", KEY)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(!stdout.contains(KEY), "the API key must never be printed");

    // Only queries were sent.
    let bodies: Vec<String> = rx.try_iter().collect();
    assert!(!bodies.is_empty());
    for b in &bodies {
        let q = serde_json::from_str::<serde_json::Value>(b).unwrap()["query"]
            .as_str()
            .unwrap()
            .trim_start()
            .to_string();
        assert!(q.starts_with("query"), "non-query operation sent: {q}");
        assert!(!q.contains("mutation"), "{q}");
        // Every entity query is bounded by a custom_fields filter or explicit ids.
        assert!(q.contains("custom_fields"), "{q}");
    }

    for f in [
        "tags.json",
        "performers.json",
        "scenes.json",
        "images.json",
        "index.json",
    ] {
        let path = out_dir.join(f);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing {f}"));
        assert!(!text.contains(KEY), "{f} contains the API key");
        assert!(
            !text.to_ascii_lowercase().contains("apikey"),
            "{f} mentions an API key header"
        );
        serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(|e| panic!("{f}: {e}"));
    }
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("index.json")).unwrap())
            .unwrap();
    assert_eq!(index["counts"]["tags"], 1);
    assert_eq!(index["counts"]["scenes"], 1);
    assert_eq!(index["counts"]["performers"], 1);
    assert_eq!(index["counts"]["images"], 0);
    assert!(index["empty"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e == "images"));
    assert!(index["queries"].as_array().unwrap().len() >= 4);
    std::fs::remove_dir_all(&out_dir).unwrap();
}

#[test]
fn missing_configuration_fails_and_writes_nothing() {
    let out_dir = std::env::temp_dir().join(format!("se-fixtures-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    let out = Command::new(env!("CARGO_BIN_EXE_fixture-capture"))
        .args(["--out", out_dir.to_str().unwrap()])
        .env_remove("STASH_URL")
        .env_remove("STASH_API_KEY")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!out_dir.exists());
}

#[test]
fn unreachable_server_fails_and_writes_nothing() {
    let out_dir = std::env::temp_dir().join(format!("se-fixtures-down-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    let out = Command::new(env!("CARGO_BIN_EXE_fixture-capture"))
        .args(["--out", out_dir.to_str().unwrap()])
        .env("STASH_URL", "http://127.0.0.1:1")
        .env("STASH_API_KEY", KEY)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains(KEY));
    assert!(!out_dir.exists());
}
