//! The records the reader returns (spec 005 data-model). Ids are Stash's ids as strings; times
//! are Stash's server-local text, exactly as stored. Every list is sorted by numeric id.

use super::field::{Field, GraphPosition};
pub use super::measurements::Measurements;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Scenes and images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Scene,
    Image,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaKind::Scene => "scene",
            MediaKind::Image => "image",
        }
    }
    pub fn parse(s: &str) -> Option<MediaKind> {
        match s.to_ascii_lowercase().as_str() {
            "scene" => Some(MediaKind::Scene),
            "image" => Some(MediaKind::Image),
            _ => None,
        }
    }
}

/// The four kinds of record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Scene,
    Image,
    Performer,
    Tag,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Scene, Kind::Image, Kind::Performer, Kind::Tag];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Scene => "scene",
            Kind::Image => "image",
            Kind::Performer => "performer",
            Kind::Tag => "tag",
        }
    }
}

impl From<MediaKind> for Kind {
    fn from(k: MediaKind) -> Kind {
        match k {
            MediaKind::Scene => Kind::Scene,
            MediaKind::Image => Kind::Image,
        }
    }
}

/// The semantic tag types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TagFlag {
    Pluggable,
    Action,
    Role,
    Trait,
    Label,
    MiseEnScene,
    Variation,
    Relation,
}

impl TagFlag {
    /// The Stash `custom_fields` key of the flag.
    pub fn key(self) -> &'static str {
        match self {
            TagFlag::Pluggable => "plugtag",
            TagFlag::Action => "isAction",
            TagFlag::Role => "isRole",
            TagFlag::Trait => "isTrait",
            TagFlag::Label => "isLabel",
            TagFlag::MiseEnScene => "isMiseEnScene",
            TagFlag::Variation => "isVariation",
            TagFlag::Relation => "isRelation",
        }
    }

    pub const ALL: [TagFlag; 8] = [
        TagFlag::Pluggable,
        TagFlag::Action,
        TagFlag::Role,
        TagFlag::Trait,
        TagFlag::Label,
        TagFlag::MiseEnScene,
        TagFlag::Variation,
        TagFlag::Relation,
    ];
}

/// A tag with at least one semantic flag, or a common variant (`variantOfTagId`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticTag {
    pub id: String,
    pub name: String,
    pub flags: BTreeSet<TagFlag>,
    /// Direct Stash parents.
    pub parents: Vec<String>,
    /// The tag keys of research R1 that are present.
    pub fields: BTreeMap<String, Field>,
    pub updated_at: String,
}

/// Display data, passed to clients and never a fact in the graph (FR-002a).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformerDisplay {
    /// `None` when absent or malformed: "not placed".
    pub graph_position: Option<GraphPosition>,
}

impl SemanticTag {
    /// The full read's rule (spec 008 research R3): a flag is set, or `variantOfTagId` is
    /// present.
    pub fn is_semantic(&self) -> bool {
        !self.flags.is_empty() || self.fields.contains_key("variantOfTagId")
    }
}

/// The ids of tags with a Trait, Label or Variation flag: the tags that make their performers
/// semantic.
pub fn performer_tag_set(tags: &[SemanticTag]) -> BTreeSet<String> {
    tags.iter()
        .filter(|t| {
            t.flags
                .iter()
                .any(|f| super::keys::PERFORMER_TAG_FLAGS.contains(&f.key()))
        })
        .map(|t| t.id.clone())
        .collect()
}

/// A performer with semantic data: a Trait, Label or Variation tag, an Archetype field, or a
/// non-empty `measurements` field (FR-002).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticPerformer {
    pub id: String,
    /// All of the performer's tags; the builder picks the semantic ones.
    pub tags: Vec<String>,
    pub archetype: bool,
    pub archetype_parents: Option<Field>,
    pub measurements: Option<Measurements>,
    /// Present when the performer has an `archetypeGraphPosition` key.
    pub display: Option<PerformerDisplay>,
    pub updated_at: String,
}

impl SemanticPerformer {
    /// The full read's rule: an Archetype, archetype parents or measurements, or a tag in
    /// `performer_tags` (see [`performer_tag_set`]).
    pub fn is_semantic(&self, performer_tags: &BTreeSet<String>) -> bool {
        self.archetype
            || self.archetype_parents.is_some()
            || self.measurements.is_some()
            || self.tags.iter().any(|t| performer_tags.contains(t))
    }
}

/// A scene or image with at least one semantic field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRecord {
    pub kind: MediaKind,
    pub id: String,
    pub performers: Vec<String>,
    /// Native tags.
    pub tags: Vec<String>,
    /// The media keys of research R1 that are present.
    pub fields: BTreeMap<String, Field>,
    pub updated_at: String,
}

