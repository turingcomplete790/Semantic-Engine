//! `write`: one write under a sharing option (research R2).
//!
//! Arguments: `store` (folder under `store/`), `option` (`A` live store, no publish; `B` write
//! `work/` then publish a generation; `C` like A, every call serialised on the lock), `scale`
//! (scenes to generate), `seed` (varies the content), `lockMode` (`wait`, the default, or
//! `fail`), and `waitMs` (default 30000).
//!
//! Every write puts a start marker first and an end marker last (`query::MARK`), so readers can
//! see whether they read a write part-way.

use super::query::{markers, ENDED, MARK, STARTED};
use super::{fail, Ctx};
use crate::store::{self, LockMode, StoreError};
use crate::synth::{self, Spec};
use engine::ErrorCode;
use oxigraph::model::{GraphName, Literal, NamedNode, Quad};
use plugin::envelope::Envelope;
use plugin::log::Logger;
use serde_json::json;
use std::time::{Duration, Instant};

fn marker(pred: &str, generation: i64) -> Quad {
    Quad::new(
        NamedNode::new_unchecked(MARK),
        NamedNode::new_unchecked(pred),
        Literal::from(generation),
        GraphName::DefaultGraph,
    )
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

pub fn op(ctx: &Ctx, _: &mut Logger) -> Envelope {
    let name = ctx.arg_str("store").unwrap_or("test");
    let option = ctx.arg_str("option").unwrap_or("B");
    if !matches!(option, "A" | "B" | "C") {
        return fail(
            "write",
            ErrorCode::InvalidInput,
            format!("option must be A, B, or C, got {option:?}"),
        );
    }
    let Some(scenes) = ctx
        .arg_str("scale")
        .and_then(synth::parse_scale)
        .or_else(|| ctx.arg_u64("scale"))
    else {
        return fail(
            "write",
            ErrorCode::InvalidInput,
            "scale is required (test, 50k, ... or a scene count)",
        );
    };
    let seed = ctx.arg_u64("seed").unwrap_or(1);
    let wait = Duration::from_millis(ctx.arg_u64("waitMs").unwrap_or(30_000));
    let mode = match ctx.arg_str("lockMode").unwrap_or("wait") {
        "fail" => LockMode::Fail,
        _ => LockMode::Wait(wait),
    };
    let dir = ctx.store_dir.join(name);

    let t0 = Instant::now();
    let w = match store::open_writer(&dir, mode) {
        Ok(w) => w,
        Err(StoreError::Busy) => {
            return Envelope::error(
                "write",
                ErrorCode::Internal,
                "busy: another writer holds the lock",
                Some(json!({ "busy": true, "waitedMs": ms(t0.elapsed()) })),
            )
        }
        Err(StoreError::LockTimeout) => {
            return Envelope::error(
                "write",
                ErrorCode::Internal,
                "timed out waiting for the writer lock",
                Some(json!({ "busy": true, "waitedMs": ms(t0.elapsed()) })),
            )
        }
        Err(e) => return fail("write", ErrorCode::Internal, e),
    };
    let lock_ms = ms(t0.elapsed());

    let t1 = Instant::now();
    let (started, _) = markers(w.store());
    let generation = started + 1;
    let spec = Spec { scenes, seed };
    let body = std::iter::once(marker(STARTED, generation))
        .chain(synth::quads(spec))
        .chain(std::iter::once(marker(ENDED, generation)));
    let mut n = 0u64;
    let counted = body.inspect(|_| n += 1);
    if let Err(e) = w.store().bulk_loader().load_quads(counted) {
        return fail("write", ErrorCode::Internal, format!("bulk load: {e}"));
    }
    let write_ms = ms(t1.elapsed());

    let t2 = Instant::now();
    if let Err(e) = w.store().flush() {
        return fail("write", ErrorCode::Internal, format!("flush: {e}"));
    }
    let flush_ms = ms(t2.elapsed());

    let t3 = Instant::now();
    let published = if option == "B" {
        match store::publish(&w) {
            Ok(g) => Some(g),
            Err(e) => return fail("write", ErrorCode::Internal, format!("publish: {e}")),
        }
    } else {
        None
    };
    let publish_ms = ms(t3.elapsed());
    drop(w);

    Envelope::ok(
        "write",
        json!({
            "option": option,
            "store": name,
            "scenes": scenes,
            "triples": n,
            "writeGeneration": generation,
            "generation": published,
            "timings": { "lock": lock_ms, "write": write_ms, "flush": flush_ms, "publish": publish_ms, "total": ms(t0.elapsed()) },
        }),
    )
}
