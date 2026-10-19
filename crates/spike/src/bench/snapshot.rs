//! `snapshot --path op|asset --scale S`: the two ways a client can take a whole published
//! generation (spec FR-009, research R6, tasks T038).
//!
//! - **op**: `exportSnapshot` with `to: output`; the snapshot comes back inside the
//!   `runPluginOperation` answer as base64 of gzip N-Quads.
//! - **asset**: `exportSnapshot` with `to: asset` writes the file, then the client fetches
//!   `/plugin/semantic-engine-spike/assets/snapshot.nq.gz` with its Stash login.
//!
//! Each run's sample is the client's total (export plus fetch, and decoding for op). A run that
//! fails is timed and recorded as a failure, so an answer too large for Stash shows in the result
//! instead of stopping the measurement. After the runs, the harness checks that the asset route
//! refuses a request without credentials, and that `snapshotInfo` lets a client tell whether the
//! generation it holds is current.

use super::{args, summary, Bench, Exit, Failure, Outcome};
use crate::measure::{Kind, Measurement, Question};
use crate::plugin_ops::snapshot::{MANIFEST_FILE, SNAPSHOT_FILE};
use crate::plugin_ops::PLUGIN_ID;
use base64::Engine as _;
use serde_json::{json, Value};
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// What one successful run held at the end: the manifest's generation and the sizes.
struct Got {
    generation: Option<u64>,
    bytes: u64,
    transfer_bytes: u64,
    quads: u64,
}

pub fn command(b: &mut Bench) -> Outcome {
    let path = b.opt("--path").unwrap_or("").to_string();
    if path != "op" && path != "asset" {
        return Err(Failure::new(Exit::Usage, "--path must be op or asset"));
    }
    let scale = b.global.scale.clone();
    let runs = b.global.runs.unwrap_or(3);
    let library = super::build::store_spec(&b.store_root()?.join(&scale)).ok_or_else(|| {
        Failure::new(
            Exit::Usage,
            format!("no synthetic store for scale {scale}; run `spike-bench synth --scale {scale}` first"),
        )
    })?;

    let mut m = Measurement::new(
        &format!("snapshot-{path}-{scale}"),
        Question::Snapshot,
        Kind::Build,
        &b.command_line(),
        b.server.clone(),
        library,
    );
    m.variant = path.clone();
    let (mut samples, mut export, mut fetch, mut decode, mut dump) =
        (vec![], vec![], vec![], vec![], vec![]);
    let mut got: Option<Got> = None;

    for run in 0..runs {
        let t = Instant::now();
        let outcome = if path == "op" {
            run_op(b, &scale, &mut export, &mut decode, &mut dump)
        } else {
            run_asset(b, &scale, &mut export, &mut fetch, &mut dump)
        };
        samples.push(ms(t));
        match outcome {
            Ok(g) => got = Some(g),
            Err(f) if f.exit == Exit::Unreachable && run == 0 && got.is_none() => {
                // Stash itself went away: not a size limit.
                return Err(f);
            }
            Err(f) => {
                let msg = b.redact(&f.message);
                b.warn(format!("run {run}: {msg}"));
                m.add_failure(run as u64, "call", &msg);
            }
        }
    }
    m = m.with_samples(samples);
    m.parts.insert("export".into(), json!(export));
    m.parts.insert("pluginDump".into(), json!(dump));
    if path == "op" {
        m.parts.insert("clientDecode".into(), json!(decode));
    } else {
        m.parts.insert("fetch".into(), json!(fetch));
    }
    m.extra.insert(
        "partsSummary".into(),
        json!({
            "export": summary(&export),
            "pluginDump": summary(&dump),
            "clientDecode": summary(&decode),
            "fetch": summary(&fetch),
        }),
    );

    if let Some(g) = &got {
        m.extra.insert(
            "sizes".into(),
            json!({
                "quads": g.quads,
                "gzipBytes": g.bytes,
                "transferBytes": g.transfer_bytes,
                "transferIs": if path == "op" { "base64 text inside the GraphQL JSON answer" } else { "the gzip file as served" },
            }),
        );
        m.extra
            .insert("freshness".into(), freshness(b, &scale, g.generation)?);
    }
    if path == "asset" {
        m.extra.insert("assetAuth".into(), asset_auth(b)?);
    }
    b.record(&m)
}

