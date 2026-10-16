//! The result envelope every operation returns inside Stash's `output` (research R1).

use engine::{ErrorCode, CONTRACT_VERSION};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub ok: bool,
    pub op: String,
    pub contract_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Envelope {
    pub fn ok(op: &str, result: Value) -> Envelope {
        Envelope {
            ok: true,
            op: op.into(),
            contract_version: CONTRACT_VERSION,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(op: &str, code: ErrorCode, message: &str, details: Option<Value>) -> Envelope {
        Envelope {
            ok: false,
            op: op.into(),
            contract_version: CONTRACT_VERSION,
            result: None,
            error: Some(ErrorBody {
                code,
                message: message.into(),
                details,
            }),
        }
    }
}

/// Stash's raw-interface output. `error` stays null whenever an envelope exists, so the
/// structured error reaches the caller instead of Stash's flattened string.
pub fn stash_output(env: Envelope) -> Value {
    serde_json::json!({ "error": null, "output": env })
}
