//! `concurrency --option A|B|C --scale S`: reads during a long write under each sharing option,
//! and two writers at once (spec FR-004, research R2, tasks T024).
//!
//! Each option gets a fresh store `persist-<option>`: a small base write (the test scale), then
//! one long write of `--scale` scenes on a thread while 4 reader threads loop `query` calls until
//! it ends, then two simultaneous small writes with `lockMode=fail` and again with `wait`.

use super::{args, call_op, summary, Bench, Exit, Failure, OpCall, Outcome};
use crate::measure::{Kind, Library, Measurement, Question};
use crate::synth::{self, Spec};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

const READERS: usize = 4;

#[derive(Debug, Clone)]
struct Read {
    at_ms: f64,
    wall_ms: f64,
    ok: bool,
    waited: bool,
    mixed: bool,
    generation: Value,
    error: Option<String>,
}

fn summarize_write(c: &Result<OpCall, Failure>) -> Value {
    match c {
        Ok(c) if c.ok() => {
            json!({ "ok": true, "wallMs": c.wall_ms, "timings": c.result()["timings"], "triples": c.result()["triples"] })
        }
        Ok(c) => json!({ "ok": false, "wallMs": c.wall_ms, "error": c.envelope["error"] }),
        Err(f) => json!({ "ok": false, "error": f.message }),
    }
}

pub fn command(b: &mut Bench) -> Outcome {
    let option = b.opt("--option").unwrap_or("B").to_string();
    if !matches!(option.as_str(), "A" | "B" | "C") {
        return Err(Failure::new(Exit::Usage, "--option must be A, B, or C"));
    }
    let scale = b.global.scale.clone();
    let scenes = synth::parse_scale(&scale).expect("validated");
    let store = format!("persist-{option}");
    let dir = b.store_root()?.join(&store);
    let _ = std::fs::remove_dir_all(&dir);
    let client = b.client()?;

    let write = |scale: &str, seed: u64, mode: &str| {
        call_op(
            client,
            "write",
            args(&[
                ("store", json!(store)),
                ("option", json!(option)),
                ("scale", json!(scale)),
                ("seed", json!(seed)),
                ("lockMode", json!(mode)),
                ("waitMs", json!(600_000)),
            ]),
        )
    };
    let base = write("test", 1, "wait")?;
    if !base.ok() {
        return Err(Failure::new(
            Exit::RunFailed,
            format!("base write: {}", base.envelope["error"]),
        ));
    }

    let done = AtomicBool::new(false);
    let reads: Mutex<Vec<Read>> = Mutex::new(Vec::new());
    let start = Instant::now();
    let long = thread::scope(|s| {
        for r in 0..READERS {
            let (done, reads, store, option) = (&done, &reads, &store, &option);
            s.spawn(move || {
                let mut i = r;
                while !done.load(Ordering::Relaxed) {
                    let at_ms = start.elapsed().as_secs_f64() * 1000.0;
                    let res = call_op(
                        client,
                        "query",
                        args(&[
                            ("store", json!(store)),
                            ("option", json!(option)),
                            ("queryId", json!((i % 5) as u64 + 1)),
                            ("waitMs", json!(600_000)),
                        ]),
                    );
                    let read = match res {
                        Ok(c) if c.ok() => {
                            let r = c.result();
                            Read {
                                at_ms,
                                wall_ms: c.wall_ms,
                                ok: true,
                                waited: r["timings"]["lock"].as_f64().unwrap_or(0.0) > 5.0,
                                mixed: r["consistency"]["mixed"] == Value::Bool(true),
                                generation: r["generation"].clone(),
                                error: None,
                            }
                        }
                        Ok(c) => Read {
                            at_ms,
                            wall_ms: c.wall_ms,
                            ok: false,
                            waited: false,
                            mixed: false,
                            generation: Value::Null,
                            error: Some(c.envelope["error"]["message"].to_string()),
                        },
                        Err(f) => Read {
                            at_ms,
                            wall_ms: 0.0,
                            ok: false,
                            waited: false,
                            mixed: false,
                            generation: Value::Null,
                            error: Some(f.message),
                        },
                    };
                    reads.lock().unwrap().push(read);
                    i += READERS;
                }
            });
        }
        let w = write(&scale, 2, "wait");
        done.store(true, Ordering::Relaxed);
        w
    });
    let write_ms = start.elapsed().as_secs_f64() * 1000.0;
    let reads = reads.into_inner().unwrap();

    // Two writers at once, failing fast, then waiting.
    let pair = |mode: &str| {
        thread::scope(|s| {
            let a = s.spawn(|| write("test", 3, mode));
            let c = s.spawn(|| write("test", 4, mode));
            [a.join().unwrap(), c.join().unwrap()]
                .iter()
                .map(summarize_write)
                .collect::<Vec<_>>()
        })
    };
    let lock_fail = pair("fail");
    let lock_wait = pair("wait");

    let ok: Vec<f64> = reads.iter().filter(|r| r.ok).map(|r| r.wall_ms).collect();
    let failed: Vec<&Read> = reads.iter().filter(|r| !r.ok).collect();
    let mut errors: Vec<String> = failed.iter().filter_map(|r| r.error.clone()).collect();
    errors.sort();
    errors.dedup();
    let generations: std::collections::BTreeSet<String> =
        reads.iter().map(|r| r.generation.to_string()).collect();
    let spec = Spec::new(scenes);
    let kind = if ok.len() >= 100 {
        Kind::Latency
    } else {
        Kind::Observation
    };
    let mut m = Measurement::new(
        &format!("persist-{option}-{scale}"),
        Question::Persistence,
        kind,
        &b.command_line(),
        b.server.clone(),
        Library::synthetic(
            scenes,
            spec.images(),
            spec.performers(),
            spec.tags(),
            long.as_ref()
                .ok()
                .and_then(|c| c.result()["triples"].as_u64())
                .unwrap_or(0),
        ),
    )
    .with_samples(ok);
    m.variant = option.clone();
    m.parts.insert(
        "readStartMs".into(),
        json!(reads.iter().map(|r| r.at_ms).collect::<Vec<_>>()),
    );
    m.parts.insert(
        "readWallMs".into(),
        json!(reads.iter().map(|r| r.wall_ms).collect::<Vec<_>>()),
    );
    for (i, r) in failed.iter().enumerate() {
        if i < 20 {
            m.add_failure(i as u64, "read", r.error.as_deref().unwrap_or(""));
        }
    }
    m.extra.insert("readers".into(), json!(READERS));
    m.extra.insert(
        "reads".into(),
        json!({
            "total": reads.len(),
            "ok": reads.iter().filter(|r| r.ok).count(),
            "waited": reads.iter().filter(|r| r.waited).count(),
            "failed": failed.len(),
            "mixed": reads.iter().filter(|r| r.mixed).count(),
            "generationsSeen": generations,
            "distinctErrors": errors,
            "latencySummary": summary(&reads.iter().filter(|r| r.ok).map(|r| r.wall_ms).collect::<Vec<_>>()),
        }),
    );
    m.extra.insert(
        "longWrite".into(),
        json!({ "result": summarize_write(&long), "wallMs": write_ms }),
    );
    m.extra.insert(
        "twoWriters".into(),
        json!({ "fail": lock_fail, "wait": lock_wait }),
    );
    b.record(&m)
}