/// `exportSnapshot` with `to: output`, then the client decodes the base64.
fn run_op(
    b: &Bench,
    scale: &str,
    export: &mut Vec<f64>,
    decode: &mut Vec<f64>,
    dump: &mut Vec<f64>,
) -> Result<Got, Failure> {
    let call = b.call_op(
        "exportSnapshot",
        args(&[("store", json!(scale)), ("to", json!("output"))]),
    )?;
    if !call.ok() {
        return Err(Failure::new(
            Exit::RunFailed,
            call.envelope["error"].to_string(),
        ));
    }
    export.push(call.wall_ms);
    let r = call.result();
    dump.push(r["timings"]["dump"].as_f64().unwrap_or(0.0));
    let text = r["data"].as_str().unwrap_or("");
    let t = Instant::now();
    let data = base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|e| Failure::new(Exit::RunFailed, format!("base64: {e}")))?;
    decode.push(ms(t));
    let manifest = &r["manifest"];
    if manifest["bytes"].as_u64() != Some(data.len() as u64) {
        return Err(Failure::new(
            Exit::RunFailed,
            format!(
                "received {} bytes, manifest says {}",
                data.len(),
                manifest["bytes"]
            ),
        ));
    }
    Ok(Got {
        generation: manifest["generation"].as_u64(),
        bytes: data.len() as u64,
        transfer_bytes: text.len() as u64,
        quads: manifest["quads"].as_u64().unwrap_or(0),
    })
}

/// `exportSnapshot` with `to: asset`, then the client GETs the file with its Stash login.
fn run_asset(
    b: &Bench,
    scale: &str,
    export: &mut Vec<f64>,
    fetch: &mut Vec<f64>,
    dump: &mut Vec<f64>,
) -> Result<Got, Failure> {
    let call = b.call_op(
        "exportSnapshot",
        args(&[("store", json!(scale)), ("to", json!("asset"))]),
    )?;
    if !call.ok() {
        return Err(Failure::new(
            Exit::RunFailed,
            call.envelope["error"].to_string(),
        ));
    }
    export.push(call.wall_ms);
    let r = call.result();
    dump.push(r["timings"]["dump"].as_f64().unwrap_or(0.0));
    let t = Instant::now();
    let asset = b.client()?.plugin_asset(PLUGIN_ID, SNAPSHOT_FILE)?;
    fetch.push(ms(t));
    if asset.status != 200 {
        return Err(Failure::new(
            Exit::RunFailed,
            format!("GET {SNAPSHOT_FILE} answered HTTP {}", asset.status),
        ));
    }
    let manifest = &r["manifest"];
    if manifest["bytes"].as_u64() != Some(asset.body.len() as u64) {
        return Err(Failure::new(
            Exit::RunFailed,
            format!(
                "fetched {} bytes, manifest says {}",
                asset.body.len(),
                manifest["bytes"]
            ),
        ));
    }
    Ok(Got {
        generation: manifest["generation"].as_u64(),
        bytes: asset.body.len() as u64,
        transfer_bytes: asset.body.len() as u64,
        quads: manifest["quads"].as_u64().unwrap_or(0),
    })
}

/// Can a client holding generation `held` tell whether it is current? `snapshotInfo` is timed
/// over 100 calls, as it is the call a client would make before every reuse.
fn freshness(b: &Bench, scale: &str, held: Option<u64>) -> Result<Value, Failure> {
    let mut times = Vec::with_capacity(100);
    let mut last = Value::Null;
    for _ in 0..100 {
        let c = b.call_op("snapshotInfo", args(&[("store", json!(scale))]))?;
        if !c.ok() {
            return Err(Failure::new(
                Exit::RunFailed,
                format!("snapshotInfo: {}", c.envelope["error"]),
            ));
        }
        times.push(c.wall_ms);
        last = c.result().clone();
    }
    let current = last["currentGeneration"].as_u64();
    Ok(json!({
        "heldGeneration": held,
        "currentGeneration": current,
        "clientIsCurrent": held.is_some() && held == current,
        "assetIsStale": last["assetIsStale"],
        "snapshotInfo": summary(&times),
        "rule": "a client compares the generation it holds with snapshotInfo.currentGeneration",
    }))
}

/// The asset route must refuse a request that carries no Stash login (R6).
fn asset_auth(b: &Bench) -> Result<Value, Failure> {
    let c = b.client()?;
    let mut rows = Vec::new();
    let mut all_refused = true;
    for file in [SNAPSHOT_FILE, MANIFEST_FILE] {
        let with = c.plugin_asset(PLUGIN_ID, file)?.status;
        let without = c.plugin_asset_without_credentials(PLUGIN_ID, file)?.status;
        let refused = without != 200;
        all_refused &= refused && with == 200;
        rows.push(
            json!({ "file": file, "withLogin": with, "withoutLogin": without, "refused": refused }),
        );
    }
    Ok(json!({ "requests": rows, "onlyWithLogin": all_refused }))
}
