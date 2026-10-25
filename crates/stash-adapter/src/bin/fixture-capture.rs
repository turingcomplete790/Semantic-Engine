//! Captures read-only test fixtures of a library's semantic data (spec 002 FR-015, spec 005
//! FR-025), through the library reader.
//!
//! usage: STASH_URL=http://localhost:9999 STASH_API_KEY=... fixture-capture --out DIR
//!
//! Writes `library-read.json` (a full `LibraryRead`, which `MemoryReader::from_file` loads), the
//! same records split into tags.json, performers.json, scenes.json and images.json (each a list
//! ordered by id), and index.json (counts, empty sections, where the read started, requests).
//! Only the reader's read requests are sent, and no credential is ever written or printed.

use serde_json::{json, Value};
use stash_adapter::reader::LibraryReader;
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
    let read = match client.full_read(&|| false, &mut |_| {}) {
        Ok(r) => r,
        Err(e) => return fail(&scrub(&e.to_string(), &key)),
    };

    let sections: [(&str, Value); 4] = [
        ("tags", json!(read.tags)),
        ("performers", json!(read.performers)),
        ("scenes", json!(read.scenes)),
        ("images", json!(read.images)),
    ];
    let mut files: Vec<(String, String)> = vec![("library-read.json".into(), pretty(&json!(read)))];
    let mut counts = serde_json::Map::new();
    let mut empty = Vec::new();
    for (name, rows) in sections {
        let n = rows.as_array().map_or(0, Vec::len);
        counts.insert(name.to_string(), json!(n));
        if n == 0 {
            empty.push(name);
        }
        files.push((format!("{name}.json"), pretty(&rows)));
    }
    let index = json!({
        "source": "fixture-capture through the library reader (read-only; no credentials recorded)",
        "counts": counts,
        "empty": empty,
        "start": read.start,
        "missing": read.missing,
        "requests": read.requests,
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
