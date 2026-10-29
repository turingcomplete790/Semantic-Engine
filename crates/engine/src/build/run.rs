//! A full build (research R4): stamp check, lock, one full read, mapping record by record, the
//! vocabulary, and one publish. Nothing is published unless every step succeeds; a build that
//! is killed leaves the previous generation current (research R2).

use super::map::owner_iri;
use super::map::{map_media, map_performer, map_tag, Ctx, Mapped};
use super::notes::{Note, NoteReason};
use super::stamp::{BuildStamp, How, Items};
use super::update::owner_kind;
use super::vocab::{vocabulary_facts, VOCAB_OWNER};
use crate::graph::{Fact, GraphError, GraphStore, Health, Owned, RebuildReason, Writer};
use semantic_vocabulary::iri::ItemIris;
use serde::Serialize;
use stash_adapter::reader::{Kind, LibraryReader, Progress};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Facts per `Writer::add`.
    pub batch_facts: usize,
    /// Notes written to the log; the rest are counted.
    pub note_log_cap: usize,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions {
            batch_facts: 50_000,
            note_log_cap: 1_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RecordCounts {
    pub tags: usize,
    pub performers: usize,
    pub scenes: usize,
    pub images: usize,
}

impl RecordCounts {
    pub fn total(&self) -> usize {
        self.tags + self.performers + self.scenes + self.images
    }
}

/// Milliseconds per phase.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PhaseTimes {
    pub read: u64,
    pub map: u64,
    pub write: u64,
    pub publish: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NoteCounts {
    pub skipped: BTreeMap<NoteReason, usize>,
    pub mismatches: BTreeMap<NoteReason, usize>,
}

/// What one build did (data-model § BuildReport; contracts/operations-1.2.md).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuildReport {
    pub generation: u64,
    pub facts: u64,
    pub records: RecordCounts,
    /// Why the store was cleared first; absent for a first build.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleared: Option<&'static str>,
    pub notes: NoteCounts,
    pub phases: PhaseTimes,
    pub seconds: f64,
}

impl BuildReport {
    /// The summary line (research R8).
    pub fn summary(&self) -> String {
        let r = &self.records;
        let counts = |m: &BTreeMap<NoteReason, usize>| {
            let total: usize = m.values().sum();
            if total == 0 {
                return "0".to_string();
            }
            let parts: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{} {v}", k.as_str()))
                .collect();
            format!("{total} ({})", parts.join(", "))
        };
        format!(
            "build: generation {} published; {} records ({} tags, {} performers, {} scenes, {} images); {} facts; skipped {}; mismatches {}; {:.1} s",
            self.generation,
            r.total(),
            r.tags,
            r.performers,
            r.scenes,
            r.images,
            self.facts,
            counts(&self.notes.skipped),
            counts(&self.notes.mismatches),
            self.seconds
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("no server identifier yet: run the \"Generate server identifier\" task first")]
    NoServerId,
    #[error("the server identifier setting is not a valid identifier")]
    BadServerId,
    #[error("a build or another writer is using the graph store; nothing was changed")]
    Busy,
    #[error("{0}")]
    Stash(stash_adapter::Error),
    #[error("graph store: {0}")]
    Store(GraphError),
}

impl From<GraphError> for BuildError {
    fn from(e: GraphError) -> Self {
        match e {
            GraphError::Busy => BuildError::Busy,
            GraphError::NoServerId => BuildError::NoServerId,
            e => BuildError::Store(e),
        }
    }
}

/// The contract's name of a rebuild reason.
pub fn reason_name(r: &RebuildReason) -> &'static str {
    match r {
        RebuildReason::Missing => "missing",
        RebuildReason::Damaged { .. } => "damaged",
        RebuildReason::OtherFormat { .. } => "otherFormat",
        RebuildReason::OtherServer => "otherServer",
    }
}

/// Progress as whole percents, reported only when the percent changes.
struct Percent<'p> {
    sink: &'p mut dyn FnMut(f64),
    last: Option<u32>,
}

impl Percent<'_> {
    fn set(&mut self, share: f64) {
        let pct = (share.clamp(0.0, 1.0) * 100.0).floor() as u32;
        if self.last.is_none_or(|l| pct > l) {
            self.last = Some(pct);
            (self.sink)(pct as f64 / 100.0);
        }
    }
}

/// Owners' facts gathered into batches for the writer, so at most one batch is held at a time.
/// Each record's facts go to the graph named by its item IRI (spec 008 research R1).
struct Batches<'w> {
    writer: &'w mut Writer,
    batch: Vec<Owned>,
    facts: usize,
    size: usize,
    write: Duration,
}

impl Batches<'_> {
    fn push(
        &mut self,
        owner: Option<String>,
        facts: impl IntoIterator<Item = Fact>,
    ) -> Result<(), BuildError> {
        let facts: Vec<Fact> = facts.into_iter().collect();
        let Some(owner) = owner else {
            return Ok(()); // an id the item-IRI scheme refuses: the record mapped to nothing
        };
        if facts.is_empty() {
            return Ok(());
        }
        self.facts += facts.len();
        self.batch.push(Owned { owner, facts });
        if self.facts >= self.size {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), BuildError> {
        if !self.batch.is_empty() {
            let t = Instant::now();
            self.writer.add_owned(&self.batch)?;
            self.batch.clear();
            self.facts = 0;
            self.write += t.elapsed();
        }
        Ok(())
    }
}

