//! `synth` and `build`: build time and stored size (spec FR-011, tasks T022).
//!
//! `synth --scale S` builds the synthetic graph locally, straight into the spike plugin's store
//! folder (`<plugin>/store/<S>/`), so `call-cost` and `snapshot` can query it through Stash.
//! `build --from-stash` asks the spike plugin to fetch the seeded library and build from it.

use super::{args, summary, Bench, Exit, Failure, Outcome};
use crate::measure::{Kind, Library, Measurement, Question, Server};
use crate::store::{self, LockMode};
use crate::synth::{self, Spec};
use serde_json::{json, Value};
use std::fs;
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// The library counts recorded next to a synthetic store, written by `synth`.
pub fn store_spec(dir: &std::path::Path) -> Option<Library> {
    serde_json::from_str(&fs::read_to_string(dir.join("spec.json")).ok()?).ok()
}

pub fn synth_command(b: &mut Bench) -> Outcome {
    let scale = b.global.scale.clone();
    let scenes = synth::parse_scale(&scale).expect("validated by the parser");
    let runs = b.global.runs.unwrap_or(3);
    let root = b.store_root()?;
    fs::create_dir_all(&root).map_err(|e| Failure::new(Exit::RunFailed, e.to_string()))?;
    let spec = Spec::new(scenes);
    let io = |e: std::io::Error| Failure::new(Exit::RunFailed, e.to_string());

    let (mut totals, mut loads, mut publishes) = (vec![], vec![], vec![]);
    let (mut triples, mut work_bytes, mut snap_bytes) = (0u64, 0u64, 0u64);
    for run in 0..runs {
        let tmp = root.join(format!("{scale}.build"));
        let _ = fs::remove_dir_all(&tmp);
        let t = Instant::now();
        let w = store::open_writer(&tmp, LockMode::Fail)
            .map_err(|e| Failure::new(Exit::RunFailed, e.to_string()))?;
        triples = synth::load(w.store(), spec).map_err(|e| Failure::new(Exit::RunFailed, e))?;
        let load_ms = ms(t);
        let t2 = Instant::now();
        store::publish(&w).map_err(|e| Failure::new(Exit::RunFailed, e.to_string()))?;
        let publish_ms = ms(t2);
        drop(w);
        totals.push(load_ms + publish_ms);
        loads.push(load_ms);
        publishes.push(publish_ms);
        work_bytes = store::disk_size(&store::work_dir(&tmp)).map_err(io)?;
        snap_bytes = store::disk_size(&store::snapshots_dir(&tmp)).map_err(io)?;
        b.say(format!(
            "synth {scale} run {}/{runs}: {triples} triples in {:.0} ms",
            run + 1,
            load_ms + publish_ms
        ));
        if run + 1 == runs {
            let dest = root.join(&scale);
            let _ = fs::remove_dir_all(&dest);
            fs::rename(&tmp, &dest).map_err(io)?;
            let lib = Library::synthetic(
                scenes,
                spec.images(),
                spec.performers(),
                spec.tags(),
                triples,
            );
            fs::write(
                dest.join("spec.json"),
                serde_json::to_string_pretty(&lib).unwrap(),
            )
            .map_err(io)?;
        } else {
            let _ = fs::remove_dir_all(&tmp);
        }
    }

    let mut m = Measurement::new(
        &format!("build-synth-{scale}"),
        Question::Build,
        Kind::Build,
        &b.command_line(),
        Server::local(),
        Library::synthetic(
            scenes,
            spec.images(),
            spec.performers(),
            spec.tags(),
            triples,
        ),
    )
    .with_samples(totals.clone());
    m.variant = "synth".into();
    m.parts.insert("load".into(), json!(loads));
    m.parts.insert("publish".into(), json!(publishes));
    let median = m.median;
    m.extra.insert("triples".into(), json!(triples));
    m.extra.insert("workBytes".into(), json!(work_bytes));
    m.extra.insert("snapshotBytes".into(), json!(snap_bytes));
    m.extra.insert(
        "snapshotNote".into(),
        json!("snapshot size counts hard-linked files again; on the same filesystem they share blocks with work/"),
    );
    m.extra
        .insert("msPerScene".into(), json!(median / scenes as f64));
    m.extra.insert(
        "bytesPerScene".into(),
        json!(work_bytes as f64 / scenes as f64),
    );
    b.record(&m)
}

pub fn build_command(b: &mut Bench) -> Outcome {
    if !b.flag("--from-stash") {
        return Err(Failure::new(Exit::Usage, "build needs --from-stash"));
    }
    let runs = b.global.runs.unwrap_or(3);
    let stats = b.client()?.stats()?;
    let (mut totals, mut fetches, mut builds) = (vec![], vec![], vec![]);
    let mut last = Value::Null;
    for run in 0..runs {
        let call = b.call_op("buildFromStash", args(&[("store", json!("stash"))]))?;
        if !call.ok() {
            return Err(Failure::new(
                Exit::RunFailed,
                format!("buildFromStash: {}", call.envelope["error"]),
            ));
        }
        let r = call.result().clone();
        totals.push(r["timings"]["total"].as_f64().unwrap_or(0.0));
        fetches.push(r["timings"]["fetch"].as_f64().unwrap_or(0.0));
        builds.push(r["timings"]["build"].as_f64().unwrap_or(0.0));
        b.say(format!(
            "build --from-stash run {}/{runs}: {} items, {} triples, {:.0} ms",
            run + 1,
            r["items"],
            r["triples"],
            totals[run]
        ));
        last = r;
    }
    let items = last["items"].as_u64().unwrap_or(0).max(1);
    let mut m = Measurement::new(
        "build-stash-test",
        Question::Build,
        Kind::Build,
        &b.command_line(),
        b.server.clone(),
        Library::test(
            stats.scene_count as u64,
            stats.image_count as u64,
            stats.performer_count as u64,
            stats.tag_count as u64,
            last["triples"].as_u64().unwrap_or(0),
        ),
    )
    .with_samples(totals);
    m.variant = "stash".into();
    m.parts.insert("fetch".into(), json!(fetches));
    m.parts.insert("build".into(), json!(builds));
    m.extra.insert("fetchSummary".into(), summary(&fetches));
    m.extra.insert("buildSummary".into(), summary(&builds));
    m.extra
        .insert("semanticItems".into(), last["counts"].clone());
    m.extra.insert("requests".into(), last["requests"].clone());
    let median = m.median;
    m.extra
        .insert("msPerItem".into(), json!(median / items as f64));
    b.record(&m)
}
