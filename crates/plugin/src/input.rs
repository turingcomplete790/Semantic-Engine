//! Stash's raw-interface input (research R1, data-model "Plugin input").

use serde::Deserialize;
use stash_adapter::ServerConnection;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginInput {
    pub server_connection: ServerConnection,
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
}

impl PluginInput {
    /// The requested operation, from `args.op`.
    pub fn op(&self) -> Option<&str> {
        self.args.get("op").and_then(|v| v.as_str())
    }
}

pub fn parse(bytes: &[u8]) -> Result<PluginInput, String> {
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Err("stdin was empty".into());
    }
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
