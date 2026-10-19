//! `hooks`: which saves reach the plugin as hooks, how fast, and what they add to each save
//! (spec FR-006, FR-007, research R4, tasks T029–T031).
//!
//! - `hooks`: the scripted single-item saves for every entity × client pair (`clients.rs`),
//!   plus a tag merge, a `tagsDestroy`, and a `tagDestroy`.
//! - `hooks --bulk N`: one `bulkSceneUpdate` of N scenes.
//! - `hooks --manual --client C [--seconds S | --since T]`: listens while the owner saves from a
//!   real client (or takes the hooks received since `T`, before `--until U`, RFC 3339 UTC) and logs each one as a
//!   manual row.
//!
//! Each save is timed `--repeat` times (default 5) with the spike plugin enabled, then again
//! with it disabled (`setPluginsEnabled`) for the no-hook latency; the plugin is always
//! re-enabled. Hooks are read from the spike plugin's `hooks.jsonl` (`recordHook`), and rows
//! are merged into `results/hooks.json`.

use super::clients::{self, Action, Save, Targets, TOUCH};
use super::seed::{self, Changed, CreatedTag, SeedManifest, SeedState};
use super::{Bench, Exit, Failure, Outcome};
use crate::measure::{
    now_rfc3339, offset_of, percentile, stash_time, Entity, HookObservation, HookReceived, Mode,
    SavingClient,
};
use crate::plugin_ops::hooks::{epoch_ms, LOG};
use crate::plugin_ops::PLUGIN_ID;
use serde_json::{json, Map, Value};
use stash_adapter::spike::{Kind, Page};
use stash_adapter::Client;
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

pub const TABLE: &str = "hooks.json";

fn fail(e: impl std::fmt::Display) -> Failure {
    Failure::new(Exit::RunFailed, e.to_string())
}

fn log_len(log: &Path) -> u64 {
    fs::metadata(log).map(|m| m.len()).unwrap_or(0)
}

