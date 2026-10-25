//! Typed read operations of the library reader (spec 005 research R3 to R5). Each selects only
//! the fields of research R1, and every one is bounded: by a "key is not null" filter, by tag
//! ids, by an `updated_at` threshold, or by explicit ids.

use crate::fixtures::{CriterionModifier, CustomFieldCriterion, IdOnlyPerformer, IdOnlyTag};
use crate::{schema, JsonMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stash's `Time` scalar, kept exactly as Stash sends it (server-local RFC 3339 text).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StashTime(pub Value);
cynic::impl_scalar!(StashTime, schema::Time);

impl StashTime {
    pub fn text(&self) -> String {
        self.0.as_str().unwrap_or_default().to_string()
    }
}

// ---- Criteria

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "TimestampCriterionInput")]
pub struct TimestampCriterion {
    pub value: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub value2: Option<String>,
    pub modifier: CriterionModifier,
}

impl TimestampCriterion {
    /// `updated_at > threshold`, compared by Stash as text to the second.
    pub fn after(threshold: &str) -> TimestampCriterion {
        TimestampCriterion {
            value: threshold.to_string(),
            value2: None,
            modifier: CriterionModifier::GreaterThan,
        }
    }
}

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "HierarchicalMultiCriterionInput")]
pub struct TagsCriterion {
    pub value: Option<Vec<cynic::Id>>,
    pub modifier: CriterionModifier,
}

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "StringCriterionInput")]
pub struct StringCriterion {
    pub value: String,
    pub modifier: CriterionModifier,
}

// ---- Filters

#[derive(cynic::InputObject, Debug, Clone, Default)]
#[cynic(graphql_type = "TagFilterType")]
pub struct TagIdFilter {
    #[cynic(rename = "custom_fields", skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
    #[cynic(rename = "updated_at", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<TimestampCriterion>,
}

#[derive(cynic::InputObject, Debug, Clone, Default)]
#[cynic(graphql_type = "PerformerFilterType")]
pub struct PerformerIdFilter {
    #[cynic(rename = "custom_fields", skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
    #[cynic(rename = "updated_at", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<TimestampCriterion>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub tags: Option<TagsCriterion>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub measurements: Option<StringCriterion>,
}

#[derive(cynic::InputObject, Debug, Clone, Default)]
#[cynic(graphql_type = "SceneFilterType")]
pub struct SceneIdFilter {
    #[cynic(rename = "custom_fields", skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
    #[cynic(rename = "updated_at", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<TimestampCriterion>,
}

#[derive(cynic::InputObject, Debug, Clone, Default)]
#[cynic(graphql_type = "ImageFilterType")]
pub struct ImageIdFilter {
    #[cynic(rename = "custom_fields", skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
    #[cynic(rename = "updated_at", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<TimestampCriterion>,
}

// ---- Id queries: discovery (a filter), change reads (a threshold), id lists (no filter)

#[derive(cynic::QueryVariables, Debug)]
pub struct TagIdVars {
    pub filter: Option<TagIdFilter>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct PerformerIdVars {
    pub filter: Option<PerformerIdFilter>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct SceneIdVars {
    pub filter: Option<SceneIdFilter>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ImageIdVars {
    pub filter: Option<ImageIdFilter>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "TagIdVars")]
pub struct TagIds {
    #[arguments(tag_filter: $filter, filter: { per_page: -1 })]
    pub find_tags: TagIdPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindTagsResultType")]
pub struct TagIdPage {
    pub tags: Vec<IdOnlyTag>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PerformerIdVars")]
pub struct PerformerIds {
    #[arguments(performer_filter: $filter, filter: { per_page: -1 })]
    pub find_performers: PerformerIdPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindPerformersResultType")]
pub struct PerformerIdPage {
    pub performers: Vec<IdOnlyPerformer>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SceneIdVars")]
pub struct SceneIds {
    #[arguments(scene_filter: $filter, filter: { per_page: -1 })]
    pub find_scenes: SceneIdPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct SceneIdPage {
    pub scenes: Vec<IdOnlyScene>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct IdOnlyScene {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ImageIdVars")]
pub struct ImageIds {
    #[arguments(image_filter: $filter, filter: { per_page: -1 })]
    pub find_images: ImageIdPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
pub struct ImageIdPage {
    pub images: Vec<IdOnlyImage>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
pub struct IdOnlyImage {
    pub id: cynic::Id,
}

// ---- Records by ids

#[derive(cynic::QueryVariables, Debug)]
pub struct IdsVars {
    pub ids: Option<Vec<cynic::Id>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "IdsVars")]
pub struct TagsByIds {
    #[arguments(ids: $ids, filter: { per_page: -1 })]
    pub find_tags: TagRows,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindTagsResultType")]
pub struct TagRows {
    pub tags: Vec<TagRow>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
pub struct TagRow {
    pub id: cynic::Id,
    pub name: String,
    pub parents: Vec<IdOnlyTag>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "IdsVars")]
pub struct PerformersByIds {
    #[arguments(ids: $ids, filter: { per_page: -1 })]
    pub find_performers: PerformerRows,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindPerformersResultType")]
pub struct PerformerRows {
    pub performers: Vec<PerformerRow>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Performer")]
pub struct PerformerRow {
    pub id: cynic::Id,
    pub tags: Vec<IdOnlyTag>,
    pub measurements: Option<String>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "IdsVars")]
pub struct ScenesByIds {
    #[arguments(ids: $ids, filter: { per_page: -1 })]
    pub find_scenes: SceneRows,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct SceneRows {
    pub scenes: Vec<SceneRow>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct SceneRow {
    pub id: cynic::Id,
    pub performers: Vec<IdOnlyPerformer>,
    pub tags: Vec<IdOnlyTag>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "IdsVars")]
pub struct ImagesByIds {
    #[arguments(ids: $ids, filter: { per_page: -1 })]
    pub find_images: ImageRows,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
pub struct ImageRows {
    pub images: Vec<ImageRow>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
pub struct ImageRow {
    pub id: cynic::Id,
    pub performers: Vec<IdOnlyPerformer>,
    pub tags: Vec<IdOnlyTag>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
}

// ---- The newest updated_at of each kind (research R5)

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct LatestTag {
    #[arguments(filter: { per_page: 1, sort: "updated_at", direction: DESC })]
    pub find_tags: LatestTagPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindTagsResultType")]
pub struct LatestTagPage {
    pub tags: Vec<TagTime>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
pub struct TagTime {
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct LatestPerformer {
    #[arguments(filter: { per_page: 1, sort: "updated_at", direction: DESC })]
    pub find_performers: LatestPerformerPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindPerformersResultType")]
pub struct LatestPerformerPage {
    pub performers: Vec<PerformerTime>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Performer")]
pub struct PerformerTime {
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct LatestScene {
    #[arguments(filter: { per_page: 1, sort: "updated_at", direction: DESC })]
    pub find_scenes: LatestScenePage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct LatestScenePage {
    pub scenes: Vec<SceneTime>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct SceneTime {
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct LatestImage {
    #[arguments(filter: { per_page: 1, sort: "updated_at", direction: DESC })]
    pub find_images: LatestImagePage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
pub struct LatestImagePage {
    pub images: Vec<ImageTime>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
pub struct ImageTime {
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
}
