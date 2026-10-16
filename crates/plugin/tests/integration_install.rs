//! Opt-in end-to-end test against a real Stash (US1 independent test, SC-005 baseline).
//!
//! Runs only when these are set:
//!   STASH_TEST_URL           e.g. http://localhost:9998
//!   STASH_TEST_API_KEY       API key of that server (never printed)
//!   STASH_TEST_PLUGINS_DIR   that server's plugins folder
//! Optional: STASH_TEST_PACKAGE (a package built by scripts/package.sh; built here if unset).
//!
//! It installs the plugin into the plugins folder and reloads plugins on that server.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn gql(url: &str, key: &str, query: &str) -> serde_json::Value {
    stash_adapter::install_tls_provider();
    let resp = reqwest::blocking::Client::new()
        .post(format!("{url}/graphql"))
        .header("ApiKey", key)
        .json(&serde_json::json!({ "query": query }))
        .send()
        .expect("Stash reachable");
    let v: serde_json::Value = resp.json().expect("JSON response");
    assert!(v.get("errors").is_none(), "GraphQL errors: {v}");
    v
}

#[test]
fn installs_and_answers_status() {
    let (Some(url), Some(key), Some(plugins)) = (
        env("STASH_TEST_URL"),
        env("STASH_TEST_API_KEY"),
        env("STASH_TEST_PLUGINS_DIR"),
    ) else {
        eprintln!(
            "notice: STASH_TEST_URL, STASH_TEST_API_KEY, STASH_TEST_PLUGINS_DIR not set; skipped"
        );
        return;
    };
    let package = env("STASH_TEST_PACKAGE").unwrap_or_else(|| {
        let out = Command::new("sh")
            .arg("scripts/package.sh")
            .current_dir(repo())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let path = String::from_utf8_lossy(&out.stdout)
            .lines()
            .last()
            .unwrap()
            .trim()
            .to_string();
        repo().join(path).to_string_lossy().to_string()
    });

    let out = Command::new("sh")
        .arg(repo().join("scripts/install.sh"))
        .args(["--from-file", &package, "--plugins-dir", &plugins, "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    gql(&url, &key, "mutation { reloadPlugins }");
    let listed = gql(&url, &key, "{ plugins { id name tasks { name } } }");
    let plugin = listed["data"]["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "semantic-engine")
        .cloned()
        .expect("Stash lists the semantic-engine plugin");
    assert_eq!(plugin["tasks"][0]["name"], "Generate server identifier");

    let start = Instant::now();
    let r = gql(
        &url,
        &key,
        r#"mutation { runPluginOperation(plugin_id: "semantic-engine", args: {op: "status"}) }"#,
    );
    let elapsed = start.elapsed();
    let env = &r["data"]["runPluginOperation"];
    assert_eq!(env["ok"], true, "{env}");
    assert!(env["result"]["stashVersion"]
        .as_str()
        .unwrap()
        .starts_with('v'));
    println!(
        "SC-005 baseline: status round trip {} ms",
        elapsed.as_millis()
    );
    let unknown = gql(
        &url,
        &key,
        r#"mutation { runPluginOperation(plugin_id: "semantic-engine", args: {op: "nope"}) }"#,
    );
    assert_eq!(
        unknown["data"]["runPluginOperation"]["error"]["code"],
        "UNKNOWN_OPERATION"
    );
}
