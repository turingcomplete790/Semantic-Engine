//! `payload`: the largest answer one call through Stash can return, and the page size Phase 3
//! should use (spec FR-008, research R7, tasks T036).
//!
//! The answer size doubles from 1 KiB until a call fails or its median passes 1 s. Every size
//! gets `--runs` (default 100) calls while its latency stays under the 200 ms budget; past that,
//! [`SLOW_RUNS`] calls are enough to place the curve. The proposed page size is the largest size
//! whose p95 is under 200 ms, and the record's samples are that size's calls.

use super::{args, summary, Bench, Exit, Failure, Outcome};
use crate::measure::{Kind, Library, Measurement, Question};
use crate::plugin_ops::payload::MAX_BYTES;
use serde_json::{json, Value};

/// The latency budget of one call (spec FR-003).
pub const BUDGET_MS: f64 = 200.0;
/// The median at which the curve stops (research R7).
pub const STOP_MS: f64 = 1000.0;
/// Calls per size once a size is over budget.
pub const SLOW_RUNS: usize = 5;
pub const START_BYTES: u64 = 1024;

/// One point of the size curve.
struct Point {
    bytes: u64,
    samples: Vec<f64>,
    received_bytes: u64,
    count: u64,
    build_ms: Vec<f64>,
}

impl Point {
    fn row(&self) -> Value {
        let s = summary(&self.samples);
        json!({
            "bytes": self.bytes,
            "receivedBytes": self.received_bytes,
            "bindings": self.count,
            "runs": s["runs"],
            "median": s["median"],
            "p95": s["p95"],
            "min": s["min"],
            "max": s["max"],
            "pluginBuildMedian": summary(&self.build_ms)["median"],
        })
    }
    fn stat(&self, name: &str) -> f64 {
        summary(&self.samples)[name]
            .as_f64()
            .unwrap_or(f64::INFINITY)
    }
}

fn human(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{} MiB", b >> 20),
        b => format!("{} KiB", b >> 10),
    }
}

pub fn command(b: &mut Bench) -> Outcome {
    let runs = b.global.runs.unwrap_or(100);
    // One call first, so connection set-up is not in the first sample.
    b.call_op("payload", args(&[("bytes", json!(START_BYTES))]))?;

    let mut curve: Vec<Point> = Vec::new();
    let mut failure: Option<Value> = None;
    let mut over_budget = false;
    let mut bytes = START_BYTES;
    'sizes: while bytes <= MAX_BYTES {
        let n = if over_budget { SLOW_RUNS } else { runs };
        let mut p = Point {
            bytes,
            samples: Vec::with_capacity(n),
            received_bytes: 0,
            count: 0,
            build_ms: Vec::with_capacity(n),
        };
        for run in 0..n {
            let call = match b.call_op("payload", args(&[("bytes", json!(bytes))])) {
                Ok(c) => c,
                Err(f) => {
                    failure = Some(
                        json!({ "bytes": bytes, "run": run, "message": b.redact(&f.message) }),
                    );
                    break 'sizes;
                }
            };
            if !call.ok() {
                failure = Some(
                    json!({ "bytes": bytes, "run": run, "message": call.envelope["error"].to_string() }),
                );
                break 'sizes;
            }
            if run == 0 {
                let r = call.result();
                p.received_bytes = r["bindings"].to_string().len() as u64;
                p.count = r["bindings"].as_array().map_or(0, |a| a.len() as u64);
            }
            p.build_ms
                .push(call.result()["timings"]["build"].as_f64().unwrap_or(0.0));
            p.samples.push(call.wall_ms);
        }
        let (median, p95) = (p.stat("median"), p.stat("p95"));
        b.say(format!(
            "{:>8}: median {median:.1} ms, p95 {p95:.1} ms ({} runs)",
            human(bytes),
            p.samples.len()
        ));
        over_budget |= median > BUDGET_MS;
        curve.push(p);
        if median > STOP_MS {
            failure =
                Some(json!({ "bytes": bytes, "reason": "median over 1 s", "median": median }));
            break;
        }
        bytes *= 2;
    }

    let proposed = curve
        .iter()
        .filter(|p| p.samples.len() >= Kind::Latency.min_runs() && p.stat("p95") < BUDGET_MS)
        .max_by_key(|p| p.bytes)
        .ok_or_else(|| {
            Failure::new(
                Exit::RunFailed,
                format!("no answer size stayed under {BUDGET_MS} ms at p95"),
            )
        })?;

    let mut m = Measurement::new(
        "limits-payload",
        Question::Limits,
        Kind::Latency,
        &b.command_line(),
        b.server.clone(),
        Library::test(0, 0, 0, 0, 0),
    )
    .with_samples(proposed.samples.clone());
    m.variant = "payload".into();
    m.extra.insert(
        "curve".into(),
        Value::Array(curve.iter().map(Point::row).collect()),
    );
    m.extra.insert(
        "failurePoint".into(),
        failure.clone().unwrap_or(Value::Null),
    );
    m.extra.insert(
        "proposedPageSize".into(),
        json!({
            "bytes": proposed.bytes,
            "receivedBytes": proposed.received_bytes,
            "bindings": proposed.count,
            "p95": proposed.stat("p95"),
            "rule": "largest size whose p95 is under 200 ms (R7)",
        }),
    );
    m.extra.insert("budgetMs".into(), json!(BUDGET_MS));
    m.extra.insert(
        "samplesAre".into(),
        json!("calls at the proposed page size"),
    );
    if let Some(f) = &failure {
        if f.get("message").is_some() {
            let run = f["run"].as_u64().unwrap_or(0);
            m.add_failure(run, "call", f["message"].as_str().unwrap_or(""));
        }
    }
    b.say(format!(
        "proposed page size: {} ({} bindings)",
        human(proposed.bytes),
        proposed.count
    ));
    b.record(&m)
}
