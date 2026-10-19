//! `call-cost --scale S`: what one engine call costs with the stored graph reopened every call
//! (spec FR-003, research R5, tasks T023).
//!
//! Order: reload plugins and time the first call (kept out of the samples), then 100 local
//! process starts of the spike plugin (`ping` on stdin, no Stash: the 002 baseline method), 100
//! `ping` calls through Stash, and `--runs` (default 100) `query` calls cycling queryId 1–5.

use super::{args, summary, Bench, Exit, Failure, Outcome};
use crate::measure::{Kind, Library, Measurement, Question, Series};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

fn local_starts(
    binary: &std::path::Path,
    plugin_dir: &std::path::Path,
    runs: usize,
) -> Result<Vec<f64>, Failure> {
    let input = json!({
        "server_connection": {"Scheme": "http", "Host": "localhost", "Port": 9998, "Dir": "", "PluginDir": plugin_dir},
        "args": {"op": "ping"}
    })
    .to_string();
    let mut out = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        let mut child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Failure::new(Exit::RunFailed, format!("{}: {e}", binary.display())))?;
        child.stdin.take().unwrap().write_all(input.as_bytes()).ok();
        child
            .wait()
            .map_err(|e| Failure::new(Exit::RunFailed, e.to_string()))?;
        out.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(out)
}

pub fn command(b: &mut Bench) -> Outcome {
    let scale = b.global.scale.clone();
    let runs = b.global.runs.unwrap_or(100);
    let plugin_dir = b.plugin_dir()?;
    let library = super::build::store_spec(&plugin_dir.join("store").join(&scale)).ok_or_else(|| {
        Failure::new(
            Exit::Usage,
            format!("no synthetic store for scale {scale}; run `spike-bench synth --scale {scale}` first"),
        )
    })?;
    let query = |b: &Bench, id: u64| {
        b.call_op(
            "query",
            args(&[("store", json!(scale)), ("queryId", json!(id))]),
        )
    };

    let mut series = Series::default();
    b.client()?.reload_plugins()?;
    series.mark_reload();
    let first = query(b, 1)?;
    if !first.ok() {
        return Err(Failure::new(
            Exit::RunFailed,
            format!("query: {}", first.envelope["error"]),
        ));
    }
    series.push(first.wall_ms);

    let starts = local_starts(
        &plugin_dir.join(if cfg!(windows) {
            "spike-plugin.exe"
        } else {
            "spike-plugin"
        }),
        &plugin_dir,
        100,
    )?;
    let mut pings = Vec::with_capacity(100);
    for _ in 0..100 {
        pings.push(b.call_op("ping", args(&[]))?.wall_ms);
    }

    let (mut open, mut q, mut ser, mut inside) = (vec![], vec![], vec![], vec![]);
    let mut rows: Vec<Value> = vec![Value::Null; 6];
    for i in 0..runs {
        let id = (i % 5) as u64 + 1;
        let c = query(b, id)?;
        if !c.ok() {
            return Err(Failure::new(
                Exit::RunFailed,
                format!("query {id}: {}", c.envelope["error"]),
            ));
        }
        let t = &c.result()["timings"];
        open.push(t["storeOpen"].as_f64().unwrap_or(0.0));
        q.push(t["query"].as_f64().unwrap_or(0.0));
        ser.push(t["serialise"].as_f64().unwrap_or(0.0));
        inside.push(t["total"].as_f64().unwrap_or(0.0));
        rows[id as usize] = c.result()["rowCount"].clone();
        series.push(c.wall_ms);
    }

    let mut base = Measurement::new(
        &format!("process-reopen-{scale}"),
        Question::Process,
        Kind::Latency,
        &b.command_line(),
        b.server.clone(),
        Library { ..library },
    );
    base.variant = "reopen".into();
    let (mut m, first) = series.into_measurements(base);
    m.parts.insert("processStart".into(), json!(starts));
    m.parts.insert("stashPing".into(), json!(pings));
    m.parts.insert("storeOpen".into(), json!(open));
    m.parts.insert("query".into(), json!(q));
    m.parts.insert("serialise".into(), json!(ser));
    m.parts.insert("pluginTotal".into(), json!(inside));
    m.extra.insert(
        "partsSummary".into(),
        json!({
            "processStart": summary(&starts),
            "stashPing": summary(&pings),
            "storeOpen": summary(&open),
            "query": summary(&q),
            "serialise": summary(&ser),
            "pluginTotal": summary(&inside),
        }),
    );
    m.extra.insert("rowsByQueryId".into(), json!(rows[1..]));
    m.extra.insert("budgetMs".into(), json!(200));
    let p95 = m.p95;
    m.extra
        .insert("meetsBudgetAtP95".into(), json!(p95 < 200.0));
    b.record(&m)?;
    if let Some(mut f) = first {
        f.id = format!("process-first-call-{scale}");
        b.record(&f)?;
    }
    Ok(())
}
