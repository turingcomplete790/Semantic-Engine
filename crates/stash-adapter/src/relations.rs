//! Reads and writes for relations, auto-apply, and recommendations (spec 004, research R8).
//!
//! Every read is bounded: tags by a "flag is not null" filter, items and performers by explicit
//! ids in batches. Writes are partial `custom_fields` updates of scenes and images.

use crate::fixtures::{not_null, TagFilter, TagsQuery, TagsVars};
use crate::{Client, Error};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The tag flags whose tags the relation, auto-apply, and recommendation logic reads: Roles,
/// Relations, Labels, Actions, Pluggables, Mise-en-scène, and common variants.
pub const DEFINITION_FLAGS: [&str; 7] = [
    "isRole",
    "isRelation",
    "isLabel",
    "isAction",
    "plugtag",
    "isMiseEnScene",
    "variantOfTagId",
];

/// One tag as the engine's relation logic needs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagRecord {
    pub id: String,
    pub name: String,
    /// Direct Stash parents, as tag ids.
    pub parents: Vec<String>,
    pub custom_fields: Map<String, Value>,
}

fn id_order(id: &str) -> (u64, String) {
    (id.parse().unwrap_or(u64::MAX), id.to_string())
}

impl Client {
    /// Every tag carrying one of [`DEFINITION_FLAGS`], merged by id and sorted by id. One
    /// "flag is not null" query per flag (custom-field criteria combine only with AND), so the
    /// number of requests is fixed, never per tag (constitution Principle VII).
    pub fn relation_definitions(&self) -> Result<Vec<TagRecord>, Error> {
        use cynic::QueryBuilder;
        let mut by_id: BTreeMap<(u64, String), TagRecord> = BTreeMap::new();
        for flag in DEFINITION_FLAGS {
            let rows = self
                .run(TagsQuery::build(TagsVars {
                    filter: Some(TagFilter {
                        custom_fields: Some(not_null(flag)),
                    }),
                }))?
                .find_tags
                .tags;
            for r in rows {
                let id = r.id.inner().to_string();
                by_id.entry(id_order(&id)).or_insert_with(|| TagRecord {
                    name: r.name,
                    parents: r.parents.iter().map(|p| p.id.inner().to_string()).collect(),
                    custom_fields: match r.custom_fields.0 {
                        Value::Object(m) => m,
                        _ => Map::new(),
                    },
                    id,
                });
            }
        }
        Ok(by_id.into_values().collect())
    }
}

/// Scenes and images, the two media kinds relations apply to.
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

/// One scene or image as auto-apply needs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemRecord {
    pub kind: MediaKind,
    pub id: String,
    /// Performer ids on the item.
    pub performers: Vec<String>,
    /// Exactly as Stash stores it (server-local RFC 3339 text).
    pub updated_at: String,
    pub custom_fields: Map<String, Value>,
}

/// One performer's tag ids (its Labels among them).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformerRecord {
    pub id: String,
    pub tags: Vec<String>,
}

// ---- Scenes and images, performers, writes (spec 004 research R8)

use crate::fixtures::{CriterionModifier, CustomFieldCriterion, IdOnlyPerformer, IdOnlyTag};
use crate::{schema, JsonMap};

/// Stash's `Time` scalar, kept exactly as Stash sends it (server-local RFC 3339 text).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StashTime(pub Value);
cynic::impl_scalar!(StashTime, schema::Time);

impl StashTime {
    fn text(&self) -> String {
        self.0.as_str().unwrap_or_default().to_string()
    }
}

/// How many items one request reads (research R8).
pub const BATCH: usize = 100;

