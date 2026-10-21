//! `delta --scale S`: incremental snapshot updates through change files (tasks T050).
//!
//! 1. The client copies the store's current generation (`store::copy_generation`).
//! 2. `applyEdit` runs through Stash with `items` 1, 10, 100, and 1000, `--runs` (default 3)
//!    times each, on the synthetic store. Nothing is written to Stash.
//! 3. The client applies every change file in generation order to its copy, timing each.
//! 4. The copy must then match the latest generation: the same quad count and the same quads for
//!    every edited scene.
//! 5. One full `exportSnapshot` at gzip level 1, to compare with the level-6 dump of T039.
//!
//! The record's samples are the single-item edits (wall time through Stash); each size's figures
//! are in `extra.perItems`.

use super::{args, summary, Bench, Exit, Failure, Outcome};
use crate::measure::{Kind, Measurement, Question};
use crate::plugin_ops::delta::{apply_delta, delta_path, item_quads};
use crate::store;
use crate::synth::SERVER_ID;
use oxigraph::store::Store;
use semantic_vocabulary::iri::ItemIris;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub const SIZES: [u64; 4] = [1, 10, 100, 1000];

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn run_failed(e: impl std::fmt::Display) -> Failure {
    Failure::new(Exit::RunFailed, e.to_string())
}

/// One `applyEdit` call as the harness saw it.
struct Edit {
    items: u64,
    to: u64,
    wall: f64,
    result: Value,
}

fn median_of(edits: &[&Edit], f: impl Fn(&Edit) -> f64) -> Value {
    summary(&edits.iter().map(|e| f(e)).collect::<Vec<_>>())["median"].clone()
}

