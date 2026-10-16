//! Captures read-only test fixtures of a library's semantic data (spec 002 FR-015).
//!
//! usage: STASH_URL=http://localhost:9999 STASH_API_KEY=... fixture-capture --out DIR
//!
//! Writes tags.json, performers.json, scenes.json, images.json (each a list ordered by id), and
//! index.json (the queries run, counts, empty sections). Only read queries are sent, and no
//! credential is ever written or printed.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

fn fail(msg: &str) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(1)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut out: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("usage: STASH_URL=... STASH_API_KEY=... fixture-capture --out DIR");
                return ExitCode::SUCCESS;
            }
            other => return fail(&format!("unknown argument: {other}")),
        }
    }
    let Some(out) = out else {
        return fail("--out DIR is required");
    };
    let url = std::env::var("STASH_URL").unwrap_or_default();
    let key = std::env::var("STASH_API_KEY").unwrap_or_default();
    if url.is_empty() {
        return fail(
            "set STASH_URL to the Stash server address, for example http://localhost:9999",
        );
    }

    let client = match stash_adapter::Client::with_api_key(&url, &key) {
        Ok(c) => c,
        Err(e) => return fail(&scrub(&e.to_string(), &key)),
    };
    // Capture everything before writing anything, so a failure leaves no partial fixtures.
    let capture = match client.capture_semantic_fixtures() {
        Ok(c) => c,
        Err(e) => return fail(&scrub(&e.to_string(), &key)),
    };

    let sections: [(&str, &BTreeMap<String, Value>); 4] = [
        ("tags", &capture.tags),
        ("performers", &capture.performers),
        ("scenes", &capture.scenes),
        ("images", &capture.images),
    ];
    let mut files: Vec<(String, String)> = Vec::new();
    let mut counts = serde_json::Map::new();
    let mut empty = Vec::new();
    for (name, map) in sections {
        let mut rows: Vec<&Value> = map.values().collect();
        rows.sort_by_key(|v| {
            v["id"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        });
        counts.insert(name.to_string(), json!(rows.len()));
        if rows.is_empty() {
            empty.push(name);
        }
        files.push((format!("{name}.json"), pretty(&json!(rows))));
    }
    let index = json!({
        "source": "fixture-capture (read-only queries; no credentials recorded)",
        "counts": counts,
        "empty": empty,
        "queries": capture.queries,
    });
    files.push(("index.json".to_string(), pretty(&index)));

    // Defence in depth: refuse to write a file that contains the key.
    if !key.is_empty() && files.iter().any(|(_, text)| text.contains(&key)) {
        return fail("a captured response contains the API key; nothing was written");
    }
    if let Err(e) = std::fs::create_dir_all(&out) {
        return fail(&format!("cannot create {}: {e}", out.display()));
    }
    for (name, text) in &files {
        if let Err(e) = std::fs::write(out.join(name), text) {
            return fail(&format!("cannot write {name}: {e}"));
        }
    }
    println!("wrote {} files to {}", files.len(), out.display());
    for (name, count) in &counts {
        println!("  {name}: {count}");
    }
    if !empty.is_empty() {
        println!("  empty: {}", empty.join(", "));
    }
    ExitCode::SUCCESS
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).expect("JSON serializes") + "\n"
}

fn scrub(msg: &str, key: &str) -> String {
    if key.is_empty() {
        msg.to_string()
    } else {
        msg.replace(key, "<redacted>")
    }
}
