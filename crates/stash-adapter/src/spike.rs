//! Spike-only operations, including mutations, for the Phase 1 spike
//! (specs/003-engine-design-spike, research R4 and R8). Compiled only with the `spike` feature,
//! so the shipped plugin compiles query operations only.
//!
//! The mutations are the ones the clients send (research R4): the semantic-tagging plugin's
//! single-item updates and the viewer's bulk updates, merges, and deletes. Every update here
//! changes custom fields only, which is what the seed and the scripted saves need.

use crate::{schema, Client, Error, JsonAny, JsonMap};
use serde_json::{Map, Value};

json_scalar_local!(BoolMap, BoolMap);

/// A JSON scalar defined in this module (the crate's own macro is private to `lib.rs`).
macro_rules! json_scalar_local {
    ($rust:ident, $gql:ident) => {
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $rust(pub serde_json::Value);
        cynic::impl_scalar!($rust, schema::$gql);
    };
}
use json_scalar_local;

// ---- Inputs

/// `CustomFieldsInput`: `full` replaces the map, `partial` updates the listed keys, `remove`
/// deletes keys.
#[derive(cynic::InputObject, Debug, Clone, Default)]
#[cynic(graphql_type = "CustomFieldsInput")]
pub struct CustomFields {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub full: Option<JsonMap>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub partial: Option<JsonMap>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub remove: Option<Vec<String>>,
}

impl CustomFields {
    pub fn partial(map: Map<String, Value>) -> CustomFields {
        CustomFields {
            partial: Some(JsonMap(Value::Object(map))),
            ..CustomFields::default()
        }
    }
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "TagCreateInput")]
pub struct TagCreateInput {
    pub name: String,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: Option<JsonMap>,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "TagUpdateInput")]
pub struct TagUpdateInput {
    pub id: cynic::Id,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: Option<CustomFields>,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "TagDestroyInput")]
pub struct TagDestroyInput {
    pub id: cynic::Id,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "TagsMergeInput")]
pub struct TagsMergeInput {
    pub source: Vec<cynic::Id>,
    pub destination: cynic::Id,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "BulkTagUpdateInput")]
pub struct BulkTagUpdateInput {
    pub ids: Option<Vec<cynic::Id>>,
    pub description: Option<String>,
}

macro_rules! update_inputs {
    ($single:ident, $single_gql:literal, $bulk:ident, $bulk_gql:literal) => {
        #[derive(cynic::InputObject, Debug)]
        #[cynic(graphql_type = $single_gql)]
        pub struct $single {
            pub id: cynic::Id,
            #[cynic(rename = "custom_fields")]
            pub custom_fields: Option<CustomFields>,
        }

        #[derive(cynic::InputObject, Debug)]
        #[cynic(graphql_type = $bulk_gql)]
        pub struct $bulk {
            pub ids: Option<Vec<cynic::Id>>,
            #[cynic(rename = "custom_fields")]
            pub custom_fields: Option<CustomFields>,
        }
    };
}
update_inputs!(
    SceneUpdateInput,
    "SceneUpdateInput",
    BulkSceneUpdateInput,
    "BulkSceneUpdateInput"
);
update_inputs!(
    ImageUpdateInput,
    "ImageUpdateInput",
    BulkImageUpdateInput,
    "BulkImageUpdateInput"
);
update_inputs!(
    PerformerUpdateInput,
    "PerformerUpdateInput",
    BulkPerformerUpdateInput,
    "BulkPerformerUpdateInput"
);

