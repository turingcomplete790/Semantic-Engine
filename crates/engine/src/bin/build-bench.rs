//! Measures a full graph build (spec 007 research R11; quickstart steps 2 and 4). Development
//! only: `scripts/package.sh` packages only the plugin binary, and CI rejects this one in the
//! package.
//!
//! usage: build-bench synth --scenes N --images M --dir DIR [--batch FACTS] [--out FILE]
//!        STASH_URL=… STASH_API_KEY=… build-bench stash --dir DIR [--out FILE]
//!
//! `synth` builds a fully tagged synthetic library (`engine::build::synth`) through a reader
//! that hands it over once. `stash` reads a real Stash, read-only, and builds into the local folder DIR. The API
//! key is read from the environment and never printed. The report (phase times, counts, facts,
//! peak memory) is printed and written to FILE, by default
//! `specs/007-graph-build/results/build-<command>.json`.

use engine::build::{run, synth, BuildOptions};
use engine::graph::GraphStore;
use serde_json::{json, Value};
use stash_adapter::reader::{
    Kind, LibraryRead, LibraryReader, MediaKind, MediaRecord, Progress, RecordBatch,
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

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return fail("usage: build-bench synth|stash --dir DIR [options]");
    };
    let (mut dir, mut out, mut scenes, mut images) = (None::<PathBuf>, None::<PathBuf>, 0, 0);
    let mut options = BuildOptions::default();
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
        "stash" => {
            let url = std::env::var("STASH_URL").unwrap_or_else(|_| "http://localhost:9998".into());
            let key = std::env::var("STASH_API_KEY").unwrap_or_default();
            let client = match stash_adapter::Client::with_api_key(&url, &key) {
                Ok(c) => c,
                Err(e) => return fail(&e.to_string()),
            };
            (json!({ "stash": url }), build(&client, &dir, &options))
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
