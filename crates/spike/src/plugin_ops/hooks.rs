//! `recordHook`: append one line per hook call to `<PluginDir>/hooks.jsonl` and exit
//! (research R4). It does nothing else, so its own run time is the floor for any hook handler.

use super::{fail, Ctx};
use crate::measure::now_rfc3339;
use engine::ErrorCode;
use plugin::envelope::Envelope;
use plugin::log::Logger;
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const LOG: &str = "hooks.jsonl";

/// Milliseconds since the Unix epoch, as a float (the harness compares it with its own clock).
pub fn epoch_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

pub fn op(ctx: &Ctx, _: &mut Logger) -> Envelope {
    let start = Instant::now();
    let received_ms = epoch_ms();
    let hc = ctx
        .input
        .args
        .get("hookContext")
        .cloned()
        .unwrap_or(Value::Null);
    let input_bytes = hc.get("input").map(|i| i.to_string().len()).unwrap_or(0);
    let mut line = json!({
        "receivedAt": now_rfc3339(),
        "receivedMs": received_ms,
        "type": hc.get("type").cloned().unwrap_or(Value::Null),
        "id": hc.get("id").cloned().unwrap_or(Value::Null),
        "inputFields": hc.get("inputFields").cloned().unwrap_or(json!([])),
        "inputBytes": input_bytes,
    });
    line["handlerMs"] = json!(start.elapsed().as_secs_f64() * 1000.0);
    let text = format!("{line}\n");
    let written = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ctx.plugin_dir.join(LOG))
        .and_then(|mut f| f.write_all(text.as_bytes()));
    match written {
        Ok(()) => Envelope::ok("recordHook", json!({ "recorded": true })),
        Err(e) => fail("recordHook", ErrorCode::Internal, format!("{LOG}: {e}")),
    }
}