impl MediaRecord {
    /// The full read's rule: at least one semantic field is present.
    pub fn is_semantic(&self) -> bool {
        !self.fields.is_empty()
    }
}

/// The newest `updated_at` of each kind, taken before discovery (FR-007). A kind with no items
/// is absent.
pub type ReadStart = BTreeMap<Kind, String>;

/// The result of a full read. A stopped read never produces one.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LibraryRead {
    pub start: ReadStart,
    pub tags: Vec<SemanticTag>,
    pub performers: Vec<SemanticPerformer>,
    pub scenes: Vec<MediaRecord>,
    pub images: Vec<MediaRecord>,
    /// Ids found in discovery but gone by the time their records were read.
    pub missing: BTreeMap<Kind, usize>,
    /// Requests sent.
    pub requests: usize,
}

/// Records for a list of ids (FR-010).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordBatch<T> {
    pub found: Vec<T>,
    /// Requested ids that no longer exist.
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Discover,
    Records,
    Done,
}

/// How far a full read has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub phase: Phase,
    pub done: usize,
    pub total: usize,
}

impl Progress {
    /// The share done, from 0 to 1. Discovery counts as the first 5% (research R7).
    pub fn share(&self) -> f64 {
        let part = |done: usize, total: usize| {
            if total == 0 {
                1.0
            } else {
                done.min(total) as f64 / total as f64
            }
        };
        match self.phase {
            Phase::Discover => 0.05 * part(self.done, self.total),
            Phase::Records => 0.05 + 0.95 * part(self.done, self.total),
            Phase::Done => 1.0,
        }
    }
}

/// Sort key for Stash ids: numeric order, with anything non-numeric last.
pub(crate) fn id_order(id: &str) -> (u64, String) {
    (id.parse().unwrap_or(u64::MAX), id.to_string())
}

/// Sorts ids by numeric order and removes duplicates.
pub(crate) fn sorted_ids(mut v: Vec<String>) -> Vec<String> {
    v.sort_by_key(|s| id_order(s));
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_library_read_round_trips_through_json() {
        let mut fields = BTreeMap::new();
        fields.insert("timeline".to_string(), Field::Value(json!([1])));
        fields.insert(
            "actionPairings".to_string(),
            Field::Unreadable {
                raw: json!("{bad"),
                reason: "not valid JSON".into(),
            },
        );
        let read = LibraryRead {
            start: [(Kind::Scene, "2026-10-24T10:00:00-06:00".to_string())].into(),
            tags: vec![SemanticTag {
                id: "3".into(),
                name: "Kissing".into(),
                flags: [TagFlag::Action].into(),
                parents: vec!["1".into()],
                fields: BTreeMap::new(),
                updated_at: "2026-10-24T10:00:00-06:00".into(),
            }],
            performers: vec![SemanticPerformer {
                id: "7".into(),
                tags: vec!["3".into()],
                archetype: true,
                archetype_parents: Some(Field::Value(json!(["8"]))),
                measurements: Some(Measurements::parse("34D-26-35")),
                display: Some(PerformerDisplay {
                    graph_position: Some(GraphPosition { x: 1.0, y: 2.5 }),
                }),
                updated_at: "2026-10-24T10:00:00-06:00".into(),
            }],
            scenes: vec![MediaRecord {
                kind: MediaKind::Scene,
                id: "12".into(),
                performers: vec!["7".into()],
                tags: vec![],
                fields,
                updated_at: "2026-10-24T10:00:00-06:00".into(),
            }],
            images: vec![],
            missing: [(Kind::Image, 1)].into(),
            requests: 9,
        };
        let text = serde_json::to_string(&read).unwrap();
        assert_eq!(serde_json::from_str::<LibraryRead>(&text).unwrap(), read);
    }

    #[test]
    fn progress_share_counts_discovery_as_the_first_five_percent() {
        let p = |phase, done, total| Progress { phase, done, total }.share();
        assert_eq!(p(Phase::Discover, 0, 10), 0.0);
        assert!((p(Phase::Discover, 10, 10) - 0.05).abs() < 1e-12);
        assert!((p(Phase::Records, 0, 4) - 0.05).abs() < 1e-12);
        assert!((p(Phase::Records, 4, 4) - 1.0).abs() < 1e-12);
        assert_eq!(p(Phase::Records, 0, 0), 1.0);
        assert_eq!(p(Phase::Done, 0, 0), 1.0);
    }
}