#[derive(cynic::QueryVariables, Debug)]
pub struct ItemIdsVars {
    pub ids: Option<Vec<cynic::Id>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ItemIdsVars")]
pub struct ScenesByIds {
    #[arguments(ids: $ids, filter: { per_page: -1 })]
    pub find_scenes: SceneItems,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct SceneItems {
    pub scenes: Vec<SceneItem>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct SceneItem {
    pub id: cynic::Id,
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
    pub performers: Vec<IdOnlyPerformer>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ItemIdsVars")]
pub struct ImagesByIds {
    #[arguments(ids: $ids, filter: { per_page: -1 })]
    pub find_images: ImageItems,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
pub struct ImageItems {
    pub images: Vec<ImageItem>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
pub struct ImageItem {
    pub id: cynic::Id,
    #[cynic(rename = "updated_at")]
    pub updated_at: StashTime,
    pub performers: Vec<IdOnlyPerformer>,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ItemIdsVars")]
pub struct PerformersByIds {
    #[arguments(ids: $ids, filter: { per_page: -1 })]
    pub find_performers: PerformerTagsPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindPerformersResultType")]
pub struct PerformerTagsPage {
    pub performers: Vec<PerformerTags>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Performer")]
pub struct PerformerTags {
    pub id: cynic::Id,
    pub tags: Vec<IdOnlyTag>,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "TimestampCriterionInput")]
pub struct TimestampCriterion {
    pub value: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub value2: Option<String>,
    pub modifier: CriterionModifier,
}

#[derive(cynic::InputObject, Debug, Default)]
#[cynic(graphql_type = "SceneFilterType")]
pub struct SceneItemFilter {
    #[cynic(rename = "updated_at", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<TimestampCriterion>,
    #[cynic(rename = "custom_fields", skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
}

#[derive(cynic::InputObject, Debug, Default)]
#[cynic(graphql_type = "ImageFilterType")]
pub struct ImageItemFilter {
    #[cynic(rename = "updated_at", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<TimestampCriterion>,
    #[cynic(rename = "custom_fields", skip_serializing_if = "Option::is_none")]
    pub custom_fields: Option<Vec<CustomFieldCriterion>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct SceneFilterVars {
    pub filter: Option<SceneItemFilter>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ImageFilterVars {
    pub filter: Option<ImageItemFilter>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "SceneFilterVars")]
pub struct SceneIdsQuery {
    #[arguments(scene_filter: $filter, filter: { per_page: -1 })]
    pub find_scenes: SceneIdsPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct SceneIdsPage {
    pub scenes: Vec<SceneIdOnly>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct SceneIdOnly {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ImageFilterVars")]
pub struct ImageIdsQuery {
    #[arguments(image_filter: $filter, filter: { per_page: -1 })]
    pub find_images: ImageIdsPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
pub struct ImageIdsPage {
    pub images: Vec<ImageIdOnly>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
pub struct ImageIdOnly {
    pub id: cynic::Id,
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

/// `CustomFieldsInput` with only `partial`: never replaces or removes other keys.
#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "CustomFieldsInput")]
pub struct CustomFieldsPatch {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub partial: Option<JsonMap>,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "SceneUpdateInput")]
pub struct SceneCustomFieldsUpdate {
    pub id: cynic::Id,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: Option<CustomFieldsPatch>,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "ImageUpdateInput")]
pub struct ImageCustomFieldsUpdate {
    pub id: cynic::Id,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: Option<CustomFieldsPatch>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct SceneUpdateVars {
    pub input: SceneCustomFieldsUpdate,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SceneUpdateVars")]
pub struct SceneCustomFieldsMutation {
    #[arguments(input: $input)]
    pub scene_update: Option<SceneIdOnly>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ImageUpdateVars {
    pub input: ImageCustomFieldsUpdate,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "ImageUpdateVars")]
pub struct ImageCustomFieldsMutation {
    #[arguments(input: $input)]
    pub image_update: Option<ImageIdOnly>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct RunTaskVars {
    pub plugin_id: cynic::Id,
    pub task_name: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "RunTaskVars")]
pub struct RunTask {
    #[arguments(plugin_id: $plugin_id, task_name: $task_name)]
    pub run_plugin_task: cynic::Id,
}

fn ids(list: &[String]) -> Option<Vec<cynic::Id>> {
    Some(list.iter().map(cynic::Id::new).collect())
}

fn object(v: JsonMap) -> Map<String, Value> {
    match v.0 {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn id_list(mut v: Vec<String>) -> Vec<String> {
    v.sort_by_key(|s| id_order(s));
    v.dedup();
    v
}

impl Client {
    /// Scenes or images by id, read [`BATCH`] at a time.
    pub fn items_by_ids(&self, kind: MediaKind, list: &[String]) -> Result<Vec<ItemRecord>, Error> {
        use cynic::QueryBuilder;
        let mut out = Vec::new();
        for chunk in list.chunks(BATCH) {
            let vars = ItemIdsVars { ids: ids(chunk) };
            match kind {
                MediaKind::Scene => {
                    for s in self.run(ScenesByIds::build(vars))?.find_scenes.scenes {
                        out.push(ItemRecord {
                            kind,
                            id: s.id.inner().to_string(),
                            performers: s
                                .performers
                                .iter()
                                .map(|p| p.id.inner().to_string())
                                .collect(),
                            updated_at: s.updated_at.text(),
                            custom_fields: object(s.custom_fields),
                        });
                    }
                }
                MediaKind::Image => {
                    for i in self.run(ImagesByIds::build(vars))?.find_images.images {
                        out.push(ItemRecord {
                            kind,
                            id: i.id.inner().to_string(),
                            performers: i
                                .performers
                                .iter()
                                .map(|p| p.id.inner().to_string())
                                .collect(),
                            updated_at: i.updated_at.text(),
                            custom_fields: object(i.custom_fields),
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    /// Each performer's tag ids, read [`BATCH`] at a time.
    pub fn performer_tags(&self, list: &[String]) -> Result<Vec<PerformerRecord>, Error> {
        use cynic::QueryBuilder;
        let mut out = Vec::new();
        for chunk in list.chunks(BATCH) {
            let page = self.run(PerformersByIds::build(ItemIdsVars { ids: ids(chunk) }))?;
            out.extend(
                page.find_performers
                    .performers
                    .into_iter()
                    .map(|p| PerformerRecord {
                        id: p.id.inner().to_string(),
                        tags: p.tags.iter().map(|t| t.id.inner().to_string()).collect(),
                    }),
            );
        }
        Ok(out)
    }

    fn item_ids(
        &self,
        kind: MediaKind,
        updated_at: Option<TimestampCriterion>,
        cf: Option<Vec<CustomFieldCriterion>>,
    ) -> Result<Vec<String>, Error> {
        use cynic::QueryBuilder;
        Ok(match kind {
            MediaKind::Scene => self
                .run(SceneIdsQuery::build(SceneFilterVars {
                    filter: Some(SceneItemFilter {
                        updated_at,
                        custom_fields: cf,
                    }),
                }))?
                .find_scenes
                .scenes
                .into_iter()
                .map(|s| s.id.inner().to_string())
                .collect(),
            MediaKind::Image => self
                .run(ImageIdsQuery::build(ImageFilterVars {
                    filter: Some(ImageItemFilter {
                        updated_at,
                        custom_fields: cf,
                    }),
                }))?
                .find_images
                .images
                .into_iter()
                .map(|i| i.id.inner().to_string())
                .collect(),
        })
    }

    /// Ids of items with `updated_at` after `threshold`. Stash compares the value as text to
    /// the second, so `threshold` must be in the server's own local-time form; ids are never
    /// sent with it, because Stash then ignores the filter (003 findings).
    pub fn changed_since(&self, kind: MediaKind, threshold: &str) -> Result<Vec<String>, Error> {
        let c = TimestampCriterion {
            value: threshold.to_string(),
            value2: None,
            modifier: CriterionModifier::GreaterThan,
        };
        self.item_ids(kind, Some(c), None).map(id_list)
    }

    /// Ids of items that can hold relations: those with action pairings or relation instances.
    /// Two flag-filtered reads, never the whole library (constitution Principle VII).
    pub fn ids_with_relation_data(&self, kind: MediaKind) -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        for key in ["actionPairings", "relationAssignments"] {
            out.extend(self.item_ids(kind, None, Some(not_null(key)))?);
        }
        Ok(id_list(out))
    }

    /// The newest `updated_at` of the kind, exactly as Stash stores it.
    pub fn latest_updated_at(&self, kind: MediaKind) -> Result<Option<String>, Error> {
        use cynic::QueryBuilder;
        Ok(match kind {
            MediaKind::Scene => self
                .run(LatestScene::build(()))?
                .find_scenes
                .scenes
                .first()
                .map(|s| s.updated_at.text()),
            MediaKind::Image => self
                .run(LatestImage::build(()))?
                .find_images
                .images
                .first()
                .map(|i| i.updated_at.text()),
        })
    }

    /// Writes the keys in `partial` to one item's custom fields; other keys are untouched.
    pub fn update_item_custom_fields(
        &self,
        kind: MediaKind,
        id: &str,
        partial: Map<String, Value>,
    ) -> Result<(), Error> {
        use cynic::MutationBuilder;
        let patch = Some(CustomFieldsPatch {
            partial: Some(JsonMap(Value::Object(partial))),
        });
        match kind {
            MediaKind::Scene => {
                self.run(SceneCustomFieldsMutation::build(SceneUpdateVars {
                    input: SceneCustomFieldsUpdate {
                        id: cynic::Id::new(id),
                        custom_fields: patch,
                    },
                }))?;
            }
            MediaKind::Image => {
                self.run(ImageCustomFieldsMutation::build(ImageUpdateVars {
                    input: ImageCustomFieldsUpdate {
                        id: cynic::Id::new(id),
                        custom_fields: patch,
                    },
                }))?;
            }
        }
        Ok(())
    }

    /// Queues one of a plugin's tasks in Stash's job queue; returns the job id.
    pub fn run_plugin_task(&self, plugin_id: &str, task_name: &str) -> Result<String, Error> {
        use cynic::MutationBuilder;
        let job = self
            .run(RunTask::build(RunTaskVars {
                plugin_id: cynic::Id::new(plugin_id),
                task_name: Some(task_name.to_string()),
            }))?
            .run_plugin_task;
        Ok(job.inner().to_string())
    }
}

// ---- Native tags, for recommendations (spec 004 research R13)

#[derive(cynic::QueryVariables, Debug)]
pub struct ItemIdVars {
    pub id: Option<cynic::Id>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ItemIdVars")]
pub struct SceneNativeTags {
    #[arguments(id: $id)]
    pub find_scene: Option<WithTagsScene>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct WithTagsScene {
    pub tags: Vec<IdOnlyTag>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "ItemIdVars")]
pub struct ImageNativeTags {
    #[arguments(id: $id)]
    pub find_image: Option<WithTagsImage>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
pub struct WithTagsImage {
    pub tags: Vec<IdOnlyTag>,
}

impl Client {
    /// One item's native Stash tag ids, or `None` when the item does not exist. Read only.
    pub fn item_native_tags(
        &self,
        kind: MediaKind,
        id: &str,
    ) -> Result<Option<Vec<String>>, Error> {
        use cynic::QueryBuilder;
        let vars = || ItemIdVars {
            id: Some(cynic::Id::new(id)),
        };
        let tags = match kind {
            MediaKind::Scene => self
                .run(SceneNativeTags::build(vars()))?
                .find_scene
                .map(|s| s.tags),
            MediaKind::Image => self
                .run(ImageNativeTags::build(vars()))?
                .find_image
                .map(|i| i.tags),
        };
        Ok(tags.map(|ts| ts.iter().map(|t| t.id.inner().to_string()).collect()))
    }
}