pub fn command(b: &mut Bench) -> Outcome {
    let scale = b.global.scale.clone();
    let runs = b.global.runs.unwrap_or(3);
    let dir = b.store_root()?.join(&scale);
    let library = super::build::store_spec(&dir).ok_or_else(|| {
        Failure::new(
            Exit::Usage,
            format!("no synthetic store for scale {scale}; run `spike-bench synth --scale {scale}` first"),
        )
    })?;
    let start = store::current_generation(&dir).map_err(run_failed)?;

    // 1. The client's copy of the starting generation.
    let copy = dir.join("client-copy");
    let _ = std::fs::remove_dir_all(&copy);
    let t = Instant::now();
    store::copy_generation(&dir, start, &copy).map_err(run_failed)?;
    let copy_ms = ms(t);
    b.say(format!(
        "copied generation {start} for the client in {copy_ms:.0} ms"
    ));

    // 2. The edits.
    let mut edits: Vec<Edit> = Vec::new();
    let mut seed = 1u64;
    let mut expected_from = start;
    for items in SIZES {
        for _ in 0..runs {
            let call = b.call_op(
                "applyEdit",
                args(&[
                    ("store", json!(scale)),
                    ("items", json!(items)),
                    ("seed", json!(seed)),
                ]),
            )?;
            seed += 1;
            if !call.ok() {
                return Err(run_failed(format!("applyEdit: {}", call.envelope["error"])));
            }
            let r = call.result().clone();
            let (from, to) = (r["from"].as_u64(), r["to"].as_u64().unwrap_or(0));
            if from != Some(expected_from) {
                return Err(run_failed(format!(
                    "edit went from {from:?}, expected {expected_from}: another writer changed the store"
                )));
            }
            expected_from = to;
            b.say(format!(
                "items {items:>4}: {:.0} ms through Stash, delta {} bytes ({} removed, {} added)",
                call.wall_ms, r["bytes"], r["removed"], r["added"]
            ));
            edits.push(Edit {
                items,
                to,
                wall: call.wall_ms,
                result: r,
            });
        }
    }

    // 3. The client applies the chain.
    let client = Store::open(&copy).map_err(run_failed)?;
    let mut apply_ms = Vec::with_capacity(edits.len());
    for e in &edits {
        let t = Instant::now();
        apply_delta(&client, &delta_path(&dir, e.to)).map_err(run_failed)?;
        apply_ms.push(ms(t));
    }

    // 4. Does the copy match the latest generation?
    let (latest_gen, latest) = store::open_current_read_only(&dir).map_err(run_failed)?;
    let t = Instant::now();
    let (copy_len, latest_len) = (
        client.len().map_err(run_failed)?,
        latest.len().map_err(run_failed)?,
    );
    let count_ms = ms(t);
    let iris = ItemIris::new(SERVER_ID).expect("valid example server id");
    let mut touched: Vec<u64> = edits
        .iter()
        .flat_map(|e| e.result["scenes"].as_array().cloned().unwrap_or_default())
        .filter_map(|s| s.as_u64())
        .collect();
    touched.sort_unstable();
    touched.dedup();
    let mut differing = Vec::new();
    for s in &touched {
        let scene = iris.scene(&s.to_string());
        if item_quads(&client, &scene).map_err(run_failed)?
            != item_quads(&latest, &scene).map_err(run_failed)?
        {
            differing.push(*s);
        }
    }
    let matches = copy_len == latest_len && differing.is_empty();
    drop((client, latest));
    let _ = std::fs::remove_dir_all(&copy);

    // 5. A full dump at gzip level 1.
    let full = b.call_op(
        "exportSnapshot",
        args(&[
            ("store", json!(scale)),
            ("to", json!("asset")),
            ("level", json!(1)),
        ]),
    )?;
    let level1 = if full.ok() {
        let r = full.result();
        json!({
            "wallMs": full.wall_ms,
            "dumpMs": r["timings"]["dump"],
            "bytes": r["manifest"]["bytes"],
            "quads": r["manifest"]["quads"],
        })
    } else {
        json!({ "error": full.envelope["error"] })
    };
    let level6 = crate::measure::Measurement::load(
        &b.global.out.join(format!("snapshot-asset-{scale}.json")),
    )
    .ok()
    .map(|m| json!({ "dumpMedianMs": m.extra["partsSummary"]["pluginDump"]["median"], "bytes": m.extra["sizes"]["gzipBytes"], "from": format!("snapshot-asset-{scale}") }));

    // Old generations are no longer needed once the copy has been checked.
    let removed_gens = store::gc(&dir, Duration::ZERO).map_err(run_failed)?;

    let per_items: Vec<Value> = SIZES
        .iter()
        .map(|&items| {
            let es: Vec<&Edit> = edits.iter().filter(|e| e.items == items).collect();
            let idx: Vec<usize> = edits
                .iter()
                .enumerate()
                .filter(|(_, e)| e.items == items)
                .map(|(i, _)| i)
                .collect();
            let t = |name: &'static str| move |e: &Edit| e.result["timings"][name].as_f64().unwrap_or(0.0);
            let n = |name: &'static str| move |e: &Edit| e.result[name].as_f64().unwrap_or(0.0);
            let plain = |e: &Edit| {
                let x = &e.result["timings"];
                x["total"].as_f64().unwrap_or(0.0) - x["delta"].as_f64().unwrap_or(0.0)
            };
            json!({
                "items": items,
                "runs": es.len(),
                "wallMedianMs": median_of(&es, |e| e.wall),
                "plugin": {
                    "lock": median_of(&es, t("lock")),
                    "readOld": median_of(&es, t("readOld")),
                    "write": median_of(&es, t("write")),
                    "publish": median_of(&es, t("publish")),
                    "delta": median_of(&es, t("delta")),
                    "total": median_of(&es, t("total")),
                },
                "plainWriteMedianMs": median_of(&es, plain),
                "deltaShareOfPlainWrite": median_of(&es, |e| t("delta")(e) / plain(e).max(f64::MIN_POSITIVE)),
                "deltaBytesMedian": median_of(&es, n("bytes")),
                "removedMedian": median_of(&es, n("removed")),
                "addedMedian": median_of(&es, n("added")),
                "clientApplyMedianMs": summary(&idx.iter().map(|&i| apply_ms[i]).collect::<Vec<_>>())["median"],
            })
        })
        .collect();

    let ones: Vec<f64> = edits
        .iter()
        .filter(|e| e.items == 1)
        .map(|e| e.wall)
        .collect();
    let mut m = Measurement::new(
        &format!("snapshot-delta-{scale}"),
        Question::Snapshot,
        Kind::Build,
        &b.command_line(),
        b.server.clone(),
        library,
    )
    .with_samples(ones);
    m.variant = "delta".into();
    m.parts.insert("clientApply".into(), json!(apply_ms));
    m.extra.insert(
        "samplesAre".into(),
        json!("single-item edits, wall time through Stash"),
    );
    m.extra.insert("perItems".into(), Value::Array(per_items));
    m.extra.insert(
        "client".into(),
        json!({
            "copyMs": copy_ms,
            "startGeneration": start,
            "latestGeneration": latest_gen,
            "deltasApplied": edits.len(),
            "applyTotalMs": apply_ms.iter().sum::<f64>(),
        }),
    );
    m.extra.insert(
        "match".into(),
        json!({
            "matches": matches,
            "copyQuads": copy_len,
            "latestQuads": latest_len,
            "touchedScenes": touched.len(),
            "differingScenes": differing,
            "countMs": count_ms,
        }),
    );
    m.extra.insert("fullDumpLevel1".into(), level1);
    m.extra
        .insert("fullDumpLevel6".into(), level6.unwrap_or(Value::Null));
    m.extra
        .insert("generationsRemovedAfter".into(), json!(removed_gens.len()));
    if !matches {
        m.add_failure(
            0,
            "match",
            "the client's copy differs from the latest generation",
        );
    }
    b.record(&m)?;
    if matches {
        Ok(())
    } else {
        Err(run_failed(
            "the client's copy differs from the latest generation",
        ))
    }
}
