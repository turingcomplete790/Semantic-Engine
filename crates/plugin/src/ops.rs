//! Operation and task registry (contracts/operations.md).

use crate::envelope::Envelope;
use crate::input::PluginInput;
use crate::log::Logger;
use engine::server_id::{self, SettingsStore, StoreError};
use engine::ErrorCode;
use serde_json::{json, Map, Value};

/// This plugin's id in Stash (the manifest file name).
pub const PLUGIN_ID: &str = "semantic-engine";

type Handler = fn(&PluginInput, &mut Logger) -> Envelope;

/// Every operation and task the plugin answers, by `args.op` name.
pub const REGISTRY: &[(&str, Handler)] = &[
    ("ping", ping),
    ("status", status),
    ("getServerId", get_server_id),
    ("generateServerId", generate_server_id),
    ("relations", crate::relations::relations),
    ("explainRelation", crate::relations::explain_relation),
    ("recommendations", crate::relations::recommendations),
    ("recordChange", crate::relations::record_change),
    ("applyRelations", crate::relations::apply_relations),
    ("reapplyRelations", crate::relations::reapply_relations),
    ("buildGraph", crate::build::build_graph),
    ("updateGraph", crate::build::update_graph),
];

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

/// This plugin's settings in Stash, through the adapter.
pub(crate) struct StashSettings(pub(crate) stash_adapter::Client);

fn store_error(e: stash_adapter::Error) -> StoreError {
    StoreError {
        unreachable: e.is_transient(),
        message: e.to_string(),
    }
}

impl SettingsStore for StashSettings {
    fn read(&self) -> Result<Map<String, Value>, StoreError> {
        self.0.plugin_settings(PLUGIN_ID).map_err(store_error)
    }
    fn write(&self, settings: Map<String, Value>) -> Result<Map<String, Value>, StoreError> {
        self.0
            .write_plugin_settings(PLUGIN_ID, settings)
            .map_err(store_error)
    }
}

pub(crate) fn settings_store(
    input: &PluginInput,
) -> Result<StashSettings, server_id::ServerIdError> {
    stash_adapter::Client::from_server_connection(&input.server_connection)
        .map(StashSettings)
        .map_err(|e| server_id::ServerIdError::Store(store_error(e)))
}

fn get_server_id(input: &PluginInput, _: &mut Logger) -> Envelope {
    match settings_store(input).and_then(|s| server_id::read_server_id(&s)) {
        Ok(id) => Envelope::ok("getServerId", json!({ "serverId": id })),
        Err(e) => Envelope::error("getServerId", e.code(), &e.message(), None),
    }
}

/// The "Generate server identifier" task. Stash does not return a task's output to the
/// caller, so the result is also logged (contracts/operations.md).
fn generate_server_id(input: &PluginInput, logger: &mut Logger) -> Envelope {
    match settings_store(input).and_then(|s| server_id::ensure_server_id(&s)) {
        Ok(outcome) => {
            logger.info(&format!("serverId={} ({})", outcome.id(), outcome.label()));
            Envelope::ok(
                "generateServerId",
                json!({ "serverId": outcome.id(), "outcome": outcome.label() }),
            )
        }
        Err(e) => {
            logger.error(&format!("{}: {}", e.code().as_str(), e.message()));
            Envelope::error("generateServerId", e.code(), &e.message(), None)
        }
    }
}

fn status(input: &PluginInput, logger: &mut Logger) -> Envelope {
    let mut result = local_status();
    let client = stash_adapter::Client::from_server_connection(&input.server_connection);
    let server = match client
        .as_ref()
        .map_err(|e| e.to_string())
        .and_then(|c| c.stash_version().map_err(|e| e.to_string()))
    {
        Ok(v) => v.unwrap_or_default(),
        Err(e) => {
            logger.error(&format!("status: {e}"));
            return Envelope::error("status", ErrorCode::StashUnreachable, &e, Some(result));
        }
    };
    result["stashVersion"] = json!(server);
    if let Ok(c) = client {
        // A missing or invalid identifier is reported as null here; getServerId explains why.
        result["serverId"] = server_id::read_server_id(&StashSettings(c))
            .map(Value::String)
            .unwrap_or(Value::Null);
    }
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