// ---- Selections

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
pub struct TagId {
    pub id: cynic::Id,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct SceneId {
    pub id: cynic::Id,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
pub struct ImageId {
    pub id: cynic::Id,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Performer")]
pub struct PerformerId {
    pub id: cynic::Id,
}

// ---- Mutations (written out: Cynic's derives cannot see through declarative macros)
#[derive(cynic::QueryVariables, Debug)]
pub struct TagCreateVars {
    pub input: TagCreateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TagCreateVars")]
pub struct TagCreate {
    #[arguments(input: $input)]
    pub tag_create: Option<TagId>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct TagUpdateVars {
    pub input: TagUpdateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TagUpdateVars")]
pub struct TagUpdate {
    #[arguments(input: $input)]
    pub tag_update: Option<TagId>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct TagDestroyVars {
    pub input: TagDestroyInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TagDestroyVars")]
pub struct TagDestroy {
    #[arguments(input: $input)]
    pub tag_destroy: bool,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct TagsDestroyVars {
    pub ids: Vec<cynic::Id>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TagsDestroyVars")]
pub struct TagsDestroy {
    #[arguments(ids: $ids)]
    pub tags_destroy: bool,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct TagsMergeVars {
    pub input: TagsMergeInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TagsMergeVars")]
pub struct TagsMerge {
    #[arguments(input: $input)]
    pub tags_merge: Option<TagId>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct BulkTagUpdateVars {
    pub input: BulkTagUpdateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "BulkTagUpdateVars")]
pub struct BulkTagUpdate {
    #[arguments(input: $input)]
    pub bulk_tag_update: Option<Vec<TagId>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct SceneUpdateVars {
    pub input: SceneUpdateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SceneUpdateVars")]
pub struct SceneUpdate {
    #[arguments(input: $input)]
    pub scene_update: Option<SceneId>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct BulkSceneUpdateVars {
    pub input: BulkSceneUpdateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "BulkSceneUpdateVars")]
pub struct BulkSceneUpdate {
    #[arguments(input: $input)]
    pub bulk_scene_update: Option<Vec<SceneId>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ImageUpdateVars {
    pub input: ImageUpdateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "ImageUpdateVars")]
pub struct ImageUpdate {
    #[arguments(input: $input)]
    pub image_update: Option<ImageId>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct BulkImageUpdateVars {
    pub input: BulkImageUpdateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "BulkImageUpdateVars")]
pub struct BulkImageUpdate {
    #[arguments(input: $input)]
    pub bulk_image_update: Option<Vec<ImageId>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct PerformerUpdateVars {
    pub input: PerformerUpdateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "PerformerUpdateVars")]
pub struct PerformerUpdate {
    #[arguments(input: $input)]
    pub performer_update: Option<PerformerId>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct BulkPerformerUpdateVars {
    pub input: BulkPerformerUpdateInput,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "BulkPerformerUpdateVars")]
pub struct BulkPerformerUpdate {
    #[arguments(input: $input)]
    pub bulk_performer_update: Option<Vec<PerformerId>>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct RunPluginOperationVars {
    pub plugin_id: cynic::Id,
    pub args: Option<JsonMap>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "RunPluginOperationVars")]
pub struct RunPluginOperation {
    #[arguments(plugin_id: $plugin_id, args: $args)]
    pub run_plugin_operation: Option<JsonAny>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct SetPluginsEnabledVars {
    pub enabled_map: BoolMap,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SetPluginsEnabledVars")]
pub struct SetPluginsEnabled {
    #[arguments(enabledMap: $enabled_map)]
    pub set_plugins_enabled: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation")]
pub struct ReloadPlugins {
    pub reload_plugins: bool,
}

// ---- Queries

#[derive(cynic::QueryFragment, Debug, Clone, Copy, PartialEq, Eq)]
#[cynic(graphql_type = "StatsResultType")]
pub struct Stats {
    #[cynic(rename = "scene_count")]
    pub scene_count: i32,
    #[cynic(rename = "image_count")]
    pub image_count: i32,
    #[cynic(rename = "performer_count")]
    pub performer_count: i32,
    #[cynic(rename = "tag_count")]
    pub tag_count: i32,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query")]
pub struct StatsQuery {
    pub stats: Stats,
}

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "FindFilterType")]
pub struct Page {
    pub page: Option<i32>,
    #[cynic(rename = "per_page")]
    pub per_page: Option<i32>,
    pub sort: Option<String>,
}

impl Page {
    /// All items, sorted by id.
    pub fn all() -> Page {
        Page {
            page: None,
            per_page: Some(-1),
            sort: Some("id".into()),
        }
    }
    /// The first `n` items by id.
    pub fn first(n: i32) -> Page {
        Page {
            page: Some(1),
            per_page: Some(n),
            sort: Some("id".into()),
        }
    }
}

#[derive(cynic::InputObject, Debug, Clone)]
#[cynic(graphql_type = "TimestampCriterionInput")]
pub struct Timestamp {
    pub value: String,
    pub value2: Option<String>,
    pub modifier: crate::fixtures::CriterionModifier,
}

macro_rules! filter_with_updated_at {
    ($name:ident, $gql:literal) => {
        #[derive(cynic::InputObject, Debug, Clone, Default)]
        #[cynic(graphql_type = $gql)]
        pub struct $name {
            #[cynic(rename = "updated_at")]
            pub updated_at: Option<Timestamp>,
        }
    };
}
filter_with_updated_at!(SceneUpdatedFilter, "SceneFilterType");
filter_with_updated_at!(ImageUpdatedFilter, "ImageFilterType");
filter_with_updated_at!(PerformerUpdatedFilter, "PerformerFilterType");
filter_with_updated_at!(TagUpdatedFilter, "TagFilterType");

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct SceneFields {
    pub id: cynic::Id,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Image")]
pub struct ImageFields {
    pub id: cynic::Id,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Performer")]
pub struct PerformerFields {
    pub id: cynic::Id,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
pub struct TagFields {
    pub id: cynic::Id,
    #[cynic(rename = "custom_fields")]
    pub custom_fields: JsonMap,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct FindScenesVars {
    pub filter: Option<SceneUpdatedFilter>,
    pub ids: Option<Vec<cynic::Id>>,
    pub page: Option<Page>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FindScenesVars")]
pub struct FindScenesQuery {
    #[arguments(scene_filter: $filter, ids: $ids, filter: $page)]
    pub find_scenes: FindScenesPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct FindScenesPage {
    pub count: i32,
    pub scenes: Vec<SceneFields>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct FindImagesVars {
    pub filter: Option<ImageUpdatedFilter>,
    pub ids: Option<Vec<cynic::Id>>,
    pub page: Option<Page>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FindImagesVars")]
pub struct FindImagesQuery {
    #[arguments(image_filter: $filter, ids: $ids, filter: $page)]
    pub find_images: FindImagesPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
pub struct FindImagesPage {
    pub count: i32,
    pub images: Vec<ImageFields>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct FindPerformersVars {
    pub filter: Option<PerformerUpdatedFilter>,
    pub ids: Option<Vec<cynic::Id>>,
    pub page: Option<Page>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FindPerformersVars")]
pub struct FindPerformersQuery {
    #[arguments(performer_filter: $filter, ids: $ids, filter: $page)]
    pub find_performers: FindPerformersPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindPerformersResultType")]
pub struct FindPerformersPage {
    pub count: i32,
    pub performers: Vec<PerformerFields>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct FindTagsVars {
    pub filter: Option<TagUpdatedFilter>,
    pub ids: Option<Vec<cynic::Id>>,
    pub page: Option<Page>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FindTagsVars")]
pub struct FindTagsQuery {
    #[arguments(tag_filter: $filter, ids: $ids, filter: $page)]
    pub find_tags: FindTagsPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindTagsResultType")]
pub struct FindTagsPage {
    pub count: i32,
    pub tags: Vec<TagFields>,
}

// ---- Client methods

/// The entity types the spike reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Tag,
    Scene,
    Image,
    Performer,
}

/// Item ids with their custom fields.
pub type Rows = Vec<(String, Map<String, Value>)>;

fn ids(v: &[String]) -> Vec<cynic::Id> {
    v.iter().map(cynic::Id::new).collect()
}

fn as_object(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

impl Client {
    pub fn stats(&self) -> Result<Stats, Error> {
        use cynic::QueryBuilder;
        Ok(self.run(StatsQuery::build(()))?.stats)
    }

    /// `runPluginOperation(plugin_id, args)`; returns the plugin's `output` as JSON.
    pub fn run_plugin_operation(
        &self,
        plugin_id: &str,
        args: Map<String, Value>,
    ) -> Result<Value, Error> {
        use cynic::MutationBuilder;
        let op = RunPluginOperation::build(RunPluginOperationVars {
            plugin_id: cynic::Id::new(plugin_id),
            args: Some(JsonMap(Value::Object(args))),
        });
        Ok(self
            .run(op)?
            .run_plugin_operation
            .map(|v| v.0)
            .unwrap_or(Value::Null))
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> Result<bool, Error> {
        use cynic::MutationBuilder;
        let mut m = Map::new();
        m.insert(plugin_id.into(), Value::Bool(enabled));
        let op = SetPluginsEnabled::build(SetPluginsEnabledVars {
            enabled_map: BoolMap(Value::Object(m)),
        });
        Ok(self.run(op)?.set_plugins_enabled)
    }

    pub fn reload_plugins(&self) -> Result<bool, Error> {
        use cynic::MutationBuilder;
        Ok(self.run(ReloadPlugins::build(()))?.reload_plugins)
    }

    /// Creates a tag with the given custom fields; returns its id.
    pub fn tag_create(
        &self,
        name: &str,
        custom_fields: Map<String, Value>,
    ) -> Result<String, Error> {
        use cynic::MutationBuilder;
        let op = TagCreate::build(TagCreateVars {
            input: TagCreateInput {
                name: name.into(),
                custom_fields: Some(JsonMap(Value::Object(custom_fields))),
            },
        });
        self.run(op)?
            .tag_create
            .map(|t| t.id.into_inner())
            .ok_or_else(|| Error::Graphql("tagCreate returned no tag".into()))
    }

    pub fn tag_update(&self, id: &str, cf: CustomFields) -> Result<(), Error> {
        use cynic::MutationBuilder;
        self.run(TagUpdate::build(TagUpdateVars {
            input: TagUpdateInput {
                id: cynic::Id::new(id),
                custom_fields: Some(cf),
            },
        }))?;
        Ok(())
    }

    pub fn tag_destroy(&self, id: &str) -> Result<bool, Error> {
        use cynic::MutationBuilder;
        Ok(self
            .run(TagDestroy::build(TagDestroyVars {
                input: TagDestroyInput {
                    id: cynic::Id::new(id),
                },
            }))?
            .tag_destroy)
    }

    pub fn tags_destroy(&self, tag_ids: &[String]) -> Result<bool, Error> {
        use cynic::MutationBuilder;
        Ok(self
            .run(TagsDestroy::build(TagsDestroyVars { ids: ids(tag_ids) }))?
            .tags_destroy)
    }

    pub fn tags_merge(&self, source: &[String], destination: &str) -> Result<(), Error> {
        use cynic::MutationBuilder;
        self.run(TagsMerge::build(TagsMergeVars {
            input: TagsMergeInput {
                source: ids(source),
                destination: cynic::Id::new(destination),
            },
        }))?;
        Ok(())
    }

    pub fn bulk_tag_update_description(
        &self,
        tag_ids: &[String],
        description: &str,
    ) -> Result<(), Error> {
        use cynic::MutationBuilder;
        self.run(BulkTagUpdate::build(BulkTagUpdateVars {
            input: BulkTagUpdateInput {
                ids: Some(ids(tag_ids)),
                description: Some(description.into()),
            },
        }))?;
        Ok(())
    }

    /// Updates one item's custom fields with the mutation its type uses.
    pub fn update_custom_fields(
        &self,
        kind: Kind,
        id: &str,
        cf: CustomFields,
    ) -> Result<(), Error> {
        use cynic::MutationBuilder;
        let id = cynic::Id::new(id);
        match kind {
            Kind::Tag => return self.tag_update(id.inner(), cf),
            Kind::Scene => {
                self.run(SceneUpdate::build(SceneUpdateVars {
                    input: SceneUpdateInput {
                        id,
                        custom_fields: Some(cf),
                    },
                }))?;
            }
            Kind::Image => {
                self.run(ImageUpdate::build(ImageUpdateVars {
                    input: ImageUpdateInput {
                        id,
                        custom_fields: Some(cf),
                    },
                }))?;
            }
            Kind::Performer => {
                self.run(PerformerUpdate::build(PerformerUpdateVars {
                    input: PerformerUpdateInput {
                        id,
                        custom_fields: Some(cf),
                    },
                }))?;
            }
        }
        Ok(())
    }

    /// Updates many items' custom fields with one bulk mutation. Tags have no bulk custom-field
    /// update in Stash v0.31.1, so for tags this sets a description instead (`bulkTagUpdate`).
    pub fn bulk_update_custom_fields(
        &self,
        kind: Kind,
        item_ids: &[String],
        cf: CustomFields,
    ) -> Result<(), Error> {
        use cynic::MutationBuilder;
        let ids = Some(ids(item_ids));
        match kind {
            Kind::Tag => return self.bulk_tag_update_description(item_ids, "spike bulk update"),
            Kind::Scene => {
                self.run(BulkSceneUpdate::build(BulkSceneUpdateVars {
                    input: BulkSceneUpdateInput {
                        ids,
                        custom_fields: Some(cf),
                    },
                }))?;
            }
            Kind::Image => {
                self.run(BulkImageUpdate::build(BulkImageUpdateVars {
                    input: BulkImageUpdateInput {
                        ids,
                        custom_fields: Some(cf),
                    },
                }))?;
            }
            Kind::Performer => {
                self.run(BulkPerformerUpdate::build(BulkPerformerUpdateVars {
                    input: BulkPerformerUpdateInput {
                        ids,
                        custom_fields: Some(cf),
                    },
                }))?;
            }
        }
        Ok(())
    }

    /// Ids and custom fields of items, filtered by `updated_at > since` (RFC 3339) and/or by
    /// ids, one page. Returns `(total count, rows)`.
    pub fn find_custom_fields(
        &self,
        kind: Kind,
        since: Option<&str>,
        item_ids: Option<&[String]>,
        page: Page,
    ) -> Result<(i32, Rows), Error> {
        use cynic::QueryBuilder;
        let ts = since.map(|s| Timestamp {
            value: s.into(),
            value2: None,
            modifier: crate::fixtures::CriterionModifier::GreaterThan,
        });
        let ids = item_ids.map(ids);
        let page = Some(page);
        macro_rules! rows {
            ($r:expr) => {
                $r.into_iter()
                    .map(|r| (r.id.into_inner(), as_object(r.custom_fields.0)))
                    .collect()
            };
        }
        Ok(match kind {
            Kind::Scene => {
                let r = self
                    .run(FindScenesQuery::build(FindScenesVars {
                        filter: ts.map(|t| SceneUpdatedFilter {
                            updated_at: Some(t),
                        }),
                        ids,
                        page,
                    }))?
                    .find_scenes;
                (r.count, rows!(r.scenes))
            }
            Kind::Image => {
                let r = self
                    .run(FindImagesQuery::build(FindImagesVars {
                        filter: ts.map(|t| ImageUpdatedFilter {
                            updated_at: Some(t),
                        }),
                        ids,
                        page,
                    }))?
                    .find_images;
                (r.count, rows!(r.images))
            }
            Kind::Performer => {
                let r = self
                    .run(FindPerformersQuery::build(FindPerformersVars {
                        filter: ts.map(|t| PerformerUpdatedFilter {
                            updated_at: Some(t),
                        }),
                        ids,
                        page,
                    }))?
                    .find_performers;
                (r.count, rows!(r.performers))
            }
            Kind::Tag => {
                let r = self
                    .run(FindTagsQuery::build(FindTagsVars {
                        filter: ts.map(|t| TagUpdatedFilter {
                            updated_at: Some(t),
                        }),
                        ids,
                        page,
                    }))?
                    .find_tags;
                (r.count, rows!(r.tags))
            }
        })
    }

    /// Every id of a type (one request, sorted by id): the id comparison that finds deletions.
    pub fn all_ids(&self, kind: Kind) -> Result<Vec<String>, Error> {
        Ok(self
            .find_custom_fields(kind, None, None, Page::all())?
            .1
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }
}

// ---- Play activity (the viewer's only Stash write today) and ids-only listings

#[derive(cynic::QueryVariables, Debug)]
pub struct SceneSaveActivityVars {
    pub id: cynic::Id,
    pub resume_time: Option<f64>,
    pub play_duration: Option<f64>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SceneSaveActivityVars")]
pub struct SceneSaveActivity {
    #[arguments(id: $id, resume_time: $resume_time, playDuration: $play_duration)]
    pub scene_save_activity: bool,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct IdsVars {
    pub page: Option<Page>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "IdsVars")]
pub struct SceneIdsQuery {
    #[arguments(filter: $page)]
    pub find_scenes: SceneIdsPage,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct SceneIdsPage {
    pub scenes: Vec<SceneId>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "IdsVars")]
pub struct ImageIdsQuery {
    #[arguments(filter: $page)]
    pub find_images: ImageIdsPage,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindImagesResultType")]
pub struct ImageIdsPage {
    pub images: Vec<ImageId>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "IdsVars")]
pub struct PerformerIdsQuery {
    #[arguments(filter: $page)]
    pub find_performers: PerformerIdsPage,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindPerformersResultType")]
pub struct PerformerIdsPage {
    pub performers: Vec<PerformerId>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "IdsVars")]
pub struct TagIdsQuery {
    #[arguments(filter: $page)]
    pub find_tags: TagIdsPage,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindTagsResultType")]
pub struct TagIdsPage {
    pub tags: Vec<TagId>,
}

impl Client {
    /// `sceneSaveActivity`: the play-position write the viewer makes while playing.
    pub fn scene_save_activity(
        &self,
        id: &str,
        resume_time: f64,
        play_duration: f64,
    ) -> Result<bool, Error> {
        use cynic::MutationBuilder;
        Ok(self
            .run(SceneSaveActivity::build(SceneSaveActivityVars {
                id: cynic::Id::new(id),
                resume_time: Some(resume_time),
                play_duration: Some(play_duration),
            }))?
            .scene_save_activity)
    }

    /// Every id of a type, selecting nothing but ids (the cheapest deletion check).
    pub fn ids_only(&self, kind: Kind) -> Result<Vec<String>, Error> {
        use cynic::QueryBuilder;
        let vars = || IdsVars {
            page: Some(Page::all()),
        };
        let ids: Vec<cynic::Id> = match kind {
            Kind::Scene => self
                .run(SceneIdsQuery::build(vars()))?
                .find_scenes
                .scenes
                .into_iter()
                .map(|r| r.id)
                .collect(),
            Kind::Image => self
                .run(ImageIdsQuery::build(vars()))?
                .find_images
                .images
                .into_iter()
                .map(|r| r.id)
                .collect(),
            Kind::Performer => self
                .run(PerformerIdsQuery::build(vars()))?
                .find_performers
                .performers
                .into_iter()
                .map(|r| r.id)
                .collect(),
            Kind::Tag => self
                .run(TagIdsQuery::build(vars()))?
                .find_tags
                .tags
                .into_iter()
                .map(|r| r.id)
                .collect(),
        };
        Ok(ids.into_iter().map(|i| i.into_inner()).collect())
    }
}

// ---- updated_at as Stash stores it (text in the server's local time, e.g.
// `2026-10-17T17:57:53-06:00`); filters compare against that text, so thresholds must use it.

json_scalar_local!(TimeText, Time);

#[derive(cynic::QueryVariables, Debug)]
pub struct UpdatedAtVars {
    pub ids: Option<Vec<cynic::Id>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Scene")]
pub struct SceneUpdatedAt {
    pub id: cynic::Id,
    #[cynic(rename = "updated_at")]
    pub updated_at: TimeText,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "FindScenesResultType")]
pub struct SceneUpdatedAtPage {
    pub scenes: Vec<SceneUpdatedAt>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "UpdatedAtVars")]
pub struct SceneUpdatedAtQuery {
    #[arguments(ids: $ids, filter: { per_page: 1, sort: "updated_at", direction: DESC })]
    pub find_scenes: SceneUpdatedAtPage,
}

impl Client {
    /// The most recently updated scene's `updated_at`, exactly as Stash stores it.
    pub fn latest_scene_updated_at(&self) -> Result<Option<String>, Error> {
        use cynic::QueryBuilder;
        Ok(self
            .run(SceneUpdatedAtQuery::build(UpdatedAtVars { ids: None }))?
            .find_scenes
            .scenes
            .into_iter()
            .next()
            .and_then(|s| s.updated_at.0.as_str().map(String::from)))
    }
}

// Plugin assets (research R6): Stash serves a plugin's `ui.assets` folders at
// `/plugin/{id}/assets/...`, behind its normal authentication.

/// A fetched plugin asset: the HTTP status and the body.
#[derive(Debug, Clone)]
pub struct Asset {
    pub status: u16,
    pub body: Vec<u8>,
}

fn get_asset(http: &reqwest::blocking::Client, url: &str) -> Result<Asset, Error> {
    let resp = http
        .get(url)
        .send()
        .map_err(|e| Error::Unreachable(e.to_string()))?;
    let status = resp.status().as_u16();
    let body = resp
        .bytes()
        .map_err(|e| Error::Unreachable(e.to_string()))?
        .to_vec();
    Ok(Asset { status, body })
}

impl Client {
    fn asset_url(&self, plugin_id: &str, path: &str) -> String {
        format!(
            "{}/plugin/{plugin_id}/assets/{}",
            self.endpoint.trim_end_matches("/graphql"),
            path.trim_start_matches('/')
        )
    }

    /// `GET /plugin/{plugin_id}/assets/{path}` with this client's credentials.
    pub fn plugin_asset(&self, plugin_id: &str, path: &str) -> Result<Asset, Error> {
        get_asset(&self.http, &self.asset_url(plugin_id, path))
    }

    /// The same request with no credentials, to check that Stash refuses it. Redirects are not
    /// followed, so a redirect to the login page shows as its 3xx status.
    pub fn plugin_asset_without_credentials(
        &self,
        plugin_id: &str,
        path: &str,
    ) -> Result<Asset, Error> {
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| Error::Setup(e.to_string()))?;
        get_asset(&http, &self.asset_url(plugin_id, path))
    }
}
