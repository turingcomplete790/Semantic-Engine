//! Opt-in end-to-end test of the server identifier against a real Stash (US2 independent test).
//!
//! Runs only when STASH_TEST_URL and STASH_TEST_API_KEY are set. If STASH_TEST_PACKAGE and
//! STASH_TEST_PLUGINS_DIR are set, it first installs that package and reloads plugins.
//! It writes the `serverId` setting of the semantic-engine plugin on that server (once; later
//! runs find it existing).

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn gql(url: &str, key: &str, query: &str) -> serde_json::Value {
    stash_adapter::install_tls_provider();
    // A fresh HTTP client per call, so each read is a separate client connection.
    let v: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("{url}/graphql"))
        .header("ApiKey", key)
        .json(&serde_json::json!({ "query": query }))
        .send()
        .expect("Stash reachable")
        .json()
        .expect("JSON response");
    assert!(v.get("errors").is_none(), "GraphQL errors: {v}");
    v
}

fn run_task_and_wait(url: &str, key: &str) {
    let r = gql(
        url,
        key,
        r#"mutation { runPluginTask(plugin_id: "semantic-engine", task_name: "Generate server identifier") }"#,
    );
    let job = r["data"]["runPluginTask"].as_str().unwrap().to_string();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let j = gql(
            url,
            key,
            &format!(r#"{{ findJob(input: {{id: "{job}"}}) {{ status error }} }}"#),
        );
        let status = j["data"]["findJob"]["status"]
            .as_str()
            .unwrap_or("FINISHED")
            .to_string();
        if status == "FINISHED" {
            return;
        }
        assert!(
            status != "FAILED" && status != "CANCELLED",
            "task {status}: {j}"
        );
        assert!(Instant::now() < deadline, "task did not finish: {j}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn stored_id(url: &str, key: &str) -> String {
    let c = gql(
        url,
        key,
        r#"{ configuration { plugins(include: ["semantic-engine"]) } }"#,
    );
    c["data"]["configuration"]["plugins"]["semantic-engine"]["serverId"]
        .as_str()
        .unwrap_or_else(|| panic!("no serverId stored: {c}"))
        .to_string()
}

#[test]
fn task_creates_one_identifier_that_every_client_reads() {
    let (Some(url), Some(key)) = (env("STASH_TEST_URL"), env("STASH_TEST_API_KEY")) else {
        eprintln!("notice: STASH_TEST_URL and STASH_TEST_API_KEY not set; skipped");
        return;
    };
    if let (Some(pkg), Some(dir)) = (env("STASH_TEST_PACKAGE"), env("STASH_TEST_PLUGINS_DIR")) {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/install.sh");
        let out = Command::new("sh")
            .arg(script)
            .args(["--from-file", &pkg, "--plugins-dir", &dir, "--yes"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        gql(&url, &key, "mutation { reloadPlugins }");
    }

    run_task_and_wait(&url, &key);
    let first = stored_id(&url, &key);
    assert!(engine::server_id::is_valid_server_id(&first), "{first}");
    run_task_and_wait(&url, &key);
    let a = stored_id(&url, &key);
    let b = stored_id(&url, &key);
    assert_eq!(first, a, "a second task run must not change the identifier");
    assert_eq!(a, b, "two clients must read the same identifier");

    let r = gql(
        &url,
        &key,
        r#"mutation { runPluginOperation(plugin_id: "semantic-engine", args: {op: "getServerId"}) }"#,
    );
    assert_eq!(
        r["data"]["runPluginOperation"]["result"]["serverId"],
        first.as_str()
    );
    println!("serverId on the test server: {first}");
}
