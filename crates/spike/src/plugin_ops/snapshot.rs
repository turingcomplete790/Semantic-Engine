//! `exportSnapshot` and `snapshotInfo`: deliver a whole published generation to a client
//! (research R6, tasks T037).
//!
//! `exportSnapshot` arguments: `store` (folder under `store/`, default `test`) and `to`:
//! - `output` (default): `{ manifest, data }`, with `data` the gzip-compressed N-Quads in base64.
//! - `asset`: writes `store/assets/snapshot.nq.gz` and `store/assets/manifest.json`, which Stash
//!   serves at `/plugin/semantic-engine-spike/assets/`, and returns `{ manifest, path }`.
//!
//! `snapshotInfo` returns the asset manifest, plus the store's current generation so a client
//! can tell whether the file it holds, or the file on offer, is behind. The asset is stale when
//! it was exported from another store or from an older generation.

use super::{fail, Ctx};
use crate::measure::rfc3339_from_secs;
use crate::store::{self, StoreError};
use base64::Engine as _;
use engine::ErrorCode;
use flate2::write::GzEncoder;
use flate2::Compression;
use oxigraph::io::{RdfFormat, RdfSerializer};
use oxigraph::store::Store;
use plugin::envelope::Envelope;
use plugin::log::Logger;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant, UNIX_EPOCH};

pub const SNAPSHOT_FILE: &str = "snapshot.nq.gz";
pub const MANIFEST_FILE: &str = "manifest.json";

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Writes every quad of `store` as gzip-compressed N-Quads. Returns the quad count.
pub fn dump_gz<W: Write>(store: &Store, out: W) -> Result<(u64, W), String> {
    let gz = GzEncoder::new(out, Compression::default());
    let mut ser = RdfSerializer::from_format(RdfFormat::NQuads).for_writer(gz);
    let mut n = 0u64;
    for q in store.iter() {
        let q = q.map_err(|e| e.to_string())?;
        ser.serialize_quad(&q).map_err(|e| e.to_string())?;
        n += 1;
    }
    let gz = ser.finish().map_err(|e| e.to_string())?;
    let out = gz.finish().map_err(|e| e.to_string())?;
    Ok((n, out))
}

/// The manifest for generation `generation` of store `name`.
pub fn manifest(dir: &Path, name: &str, generation: u64, quads: u64, bytes: u64) -> Value {
    let built = fs::metadata(store::snapshots_dir(dir).join(generation.to_string()))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| rfc3339_from_secs(d.as_secs()));
    let v = engine::versions();
    json!({
        "store": name,
        "generation": generation,
        "builtAt": built,
        "quads": quads,
        "bytes": bytes,
        "format": "application/n-quads+gzip",
        "vocabularyVersion": v.vocabulary_version,
        "engineVersion": v.engine_version,
    })
}

/// Writes `bytes` to `path` through a temporary file and a rename, so Stash never serves a
/// half-written file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

pub fn export_op(ctx: &Ctx, _: &mut Logger) -> Envelope {
    const OP: &str = "exportSnapshot";
    let to = ctx.arg_str("to").unwrap_or("output");
    if to != "output" && to != "asset" {
        return fail(
            OP,
            ErrorCode::InvalidInput,
            format!("to must be output or asset, got {to:?}"),
        );
    }
    let t0 = Instant::now();
    let name = ctx.arg_str("store").unwrap_or("test").to_string();
    let dir = ctx.store_dir.join(&name);
    let (generation, store) = match store::open_current_read_only(&dir) {
        Ok(x) => x,
        // No snapshot is an error, never an empty file (Principle IV).
        Err(e) => return fail(OP, ErrorCode::Internal, format!("store {name:?}: {e}")),
    };
    let open_ms = ms(t0.elapsed());

    let t1 = Instant::now();
    let (quads, data) = match dump_gz(&store, Vec::new()) {
        Ok(x) => x,
        Err(e) => return fail(OP, ErrorCode::Internal, format!("dump: {e}")),
    };
    let dump_ms = ms(t1.elapsed());
    let m = manifest(&dir, &name, generation, quads, data.len() as u64);

    let t2 = Instant::now();
    let mut result = if to == "output" {
        json!({
            "manifest": m,
            "data": base64::engine::general_purpose::STANDARD.encode(&data),
        })
    } else {
        let assets = store::assets_dir(&ctx.store_dir);
        let written = fs::create_dir_all(&assets)
            .and_then(|_| write_atomic(&assets.join(SNAPSHOT_FILE), &data))
            .and_then(|_| {
                write_atomic(
                    &assets.join(MANIFEST_FILE),
                    serde_json::to_string_pretty(&m)
                        .unwrap_or_default()
                        .as_bytes(),
                )
            });
        if let Err(e) = written {
            return fail(OP, ErrorCode::Internal, format!("writing assets: {e}"));
        }
        json!({ "manifest": m, "path": SNAPSHOT_FILE })
    };
    result["timings"] = json!({
        "storeOpen": open_ms,
        "dump": dump_ms,
        "deliver": ms(t2.elapsed()),
        "total": ms(t0.elapsed()),
    });
    Envelope::ok(OP, result)
}

/// Arguments: `store`, defaulting to the store the asset was exported from, else `test`. The
/// manifest is `null` when no asset has been exported; that is an error only when the store has
/// no published generation either.
pub fn info_op(ctx: &Ctx, _: &mut Logger) -> Envelope {
    const OP: &str = "snapshotInfo";
    let path = store::assets_dir(&ctx.store_dir).join(MANIFEST_FILE);
    let m: Value = match fs::read_to_string(&path) {
        Ok(t) => match serde_json::from_str(&t) {
            Ok(v) => v,
            Err(e) => return fail(OP, ErrorCode::Internal, format!("{MANIFEST_FILE}: {e}")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(e) => return fail(OP, ErrorCode::Internal, format!("{MANIFEST_FILE}: {e}")),
    };
    let name = ctx
        .arg_str("store")
        .or_else(|| m["store"].as_str())
        .unwrap_or("test")
        .to_string();
    let current = match store::current_generation(&ctx.store_dir.join(&name)) {
        Ok(g) => Some(g),
        Err(StoreError::NoCurrent) if !m.is_null() => None,
        Err(StoreError::NoCurrent) => {
            return fail(
                OP,
                ErrorCode::Internal,
                format!("store {name:?} has no published snapshot and no asset was exported"),
            )
        }
        Err(e) => return fail(OP, ErrorCode::Internal, format!("store {name:?}: {e}")),
    };
    let same_store = m["store"].as_str() == Some(name.as_str());
    let stale = !m.is_null()
        && (!same_store || current.is_some_and(|c| m["generation"].as_u64() != Some(c)));
    Envelope::ok(
        OP,
        json!({ "manifest": m, "currentGeneration": current, "assetIsStale": stale }),
    )
}
