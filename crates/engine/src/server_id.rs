//! The server identifier (data-model "Server identifier", contracts/operations.md).
//!
//! A lower-case hyphenated UUID stored once per server in Stash, as this plugin's `serverId`
//! setting. The logic here does no I/O itself: it works against a [`SettingsStore`], which the
//! plugin backs with the Stash adapter.

use serde_json::{Map, Value};

/// The plugin setting that holds the identifier.
pub const SETTING_KEY: &str = "serverId";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError {
    /// True when Stash could not be reached at all.
    pub unreachable: bool,
    pub message: String,
}

/// This plugin's settings in Stash. `write` replaces the whole map and returns what was stored.
pub trait SettingsStore {
    fn read(&self) -> Result<Map<String, Value>, StoreError>;
    fn write(&self, settings: Map<String, Value>) -> Result<Map<String, Value>, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Generated(String),
    Existing(String),
}

impl Outcome {
    pub fn id(&self) -> &str {
        match self {
            Outcome::Generated(id) | Outcome::Existing(id) => id,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Generated(_) => "generated",
            Outcome::Existing(_) => "existing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerIdError {
    NotGenerated,
    Invalid(String),
    Store(StoreError),
}

impl ServerIdError {
    pub fn code(&self) -> crate::ErrorCode {
        use crate::ErrorCode;
        match self {
            ServerIdError::NotGenerated => ErrorCode::ServerIdNotGenerated,
            ServerIdError::Invalid(_) => ErrorCode::ServerIdInvalid,
            ServerIdError::Store(e) if e.unreachable => ErrorCode::StashUnreachable,
            ServerIdError::Store(_) => ErrorCode::Internal,
        }
    }

    pub fn message(&self) -> String {
        match self {
            ServerIdError::NotGenerated => {
                "no server identifier yet; run the \"Generate server identifier\" task".into()
            }
            ServerIdError::Invalid(v) => format!(
                "the stored server identifier {v:?} is not a lower-case hyphenated UUID; it was left unchanged"
            ),
            ServerIdError::Store(e) => e.message.clone(),
        }
    }
}

/// `^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`
pub fn is_valid_server_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().zip([8, 4, 4, 4, 12]).all(|(p, n)| {
            p.len() == n
                && p.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

/// The stored value: None when absent or empty, Some otherwise (valid or not).
fn stored(settings: &Map<String, Value>) -> Option<String> {
    match settings.get(SETTING_KEY) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(other) => Some(other.to_string()),
    }
}

/// Reads the identifier. Never generates one (FR-007).
pub fn read_server_id(store: &impl SettingsStore) -> Result<String, ServerIdError> {
    let settings = store.read().map_err(ServerIdError::Store)?;
    match stored(&settings) {
        None => Err(ServerIdError::NotGenerated),
        Some(id) if is_valid_server_id(&id) => Ok(id),
        Some(id) => Err(ServerIdError::Invalid(id)),
    }
}

/// Generates and stores the identifier if absent (FR-006). An existing valid value is kept; an
/// invalid one is reported and never overwritten. Other settings are preserved.
pub fn ensure_server_id(store: &impl SettingsStore) -> Result<Outcome, ServerIdError> {
    let mut settings = store.read().map_err(ServerIdError::Store)?;
    match stored(&settings) {
        Some(id) if is_valid_server_id(&id) => return Ok(Outcome::Existing(id)),
        Some(id) => return Err(ServerIdError::Invalid(id)),
        None => {}
    }
    let new_id = uuid::Uuid::new_v4().hyphenated().to_string();
    settings.insert(SETTING_KEY.to_string(), Value::String(new_id.clone()));
    let written = store.write(settings).map_err(ServerIdError::Store)?;
    // Report what Stash actually stored, in case another writer got there first.
    match stored(&written) {
        Some(id) if id == new_id => Ok(Outcome::Generated(id)),
        Some(id) if is_valid_server_id(&id) => Ok(Outcome::Existing(id)),
        Some(id) => Err(ServerIdError::Invalid(id)),
        None => Err(ServerIdError::Store(StoreError {
            unreachable: false,
            message: "Stash did not keep the server identifier after writing it".into(),
        })),
    }
}
