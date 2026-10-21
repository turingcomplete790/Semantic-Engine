//! `applyEdit`: an edit that publishes a new generation and writes a change file (delta) beside
//! it, so a client can move from generation `from` to `to` without a full snapshot (tasks T050).
//!
//! Arguments: `store` (folder under `store/`, default `test`), `items` (scenes to edit, default
//! 1), and `seed` (picks which scenes). Each edited scene gets a different role tag on every role
//! assignment. The instance IRI includes the tag, so the edit removes the old instance and adds a
//! new one, as a real role change does.
//!
//! The change file is `store/<name>/deltas/<to>.nq.gz`: one N-Quads line per changed quad,
//! prefixed `D ` (removed) or `A ` (added). `deltas/<to>.json` records `from`, `to`, and counts.
//! A client applies deltas in generation order.

use super::{fail, Ctx};
use crate::store::{self, LockMode, StoreError};
use crate::synth::{self, SERVER_ID, VOCAB};
use engine::ErrorCode;
use flate2::write::GzEncoder;
use flate2::Compression;
use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::store::{StorageError, Store};
use plugin::envelope::Envelope;
use plugin::log::Logger;
use semantic_vocabulary::iri::ItemIris;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub fn deltas_dir(dir: &Path) -> PathBuf {
    dir.join("deltas")
}

