//! An incremental update (spec 008 research R5): re-read only what changed since the generation's
//! catch-up point, replace those items' graphs, and publish once. It falls back to a full build
//! whenever an update cannot be trusted, so the graph always equals a full rebuild (FR-007).

use super::map::{map_media, map_performer, map_tag, owner_iri, Ctx};
use super::notes::Note;
use super::run::{
    reason_name, run, BuildError, BuildOptions, BuildReport, LogLevel, NoteCounts, PhaseTimes,
};
use super::stamp::{vocabulary_version, BuildStamp, How, Items, MAPPING_VERSION};
use crate::graph::{GraphStore, Health, RebuildReason, Replaced, Writer};
use semantic_vocabulary::iri::ItemIris;
use serde::Serialize;
use stash_adapter::reader::time::one_second_earlier;
use stash_adapter::reader::{performer_tag_set, Kind, LibraryReader, MediaKind, ReadStart};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// Above this share of the store's items to re-map, an update rebuilds instead (research R5).
pub const UPDATE_SHARE_LIMIT: f64 = 1.0 / 3.0;
/// The share limit never falls below this many items: a small library rebuilds quickly either
/// way, and its updates stay updates.
pub const UPDATE_MIN_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Update,
    Current,
    Rebuild,
}

/// Why an update rebuilt in full (FR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "cause", rename_all = "camelCase")]
pub enum RebuildCause {
    FirstBuild,
    StoreNeedsRebuild { cleared: &'static str },
    MappingVersion { from: u32, to: u32 },
    VocabularyVersion { from: String, to: String },
    NoCatchUpPoint,
    TooManyChanges { items: usize, limit: usize },
}

impl std::fmt::Display for RebuildCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RebuildCause::FirstBuild => write!(f, "first build"),
            RebuildCause::StoreNeedsRebuild { cleared } => {
                write!(f, "the store needs a rebuild ({cleared})")
            }
            RebuildCause::MappingVersion { from, to } => write!(f, "mapping version {from} → {to}"),
            RebuildCause::VocabularyVersion { from, to } => write!(f, "vocabulary {from} → {to}"),
            RebuildCause::NoCatchUpPoint => write!(f, "no catch-up point recorded"),
            RebuildCause::TooManyChanges { items, limit } => {
                write!(f, "{items} items changed, over the limit of {limit}")
            }
        }
    }
}

/// One kind's outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct KindChanges {
    pub changed: usize,
    pub added: usize,
    pub removed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependents: Option<usize>,
}

impl KindChanges {
    fn any(&self) -> bool {
        self.changed + self.added + self.removed > 0
    }

