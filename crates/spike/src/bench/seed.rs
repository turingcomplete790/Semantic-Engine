//! `seed` and `cleanup`: reversible semantic test data on the test server (research R8,
//! data-model "Seed manifest").
//!
//! Before any write the manifest records what will change, so a crash part-way leaves a
//! manifest that cleanup can still reverse.

use super::{Bench, Exit, Failure, Outcome};
use crate::measure::now_rfc3339;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use stash_adapter::spike::{CustomFields, Kind, Page, Stats};
use stash_adapter::Client;
use std::fs;
use std::path::{Path, PathBuf};

/// The flagged tag types the seed creates, `TAGS_PER_TYPE` of each (vocabulary
/// `stash-mapping.ttl`; the same flags as `stash_adapter::fixtures::TAG_FLAGS`).
pub const TAG_FLAGS: [&str; 7] = stash_adapter::fixtures::TAG_FLAGS;
pub const TAGS_PER_TYPE: usize = 6;
/// Custom-field keys the seed writes on scenes and images.
pub const SCENE_KEYS: [&str; 3] = [
    "roleTagAssignments",
    "actionAssignments",
    "pluggableAssignments",
];
pub const IMAGE_KEYS: [&str; 1] = ["roleTagAssignments"];
/// Per-scene shape, matched by `synth`: 2 performers, each with 1 role and 1 action (one
/// variation), and a pluggable for the first performer. Images: 1 performer with 1 role.
pub const SCENE_SHAPE: &str = "2 performers; 1 role + 1 action each; 1 pluggable on the first";
const VARIATIONS: [&str; 4] = ["slow", "fast", "playful", "intense"];
/// Items recorded in the manifest before each batch of writes.
const BATCH: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeedState {
    Seeding,
    Seeded,
    Cleaned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsRecord {
    pub scenes: i64,
    pub images: i64,
    pub performers: i64,
    pub tags: i64,
}

impl From<Stats> for StatsRecord {
    fn from(s: Stats) -> Self {
        StatsRecord {
            scenes: s.scene_count.into(),
            images: s.image_count.into(),
            performers: s.performer_count.into(),
            tags: s.tag_count.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatedTag {
    pub id: String,
    pub flag: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    #[serde(rename = "type")]
    pub kind: Kind,
    pub id: String,
    pub custom_fields_before: Map<String, Value>,
    /// The keys the seed writes on this item.
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedManifest {
    pub server: String,
    pub stats_before: StatsRecord,
    pub created_tags: Vec<CreatedTag>,
    pub changed: Vec<Changed>,
    pub state: SeedState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeded_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleaned_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats_after_cleanup: Option<StatsRecord>,
}

#[derive(Debug)]
pub enum SeedError {
    /// Refused: another seed is in place, no manifest, or the wrong server (exit 3).
    Refused(String),
    Stash(stash_adapter::Error),
    Io(String),
}

impl std::fmt::Display for SeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeedError::Refused(m) | SeedError::Io(m) => write!(f, "{m}"),
            SeedError::Stash(e) => write!(f, "{e}"),
        }
    }
}

impl From<stash_adapter::Error> for SeedError {
    fn from(e: stash_adapter::Error) -> Self {
        SeedError::Stash(e)
    }
}

impl From<SeedError> for Failure {
    fn from(e: SeedError) -> Self {
        match e {
            SeedError::Refused(m) => Failure::new(Exit::Refused, m),
            SeedError::Stash(e) => Failure::from(e),
            SeedError::Io(m) => Failure::new(Exit::RunFailed, m),
        }
    }
}

pub fn manifest_path(out: &Path) -> PathBuf {
    out.join("seed-manifest.json")
}

pub fn load_manifest(path: &Path) -> Result<Option<SeedManifest>, SeedError> {
    match fs::read_to_string(path) {
        Ok(t) => serde_json::from_str(&t)
            .map(Some)
            .map_err(|e| SeedError::Io(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(SeedError::Io(e.to_string())),
    }
}

/// Writes the manifest atomically (temporary file, then rename).
pub fn save_manifest(path: &Path, m: &SeedManifest) -> Result<(), SeedError> {
    let io = |e: std::io::Error| SeedError::Io(format!("{}: {e}", path.display()));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(m).expect("manifest serializes") + "\n";
    fs::write(&tmp, text).map_err(io)?;
    fs::rename(&tmp, path).map_err(io)
}

fn ids_of(
    client: &Client,
    kind: Kind,
    page: Page,
) -> Result<stash_adapter::spike::Rows, SeedError> {
    Ok(client.find_custom_fields(kind, None, None, page)?.1)
}

fn encode(v: Value) -> Value {
    Value::String(v.to_string())
}

/// The custom fields the seed writes on scene number `i` (role, action, pluggable tag ids by
/// flag, performers available).
fn scene_fields(i: usize, tags: &Map<String, Value>, performers: &[String]) -> Map<String, Value> {
    let pick = |flag: &str, k: usize| -> String {
        let list = tags[flag].as_array().expect("tags by flag");
        list[k % list.len()].as_str().unwrap().to_string()
    };
    let a = &performers[i % performers.len()];
    let b = &performers[(i + 1) % performers.len()];
    let mut m = Map::new();
    m.insert(
        "roleTagAssignments".into(),
        encode(json!({ a: [pick("isRole", i)], b: [pick("isRole", i + 1)] })),
    );
    m.insert(
        "actionAssignments".into(),
        encode(json!({
            a: [{ "actionTagId": pick("isAction", i), "variations": [VARIATIONS[i % 4]] }],
            b: [{ "actionTagId": pick("isAction", i + 2), "variations": [VARIATIONS[(i + 1) % 4]] }],
        })),
    );
    m.insert(
        "pluggableAssignments".into(),
        encode(json!({ a: [pick("plugtag", i)] })),
    );
    m
}

fn image_fields(i: usize, tags: &Map<String, Value>, performers: &[String]) -> Map<String, Value> {
    let roles = tags["isRole"].as_array().unwrap();
    let p = &performers[i % performers.len()];
    let mut m = Map::new();
    m.insert(
        "roleTagAssignments".into(),
        encode(json!({ p: [roles[i % roles.len()]] })),
    );
    m
}

/// Seeds `scenes` scenes and `images` images on `server`, recording everything in the manifest
/// at `path` before writing it.
pub fn seed(
    client: &Client,
    path: &Path,
    server: &str,
    scenes: usize,
    images: usize,
) -> Result<SeedManifest, SeedError> {
    if let Some(old) = load_manifest(path)? {
        if old.state != SeedState::Cleaned {
            return Err(SeedError::Refused(format!(
                "a seed is already in place ({:?}); run `spike-bench cleanup` first",
                old.state
            )));
        }
        let kept = path.with_file_name(format!(
            "seed-manifest-{}.json",
            crate::measure::timestamp_basic()
        ));
        fs::rename(path, &kept).map_err(|e| SeedError::Io(e.to_string()))?;
    }

    let mut m = SeedManifest {
        server: server.into(),
        stats_before: client.stats()?.into(),
        created_tags: Vec::new(),
        changed: Vec::new(),
        state: SeedState::Seeding,
        seeded_at: None,
        cleaned_at: None,
        stats_after_cleanup: None,
    };
    save_manifest(path, &m)?;

    // Flagged tags; the manifest is saved after each so a crash leaks at most one.
    let mut by_flag = Map::new();
    for flag in TAG_FLAGS {
        let mut ids = Vec::new();
        for n in 0..TAGS_PER_TYPE {
            let mut cf = Map::new();
            cf.insert(flag.into(), json!(1));
            let id = client.tag_create(&format!("SPIKE {flag} {n}"), cf)?;
            m.created_tags.push(CreatedTag {
                id: id.clone(),
                flag: flag.into(),
            });
            save_manifest(path, &m)?;
            ids.push(Value::String(id));
        }
        by_flag.insert(flag.into(), Value::Array(ids));
    }

    let performers: Vec<String> = ids_of(client, Kind::Performer, Page::all())?
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    if performers.len() < 2 {
        return Err(SeedError::Io(format!(
            "the library has {} performers; the seed needs at least 2",
            performers.len()
        )));
    }

    for (kind, count, keys) in [
        (Kind::Scene, scenes, &SCENE_KEYS[..]),
        (Kind::Image, images, &IMAGE_KEYS[..]),
    ] {
        if count == 0 {
            continue;
        }
        let items = ids_of(client, kind, Page::first(count as i32))?;
        for (b, batch) in items.chunks(BATCH).enumerate() {
            for (id, before) in batch {
                m.changed.push(Changed {
                    kind,
                    id: id.clone(),
                    custom_fields_before: before.clone(),
                    keys: keys.iter().map(|k| k.to_string()).collect(),
                });
            }
            save_manifest(path, &m)?;
            for (j, (id, _)) in batch.iter().enumerate() {
                let i = b * BATCH + j;
                let fields = match kind {
                    Kind::Scene => scene_fields(i, &by_flag, &performers),
                    _ => image_fields(i, &by_flag, &performers),
                };
                client.update_custom_fields(kind, id, CustomFields::partial(fields))?;
            }
        }
    }

    m.state = SeedState::Seeded;
    m.seeded_at = Some(now_rfc3339());
    save_manifest(path, &m)?;
    Ok(m)
}

/// Reverses a seed: restores every recorded item's custom fields, deletes the created tags, and
/// marks the manifest `cleaned`. Works on a `seeding` manifest too (a crash mid-seed).
pub fn cleanup(client: &Client, path: &Path, server: &str) -> Result<SeedManifest, SeedError> {
    let mut m = load_manifest(path)?.ok_or_else(|| {
        SeedError::Refused(format!(
            "no seed manifest at {}; nothing to clean up",
            path.display()
        ))
    })?;
    if m.server != server {
        return Err(SeedError::Refused(format!(
            "the seed was made on {}, not {server}; refusing to clean up here",
            m.server
        )));
    }
    if m.state == SeedState::Cleaned {
        return Ok(m);
    }

    for c in m.changed.iter().rev() {
        let mut partial = Map::new();
        let mut remove = Vec::new();
        for k in &c.keys {
            match c.custom_fields_before.get(k) {
                Some(v) => {
                    partial.insert(k.clone(), v.clone());
                }
                None => remove.push(k.clone()),
            }
        }
        let cf = CustomFields {
            partial: (!partial.is_empty())
                .then_some(stash_adapter::JsonMap(Value::Object(partial))),
            remove: (!remove.is_empty()).then_some(remove),
            ..CustomFields::default()
        };
        client.update_custom_fields(c.kind, &c.id, cf)?;
    }
    let tag_ids: Vec<String> = m.created_tags.iter().map(|t| t.id.clone()).collect();
    if !tag_ids.is_empty() {
        client.tags_destroy(&tag_ids)?;
    }

    m.state = SeedState::Cleaned;
    m.cleaned_at = Some(now_rfc3339());
    m.stats_after_cleanup = Some(client.stats()?.into());
    save_manifest(path, &m)?;
    Ok(m)
}

pub fn seed_command(b: &mut Bench) -> Outcome {
    let scenes = b.opt_u64("--scenes")?.unwrap_or(500) as usize;
    let images = b.opt_u64("--images")?.unwrap_or(500) as usize;
    let path = manifest_path(&b.global.out);
    let m = seed(b.client()?, &path, &b.url, scenes, images)?;
    b.say(format!(
        "seeded {} tags and {} items; manifest {} (state: seeded)",
        m.created_tags.len(),
        m.changed.len(),
        path.display()
    ));
    Ok(())
}

pub fn cleanup_command(b: &mut Bench) -> Outcome {
    let path = manifest_path(&b.global.out);
    let m = cleanup(b.client()?, &path, &b.url)?;
    let after = m.stats_after_cleanup;
    b.say(format!(
        "cleaned {} items and {} tags; stats before {:?}, after {:?}",
        m.changed.len(),
        m.created_tags.len(),
        m.stats_before,
        after
    ));
    if after != Some(m.stats_before) {
        return Err(Failure::new(
            Exit::RunFailed,
            "stats after cleanup differ from stats before the seed (SC-005)",
        ));
    }
    Ok(())
}