pub fn delta_path(dir: &Path, to: u64) -> PathBuf {
    deltas_dir(dir).join(format!("{to}.nq.gz"))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn v(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{VOCAB}{local}"))
}

/// The last `:`-separated part of an item IRI: its Stash id.
fn id_of(iri: &str) -> &str {
    iri.rsplit(':').next().unwrap_or(iri)
}

/// Every quad that belongs to `scene`: the scene's own quads, each scene-scoped instance's
/// quads, and the performer links pointing at those instances. Two stores hold the same item when
/// these sets are equal.
pub fn item_quads(store: &Store, scene: &str) -> Result<HashSet<Quad>, StorageError> {
    let s = NamedNodeRef::new_unchecked(scene);
    let mut out = HashSet::new();
    for q in store.quads_for_pattern(Some(s.into()), None, None, None) {
        out.insert(q?);
    }
    let in_media = v("inMedia");
    let instances: Vec<Quad> = store
        .quads_for_pattern(None, Some(in_media.as_ref()), Some(s.into()), None)
        .collect::<Result<_, _>>()?;
    for i in instances {
        let inst = i.subject;
        for q in store.quads_for_pattern(Some(inst.as_ref()), None, None, None) {
            out.insert(q?);
        }
        let as_object: Term = inst.clone().into();
        for q in store.quads_for_pattern(None, None, Some(as_object.as_ref()), None) {
            out.insert(q?);
        }
    }
    Ok(out)
}

/// The edited version of `old`: every role instance moves to the next role tag.
fn edited(old: &HashSet<Quad>, scene: &str) -> HashSet<Quad> {
    let iris = ItemIris::new(SERVER_ID).expect("valid example server id");
    let (ty, role_class) = (
        NamedNode::new_unchecked(synth::RDF_TYPE),
        Term::from(v("RoleInstance")),
    );
    let roles: Vec<_> = old
        .iter()
        .filter(|q| q.predicate == ty && q.object == role_class)
        .map(|q| q.subject.clone())
        .collect();
    let mut new = old.clone();
    for inst in roles {
        let inst_term: Term = inst.clone().into();
        let of = |pred: &str| {
            old.iter()
                .find(|q| q.subject == inst && q.predicate == v(pred))
                .map(|q| match &q.object {
                    Term::NamedNode(n) => n.as_str().to_string(),
                    other => other.to_string(),
                })
        };
        let (Some(perf), Some(tag)) = (of("forPerformer"), of("ofTag")) else {
            continue;
        };
        new.retain(|q| q.subject != inst && q.object != inst_term);
        let first: u64 = synth::tag_id_for("Role", 0).parse().unwrap_or(0);
        let idx = id_of(&tag).parse::<u64>().unwrap_or(first) - first;
        let next = synth::tag_id_for("Role", idx + 1);
        let mut added = Vec::new();
        synth::instance(&mut added, &iris, scene, id_of(&perf), "role", &next, None);
        new.extend(added);
    }
    new
}

/// The scenes an edit of `items` with `seed` touches, out of `total`.
pub fn picked_scenes(total: u64, items: u64, seed: u64) -> Vec<u64> {
    let start = seed.wrapping_mul(items) % total.max(1);
    (0..items.min(total))
        .map(|k| (start + k) % total + 1)
        .collect()
}

/// How many scenes the store holds: from `spec.json` when the bench built it, else counted.
fn scene_count(dir: &Path, store: &Store) -> Result<u64, String> {
    if let Some(n) = fs::read_to_string(dir.join("spec.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["scenes"].as_u64())
    {
        return Ok(n);
    }
    let ty = NamedNode::new_unchecked(synth::RDF_TYPE);
    let scene: Term = v("Scene").into();
    Ok(store
        .quads_for_pattern(None, Some(ty.as_ref()), Some(scene.as_ref()), None)
        .count() as u64)
}

/// Writes one change file: `D ` lines, then `A ` lines.
fn write_delta(path: &Path, removed: &[Quad], added: &[Quad]) -> std::io::Result<u64> {
    fs::create_dir_all(path.parent().expect("deltas dir"))?;
    let tmp = path.with_extension("tmp");
    let mut gz = GzEncoder::new(fs::File::create(&tmp)?, Compression::default());
    for q in removed {
        writeln!(gz, "D {q} .")?;
    }
    for q in added {
        writeln!(gz, "A {q} .")?;
    }
    gz.finish()?.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(fs::metadata(path)?.len())
}

/// Reads a change file into its removed and added quads.
pub fn read_delta(path: &Path) -> Result<(Vec<Quad>, Vec<Quad>), String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    for line in BufReader::new(flate2::read::GzDecoder::new(file)).lines() {
        let line = line.map_err(|e| e.to_string())?;
        let (target, text) = match line.split_at_checked(2) {
            Some(("D ", t)) => (&mut removed, t),
            Some(("A ", t)) => (&mut added, t),
            _ if line.is_empty() => continue,
            _ => return Err(format!("bad change line: {line:?}")),
        };
        for q in RdfParser::from_format(RdfFormat::NQuads).for_slice(text.as_bytes()) {
            target.push(q.map_err(|e| e.to_string())?);
        }
    }
    Ok((removed, added))
}

/// Applies one change file to a client's copy, in one transaction. Returns (removed, added).
pub fn apply_delta(store: &Store, path: &Path) -> Result<(usize, usize), String> {
    let (removed, added) = read_delta(path)?;
    store
        .transaction(|mut t| {
            for q in &removed {
                t.remove(q)?;
            }
            for q in &added {
                t.insert(q)?;
            }
            Ok::<_, StorageError>(())
        })
        .map_err(|e| e.to_string())?;
    Ok((removed.len(), added.len()))
}

pub fn op(ctx: &Ctx, _: &mut Logger) -> Envelope {
    const OP: &str = "applyEdit";
    let name = ctx.arg_str("store").unwrap_or("test");
    let items = ctx.arg_u64("items").unwrap_or(1);
    if items == 0 {
        return fail(OP, ErrorCode::InvalidInput, "items must be at least 1");
    }
    let seed = ctx.arg_u64("seed").unwrap_or(1);
    let dir = ctx.store_dir.join(name);
    // A store that was never published is an error; never create an empty one here.
    let from = match store::current_generation(&dir) {
        Ok(g) => g,
        Err(e) => return fail(OP, ErrorCode::Internal, format!("store {name:?}: {e}")),
    };

    let t0 = Instant::now();
    let w = match store::open_writer(&dir, LockMode::Wait(Duration::from_secs(30))) {
        Ok(w) => w,
        Err(e @ (StoreError::Busy | StoreError::LockTimeout)) => {
            return Envelope::error(
                OP,
                ErrorCode::Internal,
                &e.to_string(),
                Some(json!({ "busy": true })),
            )
        }
        Err(e) => return fail(OP, ErrorCode::Internal, format!("store {name:?}: {e}")),
    };
    let lock_ms = ms(t0.elapsed());

    let t1 = Instant::now();
    let iris = ItemIris::new(SERVER_ID).expect("valid example server id");
    let total = match scene_count(&dir, w.store()) {
        Ok(n) if n > 0 => n,
        Ok(_) => {
            return fail(
                OP,
                ErrorCode::Internal,
                format!("store {name:?} has no scenes"),
            )
        }
        Err(e) => return fail(OP, ErrorCode::Internal, e),
    };
    let scenes = picked_scenes(total, items, seed);
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    for s in &scenes {
        let scene = iris.scene(&s.to_string());
        let old = match item_quads(w.store(), &scene) {
            Ok(q) => q,
            Err(e) => return fail(OP, ErrorCode::Internal, format!("reading {scene}: {e}")),
        };
        let new = edited(&old, &scene);
        removed.extend(old.difference(&new).cloned());
        added.extend(new.difference(&old).cloned());
    }
    let read_ms = ms(t1.elapsed());

    let t2 = Instant::now();
    let applied = w.store().transaction(|mut t| {
        for q in &removed {
            t.remove(q)?;
        }
        for q in &added {
            t.insert(q)?;
        }
        Ok::<_, StorageError>(())
    });
    if let Err(e) = applied {
        return fail(OP, ErrorCode::Internal, format!("write: {e}"));
    }
    let write_ms = ms(t2.elapsed());

    // The change file is written before the generation is published, so a reader that sees
    // generation `to` can always find its delta.
    let t3 = Instant::now();
    let to = match store::next_generation(&w) {
        Ok(g) => g,
        Err(e) => return fail(OP, ErrorCode::Internal, format!("next generation: {e}")),
    };
    let bytes = match write_delta(&delta_path(&dir, to), &removed, &added) {
        Ok(b) => b,
        Err(e) => return fail(OP, ErrorCode::Internal, format!("delta: {e}")),
    };
    let info = json!({ "from": from, "to": to, "removed": removed.len(), "added": added.len(), "bytes": bytes });
    if let Err(e) = fs::write(
        deltas_dir(&dir).join(format!("{to}.json")),
        info.to_string(),
    ) {
        return fail(OP, ErrorCode::Internal, format!("delta info: {e}"));
    }
    let delta_ms = ms(t3.elapsed());

    let t4 = Instant::now();
    match store::publish(&w) {
        Ok(g) if g == to => {}
        Ok(g) => {
            return fail(
                OP,
                ErrorCode::Internal,
                format!("published {g}, expected {to}"),
            )
        }
        Err(e) => return fail(OP, ErrorCode::Internal, format!("publish: {e}")),
    }
    let publish_ms = ms(t4.elapsed());
    drop(w);

    let mut result = info;
    result["scenes"] = json!(scenes);
    result["timings"] = json!({
        "lock": lock_ms,
        "readOld": read_ms,
        "write": write_ms,
        "publish": publish_ms,
        "delta": delta_ms,
        "total": ms(t0.elapsed()),
    });
    Envelope::ok(OP, result)
}
