//! Measures a full graph build (spec 007 research R11; quickstart steps 2 and 4). Development
//! only: `scripts/package.sh` packages only the plugin binary, and CI rejects this one in the
//! package.
//!
//! usage: build-bench synth --scenes N --images M --dir DIR [--batch FACTS] [--out FILE]
//!        build-bench update --scenes N --images M --change K --dir DIR [--out FILE]
//!        STASH_URL=… STASH_API_KEY=… build-bench stash --dir DIR [--out FILE]
//!
//! `update` (spec 008 research R10) builds the synthetic library, changes K scenes (a third
//! edited, a third deleted, a third new), and times one incremental update of them.
//! `synth` builds a fully tagged synthetic library (`engine::build::synth`) through a reader
//! that hands it over once. `stash` reads a real Stash, read-only, and builds into the local folder DIR. The API
//! key is read from the environment and never printed. The report (phase times, counts, facts,
//! peak memory) is printed and written to FILE, by default
//! `specs/007-graph-build/results/build-<command>.json`.

use engine::build::{run, synth, BuildOptions};
use engine::graph::GraphStore;
use serde_json::{json, Value};
use stash_adapter::reader::{
    Kind, LibraryRead, LibraryReader, MediaKind, MediaRecord, MemoryReader, Progress, RecordBatch,
    SemanticPerformer, SemanticTag,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

const SERVER_ID: &str = "00000000-0000-4000-8000-000000000001";

fn fail(msg: &str) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(1)
}

/// Peak resident memory in bytes (Linux `VmHWM`), or `None` elsewhere.
fn peak_memory() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

/// Hands its library over once, without the copy `MemoryReader` makes, so the measured peak is
/// that of one library in memory, as a real read from Stash gives.
struct OnceReader(RefCell<Option<LibraryRead>>);

fn unsupported<T>() -> Result<T, stash_adapter::Error> {
    Err(stash_adapter::Error::Setup(
        "not used by a full build".into(),
    ))
}

impl LibraryReader for OnceReader {
    fn full_read(
        &self,
        _: &dyn Fn() -> bool,
        _: &mut dyn FnMut(Progress),
    ) -> Result<LibraryRead, stash_adapter::Error> {
        self.0.borrow_mut().take().map_or_else(unsupported, Ok)
    }
    fn tag_records(&self, _: &[String]) -> Result<RecordBatch<SemanticTag>, stash_adapter::Error> {
        unsupported()
    }
    fn performer_records(
        &self,
        _: &[String],
    ) -> Result<RecordBatch<SemanticPerformer>, stash_adapter::Error> {
        unsupported()
    }
    fn media_records(
        &self,
        _: MediaKind,
        _: &[String],
    ) -> Result<RecordBatch<MediaRecord>, stash_adapter::Error> {
        unsupported()
    }
    fn changed_since(&self, _: Kind, _: &str) -> Result<Vec<String>, stash_adapter::Error> {
        unsupported()
    }
    fn all_ids(&self, _: Kind) -> Result<Vec<String>, stash_adapter::Error> {
        unsupported()
    }
    fn latest_updated_at(&self, _: Kind) -> Result<Option<String>, stash_adapter::Error> {
        unsupported()
    }
    fn semantic_tags(&self) -> Result<Vec<SemanticTag>, stash_adapter::Error> {
        unsupported()
    }
    fn performers_with_tags(&self, _: &[String]) -> Result<Vec<String>, stash_adapter::Error> {
        unsupported()
    }
}

fn build(
    reader: &dyn LibraryReader,
    dir: &PathBuf,
    options: &BuildOptions,
) -> Result<Value, String> {
    let _ = std::fs::remove_dir_all(dir);
    let store = GraphStore::new(dir, SERVER_ID);
    let start = Instant::now();
    let mut last = -1.0;
    let report = run(
        reader,
        &store,
        SERVER_ID,
        options,
        &mut |p| {
            if p - last >= 0.1 || p >= 1.0 {
                eprintln!(
                    "progress {:.0}% at {:.1} s",
                    p * 100.0,
                    start.elapsed().as_secs_f64()
                );
                last = p;
            }
        },
        // Note lines name an item, key and reason, never a value, so they are safe to print.
        &mut |_, m| eprintln!("{m}"),
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({
        "report": report,
        "peakMemoryBytes": peak_memory(),
    }))
}

