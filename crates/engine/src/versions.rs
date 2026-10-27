//! Versions reported by the `status` operation (data-model "Versions reported by status").

use serde::Serialize;

/// Version of the operation contract (contracts/operations.md).
pub const CONTRACT_VERSION: &str = "1.3.0";
/// Oldest Stash release this engine supports (FR-017). The adapter owns the value (spec 005).
pub const STASH_MINIMUM_VERSION: &str = stash_adapter::version::MINIMUM_STASH_VERSION;

pub use stash_adapter::version::{parse_stash_version, stash_supported};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    pub target: String,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Versions {
    pub engine_version: String,
    pub contract_version: String,
    pub vocabulary_version: String,
    pub oxigraph_version: String,
    pub rocksdb_version: String,
    pub stash_minimum_version: String,
    pub build: Build,
}

pub fn versions() -> Versions {
    let vocabulary_version = semantic_vocabulary::embedded()
        .ok()
        .and_then(|v| v.version())
        .unwrap_or_else(|| "unknown".to_string());
    Versions {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        contract_version: CONTRACT_VERSION.to_string(),
        vocabulary_version,
        oxigraph_version: env!("SE_OXIGRAPH_VERSION").to_string(),
        rocksdb_version: env!("SE_ROCKSDB_VERSION").to_string(),
        stash_minimum_version: STASH_MINIMUM_VERSION.to_string(),
        build: Build {
            target: env!("SE_TARGET").to_string(),
            profile: env!("SE_PROFILE").to_string(),
        },
    }
}
