//! The keys the reader reads, and the top-level JSON kind each must have (spec 005 research R1).
//! One table drives discovery, parsing, fixtures, and the vocabulary check, so they cannot drift.

/// The top-level kind a key's value must have. Deeper checks belong to the graph builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A type flag: `1`, `"1"`, `true` or `"true"` means set (spec 004, FR-014).
    Flag,
    List,
    Object,
    /// A Stash id, stored as a string or a number; kept as a string.
    Id,
}

/// The tag flags that mark the semantic tag types (vocabulary `stash-mapping.ttl`).
pub const TAG_FLAGS: [&str; 8] = [
    "plugtag",
    "isAction",
    "isRole",
    "isTrait",
    "isLabel",
    "isMiseEnScene",
    "isVariation",
    "isRelation",
];

/// Tag keys other than the flags.
pub const TAG_KEYS: [(&str, Shape); 8] = [
    ("variantOfTagId", Shape::Id),
    ("categories", Shape::List),
    ("variations", Shape::List),
    ("labelFormula", Shape::Object),
    ("variationData", Shape::Object),
    ("relationEnds", Shape::Object),
    ("relationRules", Shape::List),
    ("fieldMapping", Shape::Object),
];

/// Tag keys searched with "is not null" to find the semantic tags: every flag, plus common
/// variants, which may carry no flag of their own (FR-001).
pub const TAG_DISCOVERY: [&str; 9] = [
    "plugtag",
    "isAction",
    "isRole",
    "isTrait",
    "isLabel",
    "isMiseEnScene",
    "isVariation",
    "isRelation",
    "variantOfTagId",
];

/// Performer keys. `archetypeGraphPosition` is display data, kept apart from the semantic
/// fields (FR-002a).
pub const PERFORMER_KEYS: [(&str, Shape); 3] = [
    ("isArchetype", Shape::Flag),
    ("archetypeParents", Shape::List),
    ("archetypeGraphPosition", Shape::Object),
];

/// Performer keys searched with "is not null". Performers are also found by their Trait, Label
/// and Variation tags, and by a non-empty native `measurements` field (FR-002, FR-002b).
pub const PERFORMER_DISCOVERY: [&str; 2] = ["isArchetype", "archetypeParents"];

/// The display-only performer key (FR-002a).
pub const GRAPH_POSITION: &str = "archetypeGraphPosition";

/// Scene and image keys; each is also searched with "is not null" (FR-003).
pub const MEDIA_KEYS: [(&str, Shape); 8] = [
    ("roleTagAssignments", Shape::Object),
    ("actionAssignments", Shape::Object),
    ("pluggableAssignments", Shape::Object),
    ("autoRoleAssignments", Shape::Object),
    ("actionPairings", Shape::List),
    ("roleTagPairings", Shape::List),
    ("relationAssignments", Shape::List),
    ("timeline", Shape::List),
];

/// Keys that are never returned: retired (vocabulary 1.0.0, spec 001 FR-016) or plugin
/// application data and caches the vocabulary excludes on purpose (FR-002).
pub const NOT_READ: [&str; 6] = [
    "isGeneric",
    "looks",
    "archetypeRecipe",
    "directTraitVariations",
    "inheritedVariations",
    "inheritedTraitVariations",
];

/// The tag flags whose tags make a performer semantic when it carries one (FR-002).
pub const PERFORMER_TAG_FLAGS: [&str; 3] = ["isTrait", "isLabel", "isVariation"];
