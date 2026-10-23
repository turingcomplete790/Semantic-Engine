//! Read-only queries for test fixtures (spec 002 FR-015, constitution Principle VII).
//!
//! Every query is bounded by a `custom_fields` filter ("flag is not null") or by explicit ids, so
//! only the semantic vocabulary and the items that carry semantic data are fetched. Only query
//! operations exist in this module.

use crate::{schema, Client, Error, JsonAny, JsonMap};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Tag flags that mark the semantic tag types (vocabulary `stash-mapping.ttl`).
pub const TAG_FLAGS: [&str; 7] = [
    "plugtag",
    "isAction",
    "isRole",
    "isTrait",
    "isLabel",
    "isMiseEnScene",
    "isVariation",
];
/// Scene and image custom fields that carry semantic data.
pub const MEDIA_KEYS: [&str; 6] = [
    "roleTagAssignments",
    "actionAssignments",
    "pluggableAssignments",
    "actionPairings",
    "roleTagPairings",
    "timeline",
];
/// Performer flags that mark semantic performers.
pub const PERFORMER_FLAGS: [&str; 1] = ["isArchetype"];

#[derive(cynic::Enum, Clone, Copy, Debug)]
#[cynic(graphql_type = "CriterionModifier")]
pub enum CriterionModifier {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    IsNull,
    NotNull,
    IncludesAll,
    Includes,
    Excludes,
    MatchesRegex,
    NotMatchesRegex,
    Between,
    NotBetween,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "CustomFieldCriterionInput")]
pub struct CustomFieldCriterion {
    pub field: String,
    pub value: Option<Vec<JsonAny>>,
    pub modifier: CriterionModifier,
}

pub(crate) fn not_null(field: &str) -> Vec<CustomFieldCriterion> {
    vec![CustomFieldCriterion {
        field: field.to_string(),
        value: None,
        modifier: CriterionModifier::NotNull,
    }]
}

