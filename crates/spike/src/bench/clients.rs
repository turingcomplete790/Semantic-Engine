//! The saves each client makes, scripted (research R4, tasks T028).
//!
//! - **web-ui**: Stash's own edit forms; single-item `tagUpdate`, `sceneUpdate`, `imageUpdate`,
//!   `performerUpdate`. The forms send many fields; here only a custom field changes, which is
//!   enough to fire the same hook.
//! - **semantic-tagging**: the plugin's own payloads (custom-field partial updates:
//!   `src/ui/roles.js`, `traits.js`, `pluggables.js`, `matrix.js`): tag flags, scene and image
//!   assignment maps, the performer's `directTraitVariations`.
//! - **viewer**: still in progress and sends no semantic saves yet. Its only Stash write is play
//!   activity (`sceneSaveActivity`), recorded as the viewer's scene row. For tags, images, and
//!   performers the row records that nothing is sent; the "viewer (planned)" rows run the
//!   equivalent request the web UI makes, so the hook cost is known when the viewer starts
//!   saving.
//!
//! Every save touches only seeded items, or tags the harness creates, all recorded in the seed
//! manifest so cleanup reverses them.

use crate::measure::{Entity, SavingClient};
use serde_json::{json, Map, Value};
use stash_adapter::spike::{CustomFields, Kind};
use stash_adapter::{Client, Error};

/// The custom field the web-UI rows change (reversed by cleanup like the seeded keys).
pub const TOUCH: &str = "spikeTouch";
/// The performer field the semantic-tagging plugin writes (`DIRECT_TRAIT_VARIATIONS_FIELD_KEY`).
pub const DIRECT_TRAIT_VARIATIONS: &str = "directTraitVariations";

/// What a scripted save does.
#[derive(Debug, Clone)]
pub enum Action {
    Update {
        kind: Kind,
        id: String,
        fields: Map<String, Value>,
    },
    Bulk {
        kind: Kind,
        ids: Vec<String>,
        fields: Map<String, Value>,
    },
    TagsMerge {
        source: Vec<String>,
        destination: String,
    },
    TagsDestroy {
        ids: Vec<String>,
    },
    TagDestroy {
        id: String,
    },
    SaveActivity {
        id: String,
    },
    /// The client sends no save for this type.
    Nothing,
}

#[derive(Debug, Clone)]
pub struct Save {
    pub entity: Entity,
    pub client: SavingClient,
    pub mutation: String,
    pub items: u64,
    pub action: Action,
    pub note: Option<String>,
}

/// The items the scripted saves use.
#[derive(Debug, Clone)]
pub struct Targets {
    pub tag: String,
    pub scene: String,
    pub image: String,
    pub performer: String,
    /// A seeded role tag and performer ids, for assignment payloads.
    pub role_tag: String,
    pub performers: Vec<String>,
}

pub fn kind_of(e: Entity) -> Kind {
    match e {
        Entity::Tag => Kind::Tag,
        Entity::Scene => Kind::Scene,
        Entity::Image => Kind::Image,
        Entity::Performer => Kind::Performer,
    }
}

fn one(key: &str, value: Value) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(key.into(), value);
    m
}

fn mutation_for(kind: Kind) -> &'static str {
    match kind {
        Kind::Tag => "tagUpdate",
        Kind::Scene => "sceneUpdate",
        Kind::Image => "imageUpdate",
        Kind::Performer => "performerUpdate",
    }
}

/// The single-item saves for every entity × client pair, `n` varying the written value.
pub fn scripted(t: &Targets, n: u64) -> Vec<Save> {
    let id_of = |e: Entity| match e {
        Entity::Tag => t.tag.clone(),
        Entity::Scene => t.scene.clone(),
        Entity::Image => t.image.clone(),
        Entity::Performer => t.performer.clone(),
    };
    let entities = [Entity::Tag, Entity::Scene, Entity::Image, Entity::Performer];
    let mut out = Vec::new();

    for e in entities {
        let kind = kind_of(e);
        out.push(Save {
            entity: e,
            client: SavingClient::WebUi,
            mutation: mutation_for(kind).into(),
            items: 1,
            action: Action::Update {
                kind,
                id: id_of(e),
                fields: one(TOUCH, json!(n)),
            },
            note: Some("edit form; only a custom field changes here".into()),
        });
    }

    let (a, b) = (&t.performers[0], &t.performers[1 % t.performers.len()]);
    let assignments = json!({ a: [t.role_tag], b: [t.role_tag] }).to_string();
    let plugin_fields = |e: Entity| match e {
        Entity::Tag => one("isTrait", json!(1)),
        Entity::Scene | Entity::Image => one("roleTagAssignments", json!(assignments)),
        Entity::Performer => one(
            DIRECT_TRAIT_VARIATIONS,
            json!(json!({ "n": n }).to_string()),
        ),
    };
    for e in entities {
        let kind = kind_of(e);
        out.push(Save {
            entity: e,
            client: SavingClient::SemanticTagging,
            mutation: mutation_for(kind).into(),
            items: 1,
            action: Action::Update {
                kind,
                id: id_of(e),
                fields: plugin_fields(e),
            },
            note: Some("the plugin's custom_fields.partial payload".into()),
        });
    }

    for e in entities {
        let kind = kind_of(e);
        if e == Entity::Scene {
            out.push(Save {
                entity: e,
                client: SavingClient::Viewer,
                mutation: "sceneSaveActivity".into(),
                items: 1,
                action: Action::SaveActivity {
                    id: t.scene.clone(),
                },
                note: Some(
                    "the viewer's only Stash write today (play activity); not a semantic save"
                        .into(),
                ),
            });
        } else {
            out.push(Save {
                entity: e,
                client: SavingClient::Viewer,
                mutation: "(none)".into(),
                items: 0,
                action: Action::Nothing,
                note: Some("the viewer is in progress and sends no saves for this type yet".into()),
            });
        }
        out.push(Save {
            entity: e,
            client: SavingClient::Viewer,
            mutation: mutation_for(kind).into(),
            items: 1,
            action: Action::Update {
                kind,
                id: id_of(e),
                fields: one(TOUCH, json!(n + 1)),
            },
            note: Some("viewer (planned): the equivalent request the web UI makes".into()),
        });
    }
    out
}

/// Runs a save's request(s).
pub fn run(client: &Client, a: &Action) -> Result<(), Error> {
    match a {
        Action::Update { kind, id, fields } => {
            client.update_custom_fields(*kind, id, CustomFields::partial(fields.clone()))
        }
        Action::Bulk { kind, ids, fields } => {
            client.bulk_update_custom_fields(*kind, ids, CustomFields::partial(fields.clone()))
        }
        Action::TagsMerge {
            source,
            destination,
        } => client.tags_merge(source, destination),
        Action::TagsDestroy { ids } => client.tags_destroy(ids).map(|_| ()),
        Action::TagDestroy { id } => client.tag_destroy(id).map(|_| ()),
        Action::SaveActivity { id } => client.scene_save_activity(id, 0.0, 0.0).map(|_| ()),
        Action::Nothing => Ok(()),
    }
}

/// Whether a save can be repeated (merges and deletes consume their tags).
pub fn repeatable(a: &Action) -> bool {
    !matches!(
        a,
        Action::TagsMerge { .. } | Action::TagsDestroy { .. } | Action::TagDestroy { .. }
    )
}
