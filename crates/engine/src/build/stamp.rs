//! The build stamp each generation carries (spec 008 research R7): what made it, and where the
//! next update catches up from. A missing or mismatched stamp makes the update rebuild in full.

use serde::{Deserialize, Serialize};
use stash_adapter::reader::ReadStart;

/// Bumped by any change to the mapping rules (spec 007 contracts/mapping-rules.md) that changes
/// what some record gives. `owners_disjoint` pins it to the fixture's expected graph.
pub const MAPPING_VERSION: u32 = 1;

/// Which task made a generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    Build,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildStamp {
    pub mapping_version: u32,
    pub vocabulary_version: String,
    /// Per kind, the newest `updated_at` when the build or update began, in Stash's own text.
    pub catch_up: ReadStart,
    pub how: How,
}

/// The embedded vocabulary's version.
pub fn vocabulary_version() -> String {
    semantic_vocabulary::embedded()
        .ok()
        .and_then(|v| v.version())
        .unwrap_or_default()
}

impl BuildStamp {
    pub fn new(catch_up: ReadStart, how: How) -> BuildStamp {
        BuildStamp {
            mapping_version: MAPPING_VERSION,
            vocabulary_version: vocabulary_version(),
            catch_up,
            how,
        }
    }

    /// The stamp of a generation; `None` when absent, unreadable or partial.
    pub fn read(generation: &crate::graph::Generation) -> Option<BuildStamp> {
        serde_json::from_value(generation.build.clone()?).ok()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("the stamp serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stash_adapter::reader::Kind;

    #[test]
    fn a_stamp_round_trips_in_camel_case() {
        let s = BuildStamp::new(
            [(Kind::Scene, "2026-10-26T10:00:00-06:00".to_string())].into(),
            How::Update,
        );
        let v = s.to_json();
        assert_eq!(v["mappingVersion"], MAPPING_VERSION);
        assert_eq!(v["catchUp"]["scene"], "2026-10-26T10:00:00-06:00");
        assert_eq!(v["how"], "update");
        assert_eq!(serde_json::from_value::<BuildStamp>(v).unwrap(), s);
        assert!(!s.vocabulary_version.is_empty());
    }
}