const READ_SHARE: f64 = 0.30;
const WRITE_END: f64 = 0.90;

/// Runs one full build. `store` must be for the same `server_id`.
pub fn run(
    reader: &dyn LibraryReader,
    store: &GraphStore,
    server_id: &str,
    options: &BuildOptions,
    progress: &mut dyn FnMut(f64),
    log: &mut dyn FnMut(LogLevel, &str),
) -> Result<BuildReport, BuildError> {
    let started = Instant::now();
    if server_id.is_empty() {
        return Err(BuildError::NoServerId);
    }
    let iris = ItemIris::new(server_id).map_err(|_| BuildError::BadServerId)?;
    let mut pct = Percent {
        sink: progress,
        last: None,
    };
    pct.set(0.0);

    let mut cleared = None;
    if let Health::NeedsRebuild(reason) = store.check()? {
        store.clear_for_rebuild()?;
        if reason != RebuildReason::Missing {
            log(
                LogLevel::Info,
                &format!("build: the graph store was cleared first ({reason})"),
            );
            cleared = Some(reason_name(&reason));
        }
    }
    let mut writer = store.rebuild_writer()?;

    let t = Instant::now();
    let read = reader
        .full_read(&|| false, &mut |p: Progress| {
            pct.set(p.share() * READ_SHARE)
        })
        .map_err(BuildError::Stash)?;
    let read_ms = t.elapsed().as_millis() as u64;
    let records = RecordCounts {
        tags: read.tags.len(),
        performers: read.performers.len(),
        scenes: read.scenes.len(),
        images: read.images.len(),
    };

    let ctx = Ctx::new(&read.tags, &iris);
    let total = records.total().max(1);
    let mut sink = Batches {
        writer: &mut writer,
        batch: Vec::new(),
        facts: 0,
        size: options.batch_facts,
        write: Duration::ZERO,
    };
    let mut notes: Vec<Note> = Vec::new();
    let mut map_time = Duration::ZERO;
    let mut done = 0usize;
    let mut items = Items::default();
    let mut mapped =
        |owner: Option<String>, m: Mapped, t: Instant, sink: &mut Batches, pct: &mut Percent| {
            map_time += t.elapsed();
            if let Some(kind) = owner
                .as_deref()
                .filter(|_| !m.facts.is_empty())
                .and_then(owner_kind)
            {
                items.add(kind, 1);
            }
            notes.extend(m.notes.into_vec());
            done += 1;
            pct.set(READ_SHARE + (WRITE_END - READ_SHARE) * (done as f64 / total as f64));
            sink.push(owner, m.facts)
        };
    for tag in &read.tags {
        let t = Instant::now();
        let owner = owner_iri(&iris, Kind::Tag, &tag.id);
        mapped(owner, map_tag(&ctx, tag), t, &mut sink, &mut pct)?;
    }
    for p in &read.performers {
        let t = Instant::now();
        let owner = owner_iri(&iris, Kind::Performer, &p.id);
        mapped(owner, map_performer(&ctx, p), t, &mut sink, &mut pct)?;
    }
    for r in read.scenes.iter().chain(&read.images) {
        let t = Instant::now();
        let owner = owner_iri(&iris, r.kind.into(), &r.id);
        mapped(owner, map_media(&ctx, r), t, &mut sink, &mut pct)?;
    }
    sink.push(Some(VOCAB_OWNER.to_string()), vocabulary_facts())?;
    sink.flush()?;
    let write_ms = sink.write.as_millis() as u64;
    pct.set(WRITE_END);

    log(LogLevel::Info, "build: publishing");
    let t = Instant::now();
    let generation = writer.publish_with(
        BuildStamp::new(read.start.clone(), How::Build)
            .with_items(items)
            .to_json(),
    )?;
    let publish_ms = t.elapsed().as_millis() as u64;
    pct.set(1.0);

    let mut counts = NoteCounts::default();
    for n in &notes {
        let group = if n.reason.is_skip() {
            &mut counts.skipped
        } else {
            &mut counts.mismatches
        };
        *group.entry(n.reason).or_default() += 1;
    }
    for n in notes.iter().take(options.note_log_cap) {
        log(LogLevel::Warning, &n.log_line());
    }
    if notes.len() > options.note_log_cap {
        log(
            LogLevel::Warning,
            &format!(
                "notes: {} more not written",
                notes.len() - options.note_log_cap
            ),
        );
    }
    for n in &generation.notes {
        log(LogLevel::Warning, &format!("cleanup: {n}"));
    }
    let report = BuildReport {
        generation: generation.number,
        facts: generation.facts,
        records,
        cleared,
        notes: counts,
        phases: PhaseTimes {
            read: read_ms,
            map: map_time.as_millis() as u64,
            write: write_ms,
            publish: publish_ms,
        },
        seconds: started.elapsed().as_secs_f64(),
    };
    log(LogLevel::Info, &report.summary());
    Ok(report)
}
