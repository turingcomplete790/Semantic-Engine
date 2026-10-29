//! The build stamp each generation carries (spec 008 research R7): what made it, and where the
//! next update catches up from. A missing or mismatched stamp makes the update rebuild in full.

use serde::{Deserialize, Serialize};
use stash_adapter::reader::{Kind, ReadStart};

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

/// Items per kind in a generation: its owners of each kind (spec 009 research R9).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Items {
    pub tags: u64,
    pub performers: u64,
    pub scenes: u64,
    pub images: u64,
}

impl Items {
    fn slot(&mut self, kind: Kind) -> &mut u64 {
        match kind {
            Kind::Tag => &mut self.tags,
            Kind::Performer => &mut self.performers,
            Kind::Scene => &mut self.scenes,
            Kind::Image => &mut self.images,
        }
    }

    /// Adds `n` (which may be negative) to the count of `kind`, never going below zero.
    pub fn add(&mut self, kind: Kind, n: i64) {
        let slot = self.slot(kind);
        *slot = slot.saturating_add_signed(n);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildStamp {
    pub mapping_version: u32,
    pub vocabulary_version: String,
    /// Per kind, the newest `updated_at` when the build or update began, in Stash's own text.
    pub catch_up: ReadStart,
    pub how: How,
    /// Absent in stamps written before contract 1.4.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Items>,
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
            items: None,
        }
    }

    pub fn with_items(mut self, items: Items) -> BuildStamp {
        self.items = Some(items);
        self
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
