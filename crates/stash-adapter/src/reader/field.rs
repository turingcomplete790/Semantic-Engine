//! Parsing one semantic `custom_fields` value (spec 005 FR-013, FR-014; data-model "Field").
//!
//! The plugin stores most values as JSON text; some arrive as JSON values. Both are accepted. A
//! value that does not parse, or parses to the wrong top-level kind, is kept as `Unreadable` with
//! its raw form, so the record and the read continue.

use super::keys::Shape;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The parsed value of one semantic key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    Value(Value),
    Unreadable { raw: Value, reason: String },
}

impl Field {
    pub fn value(&self) -> Option<&Value> {
        match self {
            Field::Value(v) => Some(v),
            Field::Unreadable { .. } => None,
        }
    }
}

/// True when a type flag is set: `1` or `true`, as a value or as text (the plugin's own rule,
/// for example `isActionTagValue` in its `actions.js`; spec 004).
pub fn flag_set(value: &Value) -> bool {
    match value {
        Value::Number(n) => n.as_i64() == Some(1),
        Value::String(s) => matches!(s.trim(), "1" | "true"),
        Value::Bool(b) => *b,
        _ => false,
    }
}

fn unreadable(raw: &Value, reason: impl Into<String>) -> Field {
    Field::Unreadable {
        raw: raw.clone(),
        reason: reason.into(),
    }
}

/// Parses `raw` as a value of `shape`. Flags are not fields: use [`flag_set`].
pub fn parse_field(shape: Shape, raw: &Value) -> Field {
    match shape {
        Shape::Flag => Field::Value(Value::Bool(flag_set(raw))),
        Shape::Id => match raw {
            Value::String(s) if !s.trim().is_empty() => {
                Field::Value(Value::String(s.trim().into()))
            }
            Value::Number(n) if n.as_u64().is_some() => Field::Value(Value::String(n.to_string())),
            _ => unreadable(raw, "expected a tag id"),
        },
        Shape::List | Shape::Object => {
            let parsed = match raw {
                Value::String(s) => match serde_json::from_str::<Value>(s) {
                    Ok(v) => v,
                    Err(e) => return unreadable(raw, format!("not valid JSON: {e}")),
                },
                other => other.clone(),
            };
            match (shape, &parsed) {
                (Shape::List, Value::Array(_)) | (Shape::Object, Value::Object(_)) => {
                    Field::Value(parsed)
                }
                (Shape::List, _) => unreadable(raw, "expected a list"),
                _ => unreadable(raw, "expected an object"),
            }
        }
    }
}

/// A node position saved by the plugin's Hierarchy tab (display data, FR-002a).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GraphPosition {
    pub x: f64,
    pub y: f64,
}

/// The saved position, or `None` when it is missing or malformed: "not placed", as the plugin's
/// `parseArchetypeGraphPosition` treats it. Never an error.
pub fn graph_position(raw: &Value) -> Option<GraphPosition> {
    let v = match parse_field(Shape::Object, raw) {
        Field::Value(v) => v,
        Field::Unreadable { .. } => return None,
    };
    Some(GraphPosition {
        x: v.get("x")?.as_f64()?,
        y: v.get("y")?.as_f64()?,
    })
}