// ---- Tags

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "TagFilterType")]
pub struct TagFilter {
    #[cynic(rename = "custom_fields")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct TagsVars {
    pub filter: Option<TagFilter>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TagsVars")]
pub struct TagsQuery {
    #[arguments(tag_filter: $filter, filter: { per_page: -1 })]
    pub find_tags: FindTags,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindTagsResultType")]
pub struct FindTags {
    pub tags: Vec<TagRow>,
}

#[derive(cynic::QueryFragment, Debug, Serialize)]
#[cynic(graphql_type = "Tag")]
pub struct TagRow {
    pub id: cynic::Id,
    pub name: String,
    pub parents: Vec<IdOnlyTag>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}

#[derive(cynic::QueryFragment, Debug, Serialize)]
#[cynic(graphql_type = "Tag")]
pub struct IdOnlyTag {
    pub id: cynic::Id,
}

// ---- Performers

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "PerformerFilterType")]
pub struct PerformerFilter {
    #[cynic(rename = "custom_fields")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct PerformersVars {
    pub filter: Option<PerformerFilter>,
    pub ids: Option<Vec<cynic::Id>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PerformersVars")]
pub struct PerformersQuery {
    #[arguments(performer_filter: $filter, ids: $ids, filter: { per_page: -1 })]
    pub find_performers: FindPerformers,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindPerformersResultType")]
pub struct FindPerformers {
    pub performers: Vec<PerformerRow>,
}

#[derive(cynic::QueryFragment, Debug, Serialize)]
#[cynic(graphql_type = "Performer")]
pub struct PerformerRow {
    pub id: cynic::Id,
    pub name: String,
    pub tags: Vec<IdOnlyTag>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}

// ---- Scenes and images

#[derive(cynic::QueryFragment, Debug, Serialize)]
#[cynic(graphql_type = "Performer")]
pub struct IdOnlyPerformer {
    pub id: cynic::Id,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "SceneFilterType")]
pub struct SceneFilter {
    #[cynic(rename = "custom_fields")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ScenesVars {
    pub filter: Option<SceneFilter>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ScenesVars")]
pub struct ScenesQuery {
    #[arguments(scene_filter: $filter, filter: { per_page: -1 })]
    pub find_scenes: FindScenes,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct FindScenes {
    pub scenes: Vec<MediaRow>,
}

#[derive(cynic::QueryFragment, Debug, Serialize)]
#[cynic(graphql_type = "Scene")]
pub struct MediaRow {
    pub id: cynic::Id,
    pub title: Option<String>,
    pub performers: Vec<IdOnlyPerformer>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "ImageFilterType")]
pub struct ImageFilter {
    #[cynic(rename = "custom_fields")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ImagesVars {
    pub filter: Option<ImageFilter>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ImagesVars")]
pub struct ImagesQuery {
    #[arguments(image_filter: $filter, filter: { per_page: -1 })]
    pub find_images: FindImages,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
pub struct FindImages {
    pub images: Vec<ImageRow>,
}

#[derive(cynic::QueryFragment, Debug, Serialize)]
#[cynic(graphql_type = "Image")]
pub struct ImageRow {
    pub id: cynic::Id,
    pub title: Option<String>,
    pub performers: Vec<IdOnlyPerformer>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}

/// One query that was run and how many items it returned.
#[derive(Debug, Clone, Serialize)]
pub struct QueryRecord {
    pub query: String,
    pub items: usize,
}

/// The captured semantic data, merged by id per entity type.
#[derive(Debug, Default)]
pub struct Capture {
    pub tags: BTreeMap<String, Value>,
    pub performers: BTreeMap<String, Value>,
    pub scenes: BTreeMap<String, Value>,
    pub images: BTreeMap<String, Value>,
    pub queries: Vec<QueryRecord>,
}

fn add<T: Serialize>(map: &mut BTreeMap<String, Value>, id: &cynic::Id, row: &T) {
    map.entry(id.inner().to_string())
        .or_insert_with(|| serde_json::to_value(row).expect("rows serialize"));
}

impl Client {
    /// Captures the semantic data of a library with read-only, filtered queries.
    pub fn capture_semantic_fixtures(&self) -> Result<Capture, Error> {
        use cynic::QueryBuilder;
        let mut c = Capture::default();

        for flag in TAG_FLAGS {
            let rows = self
                .run(TagsQuery::build(TagsVars {
                    filter: Some(TagFilter {
                        custom_fields: Some(not_null(flag)),
                    }),
                }))?
                .find_tags
                .tags;
            c.queries.push(QueryRecord {
                query: format!("tags where {flag} is not null"),
                items: rows.len(),
            });
            for r in &rows {
                add(&mut c.tags, &r.id, r);
            }
        }

        let mut performer_ids: Vec<cynic::Id> = Vec::new();
        for key in MEDIA_KEYS {
            let scenes = self
                .run(ScenesQuery::build(ScenesVars {
                    filter: Some(SceneFilter {
                        custom_fields: Some(not_null(key)),
                    }),
                }))?
                .find_scenes
                .scenes;
            c.queries.push(QueryRecord {
                query: format!("scenes where {key} is not null"),
                items: scenes.len(),
            });
            for s in &scenes {
                performer_ids.extend(s.performers.iter().map(|p| p.id.clone()));
                add(&mut c.scenes, &s.id, s);
            }
            let images = self
                .run(ImagesQuery::build(ImagesVars {
                    filter: Some(ImageFilter {
                        custom_fields: Some(not_null(key)),
                    }),
                }))?
                .find_images
                .images;
            c.queries.push(QueryRecord {
                query: format!("images where {key} is not null"),
                items: images.len(),
            });
            for i in &images {
                performer_ids.extend(i.performers.iter().map(|p| p.id.clone()));
                add(&mut c.images, &i.id, i);
            }
        }

        for flag in PERFORMER_FLAGS {
            let rows = self
                .run(PerformersQuery::build(PerformersVars {
                    filter: Some(PerformerFilter {
                        custom_fields: Some(not_null(flag)),
                    }),
                    ids: None,
                }))?
                .find_performers
                .performers;
            c.queries.push(QueryRecord {
                query: format!("performers where {flag} is not null"),
                items: rows.len(),
            });
            for r in &rows {
                add(&mut c.performers, &r.id, r);
            }
        }

        performer_ids.sort_by(|a, b| a.inner().cmp(b.inner()));
        performer_ids.dedup_by(|a, b| a.inner() == b.inner());
        performer_ids.retain(|id| !c.performers.contains_key(id.inner()));
        if !performer_ids.is_empty() {
            let n = performer_ids.len();
            let rows = self
                .run(PerformersQuery::build(PerformersVars {
                    filter: None,
                    ids: Some(performer_ids),
                }))?
                .find_performers
                .performers;
            c.queries.push(QueryRecord {
                query: format!("performers referenced by semantic scenes and images ({n} ids)"),
                items: rows.len(),
            });
            for r in &rows {
                add(&mut c.performers, &r.id, r);
            }
        }
        Ok(c)
    }
}