    fn describe(&self, kind: &str) -> Option<String> {
        if !self.any() && self.dependents.unwrap_or(0) == 0 {
            return None;
        }
        let mut parts = Vec::new();
        for (n, what) in [
            (self.changed, "changed"),
            (self.added, "added"),
            (self.removed, "removed"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {what}"));
            }
        }
        if let Some(d) = self.dependents.filter(|d| *d > 0) {
            parts.push(format!("{d} dependents"));
        }
        Some(format!("{kind} {}", parts.join(", ")))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Changes {
    pub tags: KindChanges,
    pub performers: KindChanges,
    pub scenes: KindChanges,
    pub images: KindChanges,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FactChanges {
    pub removed: u64,
    pub added: u64,
    pub total: u64,
}

/// What one update did (data-model § UpdateReport; contracts/operations-1.3.md).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    pub mode: Mode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebuild_reason: Option<RebuildCause>,
    pub generation: u64,
    pub changes: Changes,
    pub facts: FactChanges,
    pub notes: NoteCounts,
    pub phases: PhaseTimes,
    pub seconds: f64,
    /// The full build's report, when the update rebuilt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildReport>,
}

impl UpdateReport {
    /// The summary line (contracts/operations-1.3.md).
    pub fn summary(&self) -> String {
        match self.mode {
            Mode::Current => format!(
                "update: the graph is current; generation {} kept; {:.1} s",
                self.generation, self.seconds
            ),
            Mode::Rebuild => format!(
                "update: rebuilt in full ({}); generation {} published; {} facts; {:.1} s",
                self.rebuild_reason
                    .as_ref()
                    .map(|r| r.to_string())
                    .unwrap_or_default(),
                self.generation,
                self.facts.total,
                self.seconds
            ),
            Mode::Update => {
                let c = &self.changes;
                let kinds: Vec<String> = [
                    c.tags.describe("tags"),
                    c.performers.describe("performers"),
                    c.scenes.describe("scenes"),
                    c.images.describe("images"),
                ]
                .into_iter()
                .flatten()
                .collect();
                let sum = |m: &BTreeMap<_, usize>| m.values().sum::<usize>();
                format!(
                    "update: generation {} published; {}; facts -{} +{} ({}); skipped {}; mismatches {}; {:.1} s",
                    self.generation,
                    kinds.join("; "),
                    self.facts.removed,
                    self.facts.added,
                    self.facts.total,
                    sum(&self.notes.skipped),
                    sum(&self.notes.mismatches),
                    self.seconds
                )
            }
        }
    }
}

/// The kind of an owner graph, from its item IRI (`urn:semantic-stash:{server}:{kind}:{id}`).
/// `None` for the vocabulary's graph and anything else.
pub fn owner_kind(iri: &str) -> Option<Kind> {
    let rest = iri.strip_prefix("urn:semantic-stash:")?;
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() != 3 || parts[2].is_empty() || parts[2].contains('/') {
        return None;
    }
    match parts[1] {
        "tag" => Some(Kind::Tag),
        "performer" => Some(Kind::Performer),
        "scene" => Some(Kind::Scene),
        "image" => Some(Kind::Image),
        _ => None,
    }
}

fn id_of(iri: &str) -> &str {
    iri.rsplit(':').next().unwrap_or(iri)
}

/// Whether the current generation can be updated, or why not.
fn trust(store: &GraphStore) -> Result<Result<BuildStamp, RebuildCause>, BuildError> {
    match store.check()? {
        Health::NeedsRebuild(RebuildReason::Missing) | Health::NotBuilt => {
            return Ok(Err(RebuildCause::FirstBuild))
        }
        Health::NeedsRebuild(r) => {
            return Ok(Err(RebuildCause::StoreNeedsRebuild {
                cleared: reason_name(&r),
            }))
        }
        Health::Ready { .. } => {}
    }
    let generation = store.reader()?.generation().clone();
    let Some(stamp) = BuildStamp::read(&generation) else {
        return Ok(Err(RebuildCause::NoCatchUpPoint));
    };
    if stamp.mapping_version != MAPPING_VERSION {
        return Ok(Err(RebuildCause::MappingVersion {
            from: stamp.mapping_version,
            to: MAPPING_VERSION,
        }));
    }
    let vocabulary = vocabulary_version();
    if stamp.vocabulary_version != vocabulary {
        return Ok(Err(RebuildCause::VocabularyVersion {
            from: stamp.vocabulary_version,
            to: vocabulary,
        }));
    }
    Ok(Ok(stamp))
}

/// The changes of one kind, read before anything is written.
struct KindReads {
    changed: BTreeSet<String>,
    /// Of `changed`, those saved after the catch-up second itself: the real new changes, which
    /// the share limit counts (research R13).
    fresh: usize,
    deleted: BTreeSet<String>,
    owned: BTreeSet<String>,
}

fn read_kind(
    reader: &dyn LibraryReader,
    iris: &ItemIris,
    kind: Kind,
    old: &ReadStart,
    owners: &BTreeMap<Kind, BTreeSet<String>>,
) -> Result<KindReads, BuildError> {
    // No items of this kind at the last point, or a point whose text cannot be read: every item
    // of the kind counts as changed (the share limit turns a large one into a rebuild).
    let point = old.get(&kind);
    let changed = match point.and_then(|t| one_second_earlier(t)) {
        Some(t) => reader.changed_since(kind, &t),
        None => reader.all_ids(kind),
    }
    .map_err(BuildError::Stash)?;
    // Items in the catch-up second are re-read every time (rule 7 reads from one second early);
    // a bulk save can leave hundreds there. Only those after it are new changes.
    let fresh = match point {
        Some(t) if one_second_earlier(t).is_some() => reader
            .changed_since(kind, t)
            .map_err(BuildError::Stash)?
            .len(),
        _ => changed.len(),
    };
    // Stash's ids, as owner IRIs, to compare with the store's owners.
    let all: BTreeSet<String> = reader
        .all_ids(kind)
        .map_err(BuildError::Stash)?
        .iter()
        .filter_map(|id| owner_iri(iris, kind, id))
        .collect();
    let owned = owners.get(&kind).cloned().unwrap_or_default();
    let deleted = owned.difference(&all).cloned().collect();
    Ok(KindReads {
        fresh,
        changed: changed.into_iter().collect(),
        deleted,
        owned,
    })
}

/// Tallies of one update's writes.
#[derive(Default)]
struct Tally {
    removed: u64,
    added: u64,
    write: Duration,
    map: Duration,
    notes: Vec<Note>,
}

impl Tally {
    /// Replaces an owner's graph; returns whether anything changed.
    fn replace(
        &mut self,
        writer: &mut Writer,
        owner: &str,
        facts: Vec<crate::graph::Fact>,
    ) -> Result<bool, BuildError> {
        let t = Instant::now();
        let r = writer.replace_owned(owner, &facts)?;
        self.write += t.elapsed();
        Ok(match r {
            Replaced::Unchanged => false,
            Replaced::Changed { removed, added } => {
                self.removed += removed;
                self.added += added;
                true
            }
        })
    }

    fn remove(&mut self, writer: &mut Writer, owner: &str) -> Result<bool, BuildError> {
        let t = Instant::now();
        let n = writer.remove_owned(owner)?;
        self.write += t.elapsed();
        self.removed += n;
        Ok(n > 0)
    }
}

/// Records whether an owner was added, changed or removed.
fn count(c: &mut KindChanges, was_owned: bool, changed: bool, now_owned: bool) {
    match (was_owned, now_owned) {
        (false, true) if changed => c.added += 1,
        (true, false) if changed => c.removed += 1,
        (true, true) if changed => c.changed += 1,
        _ => {}
    }
}

/// Brings the published graph up to date with the library (FR-001), or rebuilds it in full when
/// an update cannot be trusted (FR-010).
pub fn update(
    reader: &dyn LibraryReader,
    store: &GraphStore,
    server_id: &str,
    options: &BuildOptions,
    progress: &mut dyn FnMut(f64),
    log: &mut dyn FnMut(LogLevel, &str),
) -> Result<UpdateReport, BuildError> {
    let started = Instant::now();
    if server_id.is_empty() {
        return Err(BuildError::NoServerId);
    }
    let iris = ItemIris::new(server_id).map_err(|_| BuildError::BadServerId)?;
    let rebuild = |cause: RebuildCause,
                   progress: &mut dyn FnMut(f64),
                   log: &mut dyn FnMut(LogLevel, &str)|
     -> Result<UpdateReport, BuildError> {
        log(
            LogLevel::Info,
            &format!("update: rebuilding in full ({cause})"),
        );
        let b = run(reader, store, server_id, options, progress, log)?;
        let report = UpdateReport {
            mode: Mode::Rebuild,
            rebuild_reason: Some(cause),
            generation: b.generation,
            changes: Changes::default(),
            facts: FactChanges {
                removed: 0,
                added: b.facts,
                total: b.facts,
            },
            notes: b.notes.clone(),
            phases: b.phases.clone(),
            seconds: started.elapsed().as_secs_f64(),
            build: Some(b),
        };
        log(LogLevel::Info, &report.summary());
        Ok(report)
    };
    let old = match trust(store)? {
        Ok(stamp) => stamp,
        Err(cause) => return rebuild(cause, progress, log),
    };
    let current = store.reader()?.generation().clone();
    let mut writer = store.writer()?;
    progress(0.0);

    // Reads first, so a fallback leaves the store untouched (research R5).
    let t = Instant::now();
    let mut point = ReadStart::new();
    for kind in Kind::ALL {
        if let Some(stamp) = reader.latest_updated_at(kind).map_err(BuildError::Stash)? {
            point.insert(kind, stamp);
        }
    }
    let tags = reader.semantic_tags().map_err(BuildError::Stash)?;
    let mut owners: BTreeMap<Kind, BTreeSet<String>> = BTreeMap::new();
    for o in writer.owners()? {
        if let Some(kind) = owner_kind(&o) {
            owners.entry(kind).or_default().insert(o);
        }
    }
    let mut reads = BTreeMap::new();
    for kind in [Kind::Performer, Kind::Scene, Kind::Image] {
        reads.insert(
            kind,
            read_kind(reader, &iris, kind, &old.catch_up, &owners)?,
        );
    }
    let mut read_time = t.elapsed();
    progress(0.2);

    let items: usize = reads.values().map(|r| r.fresh + r.deleted.len()).sum();
    let held: usize = reads.values().map(|r| r.owned.len()).sum();
    let limit = ((held as f64 * UPDATE_SHARE_LIMIT).ceil() as usize).max(UPDATE_MIN_LIMIT);
    if items > limit {
        drop(writer);
        return rebuild(RebuildCause::TooManyChanges { items, limit }, progress, log);
    }

    let ctx = Ctx::new(&tags, &iris);
    let mut tally = Tally::default();
    let mut changes = Changes::default();

    // Tags, read whole (research R4): remove the ones gone, replace the rest.
    let tag_owned = owners.get(&Kind::Tag).cloned().unwrap_or_default();
    let mut tag_owners = BTreeSet::new();
    let mut touched_tags = BTreeSet::new();
    for tag in &tags {
        let Some(owner) = owner_iri(&iris, Kind::Tag, &tag.id) else {
            continue;
        };
        let t = Instant::now();
        let m = map_tag(&ctx, tag);
        tally.map += t.elapsed();
        tally.notes.extend(m.notes.into_vec());
        let changed = tally.replace(&mut writer, &owner, m.facts.into_iter().collect())?;
        count(&mut changes.tags, tag_owned.contains(&owner), changed, true);
        if changed {
            touched_tags.insert(owner.clone());
        }
        tag_owners.insert(owner);
    }
    for owner in tag_owned.difference(&tag_owners) {
        if tally.remove(&mut writer, owner)? {
            changes.tags.removed += 1;
        }
        touched_tags.insert(owner.clone());
    }
    progress(0.35);

    // Performers: the changed ones, and the dependents of touched tags.
    let performer_tags = performer_tag_set(&tags);
    let mut dependents = BTreeSet::new();
    if !touched_tags.is_empty() {
        let t = Instant::now();
        let ids: Vec<String> = touched_tags.iter().map(|o| id_of(o).to_string()).collect();
        dependents.extend(
            reader
                .performers_with_tags(&ids)
                .map_err(BuildError::Stash)?,
        );
        read_time += t.elapsed();
        for tag in &touched_tags {
            for o in writer.owners_referencing(tag)? {
                if owner_kind(&o) == Some(Kind::Performer) {
                    dependents.insert(id_of(&o).to_string());
                }
            }
        }
    }
    let perf = &reads[&Kind::Performer];
    let extra: BTreeSet<String> = dependents.difference(&perf.changed).cloned().collect();
    changes.performers.dependents = Some(extra.len());
    let wanted: Vec<String> = perf.changed.union(&extra).cloned().collect();
    let t = Instant::now();
    let batch = if wanted.is_empty() {
        None
    } else {
        Some(
            reader
                .performer_records(&wanted)
                .map_err(BuildError::Stash)?,
        )
    };
    read_time += t.elapsed();
    if let Some(batch) = batch {
        for p in &batch.found {
            let Some(owner) = owner_iri(&iris, Kind::Performer, &p.id) else {
                continue;
            };
            let was = perf.owned.contains(&owner);
            let (changed, now) = if p.is_semantic(&performer_tags) {
                let t = Instant::now();
                let m = map_performer(&ctx, p);
                tally.map += t.elapsed();
                tally.notes.extend(m.notes.into_vec());
                (
                    tally.replace(&mut writer, &owner, m.facts.into_iter().collect())?,
                    true,
                )
            } else {
                (tally.remove(&mut writer, &owner)?, false)
            };
            count(&mut changes.performers, was, changed, now);
        }
        for id in &batch.missing {
            if let Some(owner) = owner_iri(&iris, Kind::Performer, id) {
                if tally.remove(&mut writer, &owner)? {
                    changes.performers.removed += 1;
                }
            }
        }
    }
    for owner in &perf.deleted {
        if tally.remove(&mut writer, owner)? {
            changes.performers.removed += 1;
        }
    }
    progress(0.6);

    // Scenes and images: the changed ones; nothing of theirs depends on other items' data.
    for (kind, media) in [
        (Kind::Scene, MediaKind::Scene),
        (Kind::Image, MediaKind::Image),
    ] {
        let r = &reads[&kind];
        let c = match kind {
            Kind::Scene => &mut changes.scenes,
            _ => &mut changes.images,
        };
        if !r.changed.is_empty() {
            let wanted: Vec<String> = r.changed.iter().cloned().collect();
            let t = Instant::now();
            let batch = reader
                .media_records(media, &wanted)
                .map_err(BuildError::Stash)?;
            read_time += t.elapsed();
            for m in &batch.found {
                let Some(owner) = owner_iri(&iris, kind, &m.id) else {
                    continue;
                };
                let was = r.owned.contains(&owner);
                let (changed, now) = if m.is_semantic() {
                    let t = Instant::now();
                    let mapped = map_media(&ctx, m);
                    tally.map += t.elapsed();
                    tally.notes.extend(mapped.notes.into_vec());
                    (
                        tally.replace(&mut writer, &owner, mapped.facts.into_iter().collect())?,
                        true,
                    )
                } else {
                    (tally.remove(&mut writer, &owner)?, false)
                };
                count(c, was, changed, now);
            }
            for id in &batch.missing {
                if let Some(owner) = owner_iri(&iris, kind, id) {
                    if tally.remove(&mut writer, &owner)? {
                        c.removed += 1;
                    }
                }
            }
        }
        for owner in &r.deleted {
            if tally.remove(&mut writer, owner)? {
                c.removed += 1;
            }
        }
    }
    progress(0.9);

    let mut notes = NoteCounts::default();
    for n in &tally.notes {
        let group = if n.reason.is_skip() {
            &mut notes.skipped
        } else {
            &mut notes.mismatches
        };
        *group.entry(n.reason).or_default() += 1;
    }
    let any = [
        &changes.tags,
        &changes.performers,
        &changes.scenes,
        &changes.images,
    ]
    .iter()
    .any(|c| c.any());
    let (mode, generation, total, publish_ms) = if any {
        log(LogLevel::Info, "update: publishing");
        let t = Instant::now();
        // Item counts: the owners held when the update began, plus added, minus removed.
        let mut items = Items::default();
        for (kind, held) in &owners {
            items.add(*kind, held.len() as i64);
        }
        for (kind, c) in [
            (Kind::Tag, &changes.tags),
            (Kind::Performer, &changes.performers),
            (Kind::Scene, &changes.scenes),
            (Kind::Image, &changes.images),
        ] {
            items.add(kind, c.added as i64 - c.removed as i64);
        }
        let stamp = BuildStamp::new(point, How::Update).with_items(items);
        let g = writer.publish_with(stamp.to_json())?;
        for n in &g.notes {
            log(LogLevel::Warning, &format!("cleanup: {n}"));
        }
        (
            Mode::Update,
            g.number,
            g.facts,
            t.elapsed().as_millis() as u64,
        )
    } else {
        // Nothing changed: no generation, and the catch-up point stays (research R6).
        drop(writer);
        (Mode::Current, current.number, current.facts, 0)
    };
    progress(1.0);

    for n in tally.notes.iter().take(options.note_log_cap) {
        log(LogLevel::Warning, &n.log_line());
    }
    if tally.notes.len() > options.note_log_cap {
        log(
            LogLevel::Warning,
            &format!(
                "notes: {} more not written",
                tally.notes.len() - options.note_log_cap
            ),
        );
    }
    let report = UpdateReport {
        mode,
        rebuild_reason: None,
        generation,
        changes,
        facts: FactChanges {
            removed: tally.removed,
            added: tally.added,
            total,
        },
        notes,
        phases: PhaseTimes {
            read: read_time.as_millis() as u64,
            map: tally.map.as_millis() as u64,
            write: tally.write.as_millis() as u64,
            publish: publish_ms,
        },
        seconds: started.elapsed().as_secs_f64(),
        build: None,
    };
    log(LogLevel::Info, &report.summary());
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_kinds_come_from_item_iris_only() {
        let s = "urn:semantic-stash:00000000-0000-4000-8000-000000000001";
        assert_eq!(owner_kind(&format!("{s}:scene:12")), Some(Kind::Scene));
        assert_eq!(owner_kind(&format!("{s}:tag:7")), Some(Kind::Tag));
        assert_eq!(owner_kind(&format!("{s}:scene:12/role:7:2")), None);
        assert_eq!(owner_kind("urn:semantic-stash:vocab:"), None);
        assert_eq!(id_of(&format!("{s}:performer:40")), "40");
    }
}
