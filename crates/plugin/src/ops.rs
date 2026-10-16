//! Operation and task registry (contracts/operations.md).

use crate::envelope::Envelope;
use crate::input::PluginInput;
use crate::log::Logger;
use engine::ErrorCode;
use serde_json::{json, Value};

type Handler = fn(&PluginInput, &mut Logger) -> Envelope;

/// Every operation and task the plugin answers, by `args.op` name.
pub const REGISTRY: &[(&str, Handler)] = &[("ping", ping), ("status", status)];

pub fn names() -> Vec<&'static str> {
    REGISTRY.iter().map(|(n, _)| *n).collect()
}

pub fn dispatch(input: &PluginInput, logger: &mut Logger) -> Envelope {
    let op = input.op().unwrap_or("");
    match REGISTRY.iter().find(|(n, _)| *n == op) {
        Some((_, handler)) => handler(input, logger),
        None => Envelope::error(
            op,
            ErrorCode::UnknownOperation,
            if op.is_empty() {
                "no operation given; set args.op"
            } else {
                "unknown operation"
            },
            Some(json!({ "known": names() })),
        ),
    }
}

fn ping(_: &PluginInput, _: &mut Logger) -> Envelope {
    Envelope::ok("ping", json!({ "pong": true }))
}

/// Local versions, available without Stash.
fn local_status() -> Value {
    let v = engine::versions();
    let mut out = serde_json::to_value(&v).expect("versions serialize");
    out["rocksdb"] = json!({ "version": v.rocksdb_version, "available": true });
    out["serverId"] = Value::Null;
    out
}

fn status(input: &PluginInput, logger: &mut Logger) -> Envelope {
    let mut result = local_status();
    let server = match stash_adapter::Client::from_server_connection(&input.server_connection)
        .and_then(|c| c.stash_version())
    {
        Ok(v) => v.unwrap_or_default(),
        Err(e) => {
            logger.error(&format!("status: {e}"));
            return Envelope::error(
                "status",
                ErrorCode::StashUnreachable,
                &e.to_string(),
                Some(result),
            );
        }
    };
    result["stashVersion"] = json!(server);
    if !engine::versions::stash_supported(&server) {
        return Envelope::error(
            "status",
            ErrorCode::StashTooOld,
            &format!(
                "Stash {server} is older than the minimum supported {}",
                engine::STASH_MINIMUM_VERSION
            ),
            Some(result),
        );
    }
    Envelope::ok("status", result)
}