/// Hook lines appended to the log after `offset`.
pub fn read_new(log: &Path, offset: u64) -> Vec<HookReceived> {
    let Ok(mut f) = fs::File::open(log) else {
        return Vec::new();
    };
    let mut text = String::new();
    if f.seek(SeekFrom::Start(offset)).is_err() || f.read_to_string(&mut text).is_err() {
        return Vec::new();
    }
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|v| HookReceived {
            hook_type: v["type"].as_str().unwrap_or("").to_string(),
            id: v["id"].as_i64(),
            received_ms: v["receivedMs"].as_f64().unwrap_or(0.0),
            handler_ms: v["handlerMs"].as_f64().unwrap_or(0.0),
            input_fields: v["inputFields"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            input_bytes: v["inputBytes"].as_u64().unwrap_or(0),
            received_at: v["receivedAt"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

/// Hooks run synchronously inside the save (research R4), so they are in the log when the save
/// returns; a short grace period catches anything slower and proves that nothing else arrives.
fn hooks_after(log: &Path, offset: u64) -> Vec<HookReceived> {
    thread::sleep(Duration::from_millis(150));
    read_new(log, offset)
}

/// Sleeps past the next second boundary and returns an `updated_at` threshold (in Stash's own
/// local-time text form) that excludes every earlier save but includes the next one: Stash
/// stores `updated_at` to the second and compares filter values as text.
fn fresh_second(offset: i64) -> String {
    thread::sleep(Duration::from_millis(1100));
    let now = (epoch_ms() / 1000.0).floor() as u64;
    stash_time(now - 1, offset)
}

/// The Stash server's UTC offset, read from its own `updated_at` text.
pub fn server_offset(client: &Client) -> Result<i64, Failure> {
    let stamp = client.latest_scene_updated_at()?.ok_or_else(|| {
        Failure::new(
            Exit::RunFailed,
            "the library has no scenes to read a timestamp from",
        )
    })?;
    offset_of(&stamp).ok_or_else(|| {
        Failure::new(
            Exit::RunFailed,
            format!("cannot read the UTC offset of {stamp:?}"),
        )
    })
}

fn median(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    Some(percentile(&s, 50.0))
}

/// Re-enables the spike plugin when dropped, so a failure never leaves it disabled.
struct Enabled<'a>(&'a Client);
impl Drop for Enabled<'_> {
    fn drop(&mut self) {
        let _ = self.0.set_plugin_enabled(PLUGIN_ID, true);
    }
}

struct Run<'a> {
    client: &'a Client,
    log: PathBuf,
    manifest_path: PathBuf,
    manifest: SeedManifest,
    /// The server's UTC offset, for `updated_at` thresholds.
    offset: i64,
}

impl Run<'_> {
    fn save_manifest(&self) -> Result<(), Failure> {
        seed::save_manifest(&self.manifest_path, &self.manifest).map_err(Failure::from)
    }

    /// Records items (and the key about to be written) in the seed manifest before writing.
    fn ensure_recorded(&mut self, kind: Kind, ids: &[String], key: &str) -> Result<(), Failure> {
        if kind == Kind::Tag {
            return Ok(()); // the harness only touches tags it created, which cleanup deletes
        }
        let mut missing = Vec::new();
        for id in ids {
            match self
                .manifest
                .changed
                .iter_mut()
                .find(|c| c.kind == kind && &c.id == id)
            {
                Some(c) => {
                    if !c.keys.iter().any(|k| k == key) {
                        c.keys.push(key.into());
                    }
                }
                None => missing.push(id.clone()),
            }
        }
        for chunk in missing.chunks(200) {
            let (_, rows) = self
                .client
                .find_custom_fields(kind, None, Some(chunk), Page::all())?;
            for (id, before) in rows {
                self.manifest.changed.push(Changed {
                    kind,
                    id,
                    custom_fields_before: before,
                    keys: vec![key.into()],
                });
            }
        }
        self.save_manifest()
    }

    fn record_save(&mut self, s: &Save) -> Result<(), Failure> {
        match &s.action {
            Action::Update { kind, id, fields } => {
                for k in fields.keys() {
                    self.ensure_recorded(*kind, std::slice::from_ref(id), k)?;
                }
            }
            Action::Bulk { kind, ids, fields } => {
                for k in fields.keys() {
                    self.ensure_recorded(*kind, ids, k)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Creates `n` throwaway tags, recorded in the manifest before they are used.
    fn new_tags(&mut self, n: usize, label: &str) -> Result<Vec<String>, Failure> {
        let mut ids = Vec::new();
        for i in 0..n {
            let id = self.client.tag_create(
                &format!("SPIKE {label} {} {i}", epoch_ms() as u64),
                Map::new(),
            )?;
            self.manifest.created_tags.push(CreatedTag {
                id: id.clone(),
                flag: label.into(),
            });
            ids.push(id);
        }
        self.save_manifest()?;
        Ok(ids)
    }

    /// Removes tags a merge or delete consumed from the manifest, so cleanup does not try them.
    fn forget_tags(&mut self, ids: &[String]) -> Result<(), Failure> {
        self.manifest.created_tags.retain(|t| !ids.contains(&t.id));
        self.save_manifest()
    }

    /// Times one save and reads the hooks it produced. Returns (latency ms, hooks, delay ms).
    fn once(&self, action: &Action) -> Result<(f64, Vec<HookReceived>, Option<f64>), Failure> {
        let offset = log_len(&self.log);
        let sent = epoch_ms();
        let t = Instant::now();
        clients::run(self.client, action)?;
        let latency = t.elapsed().as_secs_f64() * 1000.0;
        let hooks = hooks_after(&self.log, offset);
        let delay = hooks
            .iter()
            .map(|h| h.received_ms)
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
            .map(|last| last - sent);
        Ok((latency, hooks, delay))
    }
}

fn expected_ids(action: &Action) -> Vec<String> {
    match action {
        Action::Update { id, .. } => vec![id.clone()],
        Action::Bulk { ids, .. } => ids.clone(),
        Action::TagsMerge { destination, .. } => vec![destination.clone()],
        Action::TagsDestroy { ids } => ids.clone(),
        Action::TagDestroy { id } => vec![id.clone()],
        Action::SaveActivity { .. } | Action::Nothing => vec![],
    }
}

fn bumped(
    client: &Client,
    entity: Entity,
    action: &Action,
    since: &str,
) -> Result<Option<bool>, Failure> {
    let ids = match action {
        Action::Update { id, .. } | Action::SaveActivity { id } => vec![id.clone()],
        Action::Bulk { ids, .. } => ids.clone(),
        // Merged-away and deleted tags have nothing left to filter on (deletions need an id
        // comparison: `catch-up`).
        _ => return Ok(None),
    };
    // The `updated_at` filter is ignored when `ids` is also given, so list what the filter
    // returns and look for the saved items in it.
    let kind = clients::kind_of(entity);
    let (_, rows) = client.find_custom_fields(kind, Some(since), None, Page::all())?;
    let found: BTreeSet<&String> = rows.iter().map(|(id, _)| id).collect();
    Ok(Some(ids.iter().all(|i| found.contains(i))))
}

/// Observes one save: `repeats` timed runs with the hook enabled (the first one's hooks form
/// the row). The no-hook latency is filled in later.
fn observe(run: &mut Run, s: &Save, repeats: usize) -> Result<HookObservation, Failure> {
    let mut row = HookObservation {
        entity: s.entity,
        client: s.client,
        mode: Mode::Scripted,
        mutation: s.mutation.clone(),
        items: s.items,
        hooks_received: vec![],
        delay: None,
        save_latency: None,
        save_latency_no_hook: None,
        updated_at_bumped: None,
        missed: false,
        out_of_order: false,
        repeats: 0,
        note: s.note.clone(),
    };
    if matches!(s.action, Action::Nothing) {
        return Ok(row);
    }
    run.record_save(s)?;
    let since = fresh_second(run.offset);
    let times = if clients::repeatable(&s.action) {
        repeats
    } else {
        1
    };
    let mut lats = Vec::new();
    for r in 0..times {
        let (lat, hooks, delay) = run.once(&s.action)?;
        lats.push(lat);
        if r == 0 {
            row.hooks_received = hooks;
            row.delay = delay;
        }
    }
    row.repeats = times as u64;
    row.save_latency = median(&lats);
    row.updated_at_bumped = bumped(run.client, s.entity, &s.action, &since)?;
    let want = expected_ids(&s.action);
    let got: Vec<String> = row
        .hooks_received
        .iter()
        .filter_map(|h| h.id.map(|i| i.to_string()))
        .collect();
    let got_set: BTreeSet<&String> = got.iter().collect();
    row.missed = want.iter().any(|w| !got_set.contains(w));
    let in_want: Vec<&String> = got.iter().filter(|g| want.contains(g)).collect();
    row.out_of_order = in_want.windows(2).any(|w| {
        let pos = |x: &String| want.iter().position(|y| y == x);
        pos(w[0]) > pos(w[1])
    });
    Ok(row)
}

/// The no-hook latency of each row: the same saves with the spike plugin disabled.
fn no_hook_pass(
    run: &mut Run,
    saves: &[Save],
    rows: &mut [HookObservation],
    repeats: usize,
) -> Outcome {
    let client = run.client;
    client.set_plugin_enabled(PLUGIN_ID, false)?;
    let _guard = Enabled(client);
    for (s, row) in saves.iter().zip(rows.iter_mut()) {
        let action = match &s.action {
            Action::Nothing => continue,
            Action::TagsMerge { .. } => {
                let ids = run.new_tags(2, "merge-nohook")?;
                let a = Action::TagsMerge {
                    source: vec![ids[0].clone()],
                    destination: ids[1].clone(),
                };
                run.forget_tags(&ids[..1])?;
                a
            }
            Action::TagsDestroy { ids } => {
                let fresh = run.new_tags(ids.len(), "destroy-nohook")?;
                run.forget_tags(&fresh)?;
                Action::TagsDestroy { ids: fresh }
            }
            Action::TagDestroy { .. } => {
                let fresh = run.new_tags(1, "destroy1-nohook")?;
                run.forget_tags(&fresh)?;
                Action::TagDestroy {
                    id: fresh[0].clone(),
                }
            }
            other => other.clone(),
        };
        let times = if clients::repeatable(&action) {
            repeats
        } else {
            1
        };
        let mut lats = Vec::new();
        for _ in 0..times {
            let t = Instant::now();
            clients::run(client, &action)?;
            lats.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        row.save_latency_no_hook = median(&lats);
    }
    Ok(())
}

fn row_key(r: &HookObservation) -> String {
    format!(
        "{:?}|{:?}|{:?}|{}|{}|{}",
        r.entity,
        r.client,
        r.mode,
        r.mutation,
        r.items,
        r.note.as_deref().unwrap_or("")
    )
}

/// Merges rows into `results/hooks.json`: a row with the same entity, client, mode, mutation,
/// item count, and note replaces the old one; manual rows are always added.
pub fn merge_table(
    out: &Path,
    rows: &[HookObservation],
    secrets: &[&str],
) -> Result<Vec<HookObservation>, Failure> {
    let path = out.join(TABLE);
    let mut all: Vec<HookObservation> = fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| serde_json::from_value(v["rows"].clone()).ok())
        .unwrap_or_default();
    for r in rows {
        if r.mode == Mode::Scripted {
            all.retain(|o| row_key(o) != row_key(r));
        }
        all.push(r.clone());
    }
    let doc = json!({ "updatedAt": now_rfc3339(), "rows": all });
    fs::create_dir_all(out).map_err(fail)?;
    let text = crate::measure::redact(&serde_json::to_string_pretty(&doc).unwrap(), secrets);
    fs::write(&path, text + "\n").map_err(fail)?;
    Ok(all)
}

const ENTITIES: [Entity; 4] = [Entity::Tag, Entity::Scene, Entity::Image, Entity::Performer];
const CLIENTS: [SavingClient; 3] = [
    SavingClient::WebUi,
    SavingClient::SemanticTagging,
    SavingClient::Viewer,
];

/// The entity × client pairs with no row of the given mode.
pub fn missing_pairs(rows: &[HookObservation], mode: Mode) -> Vec<(Entity, SavingClient)> {
    let mut out = Vec::new();
    for e in ENTITIES {
        for c in CLIENTS {
            if !rows
                .iter()
                .any(|r| r.entity == e && r.client == c && r.mode == mode)
            {
                out.push((e, c));
            }
        }
    }
    out
}

fn load_seeded(b: &Bench) -> Result<(PathBuf, SeedManifest), Failure> {
    let path = seed::manifest_path(&b.global.out);
    let m = seed::load_manifest(&path)?.ok_or_else(|| {
        Failure::new(
            Exit::Refused,
            "no seed manifest; run `spike-bench seed` first",
        )
    })?;
    if m.state != SeedState::Seeded || m.server != b.url {
        return Err(Failure::new(
            Exit::Refused,
            format!(
                "the seed must be in place on {} (state {:?}, server {})",
                b.url, m.state, m.server
            ),
        ));
    }
    Ok((path, m))
}

fn targets(run: &Run) -> Result<Targets, Failure> {
    let m = &run.manifest;
    let first = |k: Kind| {
        m.changed
            .iter()
            .find(|c| c.kind == k)
            .map(|c| c.id.clone())
            .ok_or_else(|| Failure::new(Exit::Refused, format!("the seed changed no {k:?}")))
    };
    let tag_of = |flag: &str| {
        m.created_tags
            .iter()
            .find(|t| t.flag == flag)
            .map(|t| t.id.clone())
    };
    let performers = run.client.ids_only(Kind::Performer)?;
    if performers.len() < 2 {
        return Err(Failure::new(Exit::RunFailed, "need at least 2 performers"));
    }
    Ok(Targets {
        tag: tag_of("isTrait")
            .ok_or_else(|| Failure::new(Exit::Refused, "the seed created no isTrait tag"))?,
        scene: first(Kind::Scene)?,
        image: first(Kind::Image)?,
        performer: performers[0].clone(),
        role_tag: tag_of("isRole")
            .ok_or_else(|| Failure::new(Exit::Refused, "the seed created no isRole tag"))?,
        performers,
    })
}

pub fn command(b: &mut Bench) -> Outcome {
    let log = b.plugin_dir()?.join(LOG);
    let (manifest_path, manifest) = load_seeded(b)?;
    // Own the client for this run so `b` stays free for output.
    let owned = b
        .client
        .take()
        .ok_or_else(|| Failure::new(Exit::Usage, "no Stash connection"))?;
    let client = &owned;
    let offset = server_offset(client)?;
    let mut run = Run {
        client,
        log,
        manifest_path,
        manifest,
        offset,
    };
    let repeats = b.opt_u64("--repeat")?.unwrap_or(5) as usize;
    let secrets: Vec<String> = b.secrets().iter().map(|s| s.to_string()).collect();
    let secrets: Vec<&str> = secrets.iter().map(String::as_str).collect();

    let rows = if b.flag("--manual") {
        manual(b, &run)?
    } else {
        let n = epoch_ms() as u64 % 100_000;
        let saves: Vec<Save> = match b.opt_u64("--bulk")? {
            Some(count) => {
                let ids: Vec<String> = run
                    .client
                    .find_custom_fields(Kind::Scene, None, None, Page::first(count as i32))?
                    .1
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                let mut fields = Map::new();
                fields.insert(TOUCH.into(), json!(n));
                vec![Save {
                    entity: Entity::Scene,
                    client: SavingClient::WebUi,
                    mutation: "bulkSceneUpdate".into(),
                    items: ids.len() as u64,
                    action: Action::Bulk {
                        kind: Kind::Scene,
                        ids,
                        fields,
                    },
                    note: Some("web UI bulk edit (also the viewer's planned bulk tagging)".into()),
                }]
            }
            None => {
                let t = targets(&run)?;
                let mut saves = clients::scripted(&t, n);
                let merge = run.new_tags(2, "merge")?;
                let many = run.new_tags(3, "destroy")?;
                let single = run.new_tags(1, "destroy1")?;
                saves.push(Save {
                    entity: Entity::Tag,
                    client: SavingClient::WebUi,
                    mutation: "tagsMerge".into(),
                    items: 2,
                    action: Action::TagsMerge {
                        source: vec![merge[0].clone()],
                        destination: merge[1].clone(),
                    },
                    note: Some("merge one tag into another".into()),
                });
                saves.push(Save {
                    entity: Entity::Tag,
                    client: SavingClient::WebUi,
                    mutation: "tagsDestroy".into(),
                    items: 3,
                    action: Action::TagsDestroy { ids: many.clone() },
                    note: Some("delete three tags at once".into()),
                });
                saves.push(Save {
                    entity: Entity::Tag,
                    client: SavingClient::SemanticTagging,
                    mutation: "tagDestroy".into(),
                    items: 1,
                    action: Action::TagDestroy {
                        id: single[0].clone(),
                    },
                    note: Some("the plugin deleting a variation tag".into()),
                });
                saves
            }
        };
        let mut rows = Vec::new();
        for s in &saves {
            let row = observe(&mut run, s, repeats)?;
            b.say(format!(
                "{:?} {:?} {}: {} hook(s), delay {:?} ms, save {:?} ms",
                row.client,
                row.entity,
                row.mutation,
                row.hooks_received.len(),
                row.delay.map(|d| d.round()),
                row.save_latency.map(|d| (d * 10.0).round() / 10.0)
            ));
            // Merged-away and deleted tags are gone: forget them before cleanup needs them.
            match &s.action {
                Action::TagsMerge { source, .. } => run.forget_tags(source)?,
                Action::TagsDestroy { ids } => run.forget_tags(ids)?,
                Action::TagDestroy { id } => run.forget_tags(std::slice::from_ref(id))?,
                _ => {}
            }
            rows.push(row);
        }
        no_hook_pass(&mut run, &saves, &mut rows, repeats)?;
        rows
    };

    let all = merge_table(&b.global.out, &rows, &secrets)?;
    b.say(format!(
        "{} rows written; table now has {} rows ({})",
        rows.len(),
        all.len(),
        b.global.out.join(TABLE).display()
    ));
    let scripted = missing_pairs(&all, Mode::Scripted);
    let manual = missing_pairs(&all, Mode::Manual);
    let manual_clients: BTreeSet<String> = CLIENTS
        .iter()
        .filter(|c| {
            !all.iter()
                .any(|r| r.client == **c && r.mode == Mode::Manual)
        })
        .map(|c| format!("{c:?}"))
        .collect();
    if !manual_clients.is_empty() {
        b.say(format!(
            "manual rows still needed for: {manual_clients:?} ({} pairs without a manual row)",
            manual.len()
        ));
    }
    if !scripted.is_empty() && !b.flag("--manual") && b.opt("--bulk").is_none() {
        return Err(Failure::new(
            Exit::RunFailed,
            format!("scripted rows missing for {scripted:?}"),
        ));
    }
    Ok(())
}

/// Listens for `--seconds` (default 120) while the owner saves from `--client`, then logs each
/// hook received as a manual row.
fn manual(b: &mut Bench, run: &Run) -> Result<Vec<HookObservation>, Failure> {
    let client = match b.opt("--client") {
        Some("web-ui") => SavingClient::WebUi,
        Some("semantic-tagging") => SavingClient::SemanticTagging,
        Some("viewer") => SavingClient::Viewer,
        other => {
            return Err(Failure::new(
                Exit::Usage,
                format!("--manual needs --client web-ui|semantic-tagging|viewer, got {other:?}"),
            ))
        }
    };
    // `--since <RFC 3339 UTC>` labels hooks that already arrived; otherwise listen for a window.
    let hooks = match b.opt("--since").map(String::from) {
        Some(since) => {
            let until = b.opt("--until").map(String::from);
            let hooks: Vec<HookReceived> = read_new(&run.log, 0)
                .into_iter()
                .filter(|h| h.received_at.as_str() >= since.as_str())
                .filter(|h| until.as_deref().is_none_or(|u| h.received_at.as_str() < u))
                .collect();
            b.say(format!("{} hook(s) received since {since}", hooks.len()));
            hooks
        }
        None => {
            let seconds = b.opt_u64("--seconds")?.unwrap_or(120);
            let offset = log_len(&run.log);
            b.say(format!(
                "listening {seconds} s: save a tag, a scene, an image, and a performer from {client:?} now"
            ));
            thread::sleep(Duration::from_secs(seconds));
            read_new(&run.log, offset)
        }
    };
    let mut rows = Vec::new();
    for h in hooks {
        let entity = match h.hook_type.split('.').next() {
            Some("Tag") => Entity::Tag,
            Some("Scene") => Entity::Scene,
            Some("Image") => Entity::Image,
            Some("Performer") => Entity::Performer,
            _ => continue,
        };
        rows.push(HookObservation {
            entity,
            client,
            mode: Mode::Manual,
            mutation: h.hook_type.clone(),
            items: 1,
            hooks_received: vec![h],
            delay: None,
            save_latency: None,
            save_latency_no_hook: None,
            updated_at_bumped: None,
            missed: false,
            out_of_order: false,
            repeats: 0,
            note: Some("saved by the owner from the real client".into()),
        });
    }
    if rows.is_empty() {
        b.warn("no hooks to record");
    }
    Ok(rows)
}
