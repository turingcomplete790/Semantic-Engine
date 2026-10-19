//! `payload`: an answer of a requested size, to find the largest answer one call can return
//! (research R7, tasks T035).
//!
//! Argument: `bytes`, the approximate size of the `bindings` array once serialised. Each binding
//! is an item IRI and its label, the shape a Phase 3 query answer has.

use super::{fail, Ctx};
use crate::synth::SERVER_ID;
use engine::ErrorCode;
use plugin::envelope::Envelope;
use plugin::log::Logger;
use serde_json::{json, Value};
use std::time::Instant;

/// The largest size the op builds (1 GiB), so a typo cannot exhaust memory.
pub const MAX_BYTES: u64 = 1 << 30;

/// One realistic binding: an item IRI and its label.
pub fn binding(n: u64) -> Value {
    json!({
        "item": format!("urn:semantic-stash:{SERVER_ID}:scene:{n}"),
        "label": format!("Scene {n}: synthetic answer row"),
    })
}

/// Bindings whose serialised array is about `bytes` long (never more than one binding over).
pub fn bindings(bytes: u64) -> Vec<Value> {
    let mut out = Vec::new();
    // `[` and `]`, then each binding plus its comma.
    let mut size = 2u64;
    let mut n = 0u64;
    while size < bytes {
        let b = binding(n);
        size += b.to_string().len() as u64 + u64::from(n > 0);
        out.push(b);
        n += 1;
    }
    out
}

pub fn op(ctx: &Ctx, _: &mut Logger) -> Envelope {
    let Some(bytes) = ctx.arg_u64("bytes") else {
        return fail("payload", ErrorCode::InvalidInput, "bytes is required");
    };
    if bytes > MAX_BYTES {
        return fail(
            "payload",
            ErrorCode::InvalidInput,
            format!("bytes must be at most {MAX_BYTES}, got {bytes}"),
        );
    }
    let t = Instant::now();
    let rows = bindings(bytes);
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    Envelope::ok(
        "payload",
        json!({
            "requestedBytes": bytes,
            "count": rows.len(),
            "bindings": rows,
            "timings": { "build": build_ms },
        }),
    )
}
