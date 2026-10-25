//! T038 / FR-015 / SC-006 (spec 002) and spec 005 FR-025: the fixture tool captures through the
//! library reader, only reads, writes the expected files, and never writes a credential.

mod common;
use std::process::Command;

const KEY: &str = "secret-api-key-123";

#[test]
fn captures_read_only_without_credentials() {
    let (port, rx) = common::serve(common::library_handler(common::library("library.json")));
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
        // Every query is bounded: the version, a newest-first single row, a "key is not
        // null" filter, a tag or measurements criterion, or explicit ids.
        let v: serde_json::Value = serde_json::from_str(b).unwrap();
        let vars = v["variables"].as_object().cloned().unwrap_or_default();
        let bounded = q.contains("version")
            || q.contains("per_page: 1,")
            || vars.values().any(|x| {
                x.is_array()
                    || x["custom_fields"][0]["modifier"] == "NOT_NULL"
                    || x["tags"]["modifier"] == "INCLUDES"
                    || x["measurements"]["modifier"] == "NOT_NULL"
            });
        assert!(bounded, "unbounded query: {b}");
    }

    for f in [
        "library-read.json",
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
    assert_eq!(index["counts"]["tags"], 6);
    assert_eq!(index["counts"]["scenes"], 3);
    assert_eq!(index["counts"]["images"], 2);
    let read = stash_adapter::reader::MemoryReader::from_file(&out_dir.join("library-read.json"));
    assert!(read.is_ok(), "library-read.json loads into MemoryReader");
    assert_eq!(index["counts"]["performers"], 4);
    assert_eq!(index["empty"], serde_json::json!([]));
    assert!(index["requests"].as_u64().unwrap() >= 30);
    assert_eq!(index["start"]["scene"], "2026-10-24T10:27:55-06:00");
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
