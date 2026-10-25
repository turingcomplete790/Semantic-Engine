//! Read-only timings of the library reader (spec 005 quickstart step 4; SC-002 to SC-005).
//!
//! usage: STASH_URL=http://localhost:9998 STASH_API_KEY=... reader-bench full|ids|change
//!        [--runs N] [--out DIR]
//!
//! - `full`: a full read; time, ms per record, requests, record counts, peak memory.
//! - `ids`: every id of every kind (the deletion check).
//! - `change`: the change read right after a save: from one second before the newest
//!   `updated_at` of each kind, which returns at least the newest item.
//!
//! Only `LibraryReader` reads are sent, so it is safe against any server, Production included.
//! Results go to `--out` (default `specs/005-stash-adapter/results`) as `reader-{command}.json`.
//! No credential is printed or written.

use serde_json::{json, Value};
use stash_adapter::reader::time::one_second_earlier;
use stash_adapter::reader::{Kind, LibraryReader};
use stash_adapter::Client;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

fn fail(msg: &str) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(1)
}

/// Peak resident memory of this process in KiB (`VmHWM`, Linux only).
fn peak_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn summary(mut ms: Vec<f64>) -> Value {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
    json!({ "runs": ms.len(), "min": ms[0], "median": at(0.5), "p95": at(0.95), "max": ms[ms.len() - 1] })
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let out = f();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return fail("usage: reader-bench full|ids|change [--runs N] [--out DIR]");
    };
    let mut runs = 5usize;
    let mut out = PathBuf::from("specs/005-stash-adapter/results");
    while let Some(a) = args.next() {
        match a.as_str() {
            "--runs" => match args.next().and_then(|v| v.parse().ok()) {
                Some(n) if n > 0 => runs = n,
                _ => return fail("--runs needs a positive number"),
            },
            "--out" => match args.next() {
                Some(d) => out = PathBuf::from(d),
                None => return fail("--out needs a folder"),
            },
            other => return fail(&format!("unknown argument: {other}")),
        }
    }
    let url = std::env::var("STASH_URL").unwrap_or_else(|_| "http://localhost:9998".into());
    let key = std::env::var("STASH_API_KEY").unwrap_or_default();
    let scrub = |m: String| {
        if key.is_empty() {
            m
        } else {
            m.replace(&key, "<redacted>")
        }
    };
    let client = match Client::with_api_key(&url, &key) {
        Ok(c) => c,
        Err(e) => return fail(&scrub(e.to_string())),
    };
    let version = client.stash_version().ok().flatten();

    let result = match command.as_str() {
        "full" => {
            let mut ms = Vec::new();
            let mut last = None;
            for _ in 0..runs {
                let (r, t) = timed(|| client.full_read(&|| false, &mut |_| {}));
                match r {
                    Ok(read) => last = Some(read),
                    Err(e) => return fail(&scrub(e.to_string())),
                }
                ms.push(t);
            }
            let read = last.expect("at least one run");
            let records =
                read.tags.len() + read.performers.len() + read.scenes.len() + read.images.len();
            let s = summary(ms);
            let median = s["median"].as_f64().unwrap();
            json!({
                "records": {
                    "tags": read.tags.len(), "performers": read.performers.len(),
                    "scenes": read.scenes.len(), "images": read.images.len(), "total": records,
                },
                "requests": read.requests,
                "ms": s,
                "msPerRecord": if records > 0 { median / records as f64 } else { 0.0 },
                "readBytes": serde_json::to_vec(&read).map(|v| v.len()).unwrap_or(0),
                "peakKiB": peak_kib(),
            })
        }
        "ids" => {
            let mut ms = Vec::new();
            let mut counts = json!({});
            for _ in 0..runs {
                let (r, t) = timed(|| {
                    Kind::ALL
                        .iter()
                        .map(|k| LibraryReader::all_ids(&client, *k).map(|v| (k.as_str(), v.len())))
                        .collect::<Result<Vec<_>, _>>()
                });
                match r {
                    Ok(list) => {
                        counts = json!(list
                            .into_iter()
                            .collect::<std::collections::BTreeMap<_, _>>())
                    }
                    Err(e) => return fail(&scrub(e.to_string())),
                }
                ms.push(t);
            }
            json!({ "ids": counts, "ms": summary(ms) })
        }
        "change" => {
            let mut thresholds = Vec::new();
            for k in Kind::ALL {
                match LibraryReader::latest_updated_at(&client, k) {
                    Ok(Some(t)) => match one_second_earlier(&t) {
                        Some(th) => thresholds.push((k, th)),
                        None => return fail(&format!("{t:?} is not a Stash time")),
                    },
                    Ok(None) => {}
                    Err(e) => return fail(&scrub(e.to_string())),
                }
            }
            let mut per_kind = serde_json::Map::new();
            for (k, th) in &thresholds {
                let mut ms = Vec::new();
                let mut found = 0;
                for _ in 0..runs {
                    let (r, t) = timed(|| LibraryReader::changed_since(&client, *k, th));
                    match r {
                        Ok(ids) => found = ids.len(),
                        Err(e) => return fail(&scrub(e.to_string())),
                    }
                    ms.push(t);
                }
                per_kind.insert(
                    k.as_str().into(),
                    json!({ "threshold": th, "found": found, "ms": summary(ms) }),
                );
            }
            Value::Object(per_kind)
        }
        other => return fail(&format!("unknown command: {other}")),
    };

    let doc = json!({
        "command": format!("reader-bench {command} --runs {runs}"),
        "server": url,
        "stashVersion": version,
        "engineCommit": option_env!("SE_COMMIT"),
        "takenAt": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "result": result,
    });
    let text = serde_json::to_string_pretty(&doc).expect("JSON serializes") + "\n";
    if !key.is_empty() && text.contains(&key) {
        return fail("the result contains the API key; nothing was written");
    }
    if let Err(e) = std::fs::create_dir_all(&out) {
        return fail(&format!("cannot create {}: {e}", out.display()));
    }
    let path = out.join(format!("reader-{command}.json"));
    if let Err(e) = std::fs::write(&path, &text) {
        return fail(&format!("cannot write {}: {e}", path.display()));
    }
    println!("{text}");
    println!("wrote {}", path.display());
    ExitCode::SUCCESS
}
