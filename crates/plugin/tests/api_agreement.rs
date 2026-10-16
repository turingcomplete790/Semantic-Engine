//! FR-012: the plugin's operations, `api/operations.json`, and `docs/api.md` must agree on
//! operation and task names, error codes, and the contract version.

use std::collections::BTreeSet;
use std::path::Path;

fn repo_file(rel: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel),
    )
    .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn json() -> serde_json::Value {
    serde_json::from_str(&repo_file("api/operations.json")).unwrap()
}

/// Backticked names in `### ` headings under the given `## ` section of docs/api.md.
fn doc_headings(section: &str) -> BTreeSet<String> {
    let doc = repo_file("docs/api.md");
    let mut in_section = false;
    let mut out = BTreeSet::new();
    for line in doc.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            in_section = h.trim() == section;
        } else if in_section {
            if let Some(h) = line.strip_prefix("### ") {
                let name = h
                    .split('`')
                    .nth(1)
                    .unwrap_or_else(|| panic!("heading without `name`: {h}"));
                out.insert(name.to_string());
            }
        }
    }
    out
}

#[test]
fn operation_and_task_names_agree() {
    let code: BTreeSet<String> = plugin::ops::names().into_iter().map(String::from).collect();
    let j = json();
    let mut listed = BTreeSet::new();
    for o in j["operations"].as_array().unwrap() {
        listed.insert(o["name"].as_str().unwrap().to_string());
    }
    for t in j["tasks"].as_array().unwrap() {
        listed.insert(t["op"].as_str().unwrap().to_string());
    }
    let mut documented = doc_headings("Operations");
    documented.extend(doc_headings("Tasks"));
    assert_eq!(code, listed, "plugin registry vs api/operations.json");
    assert_eq!(code, documented, "plugin registry vs docs/api.md headings");
}

#[test]
fn task_names_match_the_manifest() {
    let manifest = repo_file("plugin/semantic-engine.yml");
    for t in json()["tasks"].as_array().unwrap() {
        let name = t["name"].as_str().unwrap();
        let op = t["op"].as_str().unwrap();
        assert!(
            manifest.contains(&format!("- name: {name}")),
            "manifest lacks task {name}"
        );
        assert!(
            manifest.contains(&format!("op: {op}")),
            "manifest lacks op {op}"
        );
    }
}

#[test]
fn error_codes_agree() {
    let code: BTreeSet<String> = engine::ErrorCode::ALL
        .iter()
        .map(|c| c.as_str().to_string())
        .collect();
    let j = json();
    let listed: BTreeSet<String> = j["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["code"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(code, listed, "ErrorCode vs api/operations.json");
    // Every per-operation error must be a known code.
    for group in ["operations", "tasks"] {
        for o in j[group].as_array().unwrap() {
            for e in o["errors"].as_array().unwrap() {
                assert!(
                    listed.contains(e.as_str().unwrap()),
                    "unknown code {e} in {o}"
                );
            }
        }
    }
    let doc = repo_file("docs/api.md");
    for c in &code {
        assert!(
            doc.contains(&format!("`{c}`")),
            "docs/api.md does not document {c}"
        );
    }
}

#[test]
fn contract_version_agrees() {
    let doc = repo_file("docs/api.md");
    assert_eq!(json()["contractVersion"], engine::CONTRACT_VERSION);
    assert!(
        doc.contains(&format!(
            "Contract version: **{}**",
            engine::CONTRACT_VERSION
        )),
        "docs/api.md must state the contract version"
    );
    assert!(
        doc.contains(&format!("## {} ", engine::CONTRACT_VERSION))
            || doc.contains(&format!("### {} ", engine::CONTRACT_VERSION)),
        "docs/api.md change log must have an entry for {}",
        engine::CONTRACT_VERSION
    );
}