/// Builds the synthetic library, changes `k` scenes, and times one update (research R10).
fn update_bench(
    dir: &PathBuf,
    scenes: usize,
    images: usize,
    k: usize,
    options: &BuildOptions,
) -> Result<Value, String> {
    let _ = std::fs::remove_dir_all(dir);
    let store = GraphStore::new(dir, SERVER_ID);
    let mut lib = synth::library(scenes, images);
    let t = Instant::now();
    run(
        &MemoryReader::new(lib.clone()),
        &store,
        SERVER_ID,
        options,
        &mut |_| {},
        &mut |_, _| {},
    )
    .map_err(|e| e.to_string())?;
    let build_ms = t.elapsed().as_millis() as u64;

    // The changes, each saved after every synthetic item (synth's times are all in September).
    let stamp = |n: usize| {
        format!(
            "2026-10-27T{:02}:{:02}:{:02}-06:00",
            n / 3600 % 24,
            n / 60 % 60,
            n % 60
        )
    };
    let (edits, deletes) = (k.div_ceil(3), k / 3);
    let adds = k - edits - deletes;
    let mut n = 0;
    for m in lib.scenes.iter_mut().take(edits) {
        n += 1;
        m.fields.insert(
            "pluggableAssignments".into(),
            stash_adapter::reader::field::Field::Value(json!({})),
        );
        m.updated_at = stamp(n);
    }
    let gone: std::collections::BTreeSet<String> = lib
        .scenes
        .iter()
        .skip(edits)
        .take(deletes)
        .map(|m| m.id.clone())
        .collect();
    lib.scenes.retain(|m| !gone.contains(&m.id));
    let template = lib.scenes[0].clone();
    for i in 0..adds {
        n += 1;
        let mut m = template.clone();
        m.id = (scenes + 1 + i).to_string();
        m.updated_at = stamp(n);
        lib.scenes.push(m);
    }
    if n > 0 {
        lib.start
            .insert(stash_adapter::reader::Kind::Scene, stamp(n));
    }

    let reader = MemoryReader::new(lib);
    let t = Instant::now();
    let report = engine::build::update::update(
        &reader,
        &store,
        SERVER_ID,
        options,
        &mut |_| {},
        &mut |_, m| eprintln!("{m}"),
    )
    .map_err(|e| e.to_string())?;
    let update_ms = t.elapsed().as_millis() as u64;
    Ok(json!({
        "buildMs": build_ms,
        "updateMs": update_ms,
        "changes": { "edited": edits, "deleted": deletes, "added": adds },
        "report": report,
        "peakMemoryBytes": peak_memory(),
    }))
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return fail("usage: build-bench synth|stash --dir DIR [options]");
    };
    let (mut dir, mut out, mut scenes, mut images) = (None::<PathBuf>, None::<PathBuf>, 0, 0);
    let mut options = BuildOptions::default();
    let mut change = 0usize;
    let rest: Vec<String> = args.collect();
    for pair in rest.chunks(2) {
        let value = pair.get(1).cloned();
        let number = value.as_deref().and_then(|v| v.parse::<usize>().ok());
        match pair[0].as_str() {
            "--dir" => dir = value.map(PathBuf::from),
            "--out" => out = value.map(PathBuf::from),
            "--scenes" => scenes = number.unwrap_or(0),
            "--images" => images = number.unwrap_or(0),
            "--batch" => options.batch_facts = number.unwrap_or(options.batch_facts),
            "--change" => change = number.unwrap_or(0),
            other => return fail(&format!("unknown argument: {other}")),
        }
    }
    let Some(dir) = dir else {
        return fail("--dir DIR is required");
    };
    let (input, result) = match command.as_str() {
        "synth" => {
            if scenes == 0 {
                return fail("synth needs --scenes N (and optionally --images M)");
            }
            let t = Instant::now();
            let reader = OnceReader(RefCell::new(Some(synth::library(scenes, images))));
            let generate_ms = t.elapsed().as_millis() as u64;
            let generated_peak = peak_memory();
            (
                json!({ "scenes": scenes, "images": images, "generateMs": generate_ms,
                        "peakMemoryAfterGenerateBytes": generated_peak }),
                build(&reader, &dir, &options),
            )
        }
        "update" => {
            if scenes == 0 {
                return fail("update needs --scenes N, and --change K");
            }
            (
                json!({ "scenes": scenes, "images": images, "change": change }),
                update_bench(&dir, scenes, images, change, &options),
            )
        }
        "stash" => {
            let url = std::env::var("STASH_URL").unwrap_or_else(|_| "http://localhost:9998".into());
            let key = std::env::var("STASH_API_KEY").unwrap_or_default();
            let client = match stash_adapter::Client::with_api_key(&url, &key) {
                Ok(c) => c,
                Err(e) => return fail(&e.to_string()),
            };
            // Discovery bounds (spec 008 FR-020): one full read on its own, with its request count.
            let t = Instant::now();
            let read = match client.full_read(&|| false, &mut |_| {}) {
                Ok(r) => r,
                Err(e) => return fail(&e.to_string()),
            };
            let records =
                read.tags.len() + read.performers.len() + read.scenes.len() + read.images.len();
            let discovery = json!({
                "readMs": t.elapsed().as_millis() as u64,
                "requests": read.requests,
                "records": records,
                "requestsPerRecord": read.requests as f64 / records.max(1) as f64,
            });
            (
                json!({ "stash": url, "fullRead": discovery }),
                build(&client, &dir, &options),
            )
        }
        other => return fail(&format!("unknown command: {other}")),
    };
    let result = match result {
        Ok(r) => r,
        Err(e) => return fail(&e),
    };
    let doc = json!({ "command": command, "batchFacts": options.batch_facts, "input": input, "result": result });
    let text = serde_json::to_string_pretty(&doc).expect("JSON serializes") + "\n";
    println!("{text}");
    let path = out.unwrap_or_else(|| {
        PathBuf::from(format!(
            "specs/007-graph-build/results/build-{command}.json"
        ))
    });
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, &text) {
        Ok(()) => println!("wrote {}", path.display()),
        Err(e) => eprintln!("could not write {}: {e}", path.display()),
    }
    ExitCode::SUCCESS
}
