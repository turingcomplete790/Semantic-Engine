//! A library reader with no network, for callers' tests (spec 005 FR-023). It answers every read
//! from the records it holds, usually a recorded `LibraryRead` file, and keeps rules B1 to B9 of
//! contracts/reader-interface.md.

use super::records::{
    id_order, sorted_ids, Kind, LibraryRead, MediaKind, MediaRecord, Phase, Progress, RecordBatch,
    SemanticPerformer, SemanticTag,
};
use super::{LibraryReader, RECORD_BATCH};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub struct MemoryReader {
    read: LibraryRead,
    extra_ids: BTreeMap<Kind, Vec<String>>,
    fail: Option<Error>,
}

impl MemoryReader {
    pub fn new(read: LibraryRead) -> MemoryReader {
        MemoryReader {
            read,
            extra_ids: BTreeMap::new(),
            fail: None,
        }
    }

    /// Loads a recorded `LibraryRead` JSON file, as written by `fixture-capture`.
    pub fn from_file(path: &Path) -> Result<MemoryReader, Error> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::Setup(format!("{}: {e}", path.display())))?;
        let read = serde_json::from_str(&text)
            .map_err(|e| Error::Setup(format!("{}: {e}", path.display())))?;
        Ok(MemoryReader::new(read))
    }

    /// Adds ids that exist but carry no semantic data, for `all_ids`.
    pub fn with_ids(mut self, kind: Kind, ids: impl IntoIterator<Item = String>) -> MemoryReader {
        self.extra_ids.entry(kind).or_default().extend(ids);
        self
    }

    /// Makes every call fail with `error`, for testing callers' failure handling.
    pub fn fail_with(mut self, error: Error) -> MemoryReader {
        self.fail = Some(error);
        self
    }

    fn check(&self) -> Result<(), Error> {
        match &self.fail {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }

    fn media(&self, kind: MediaKind) -> &[MediaRecord] {
        match kind {
            MediaKind::Scene => &self.read.scenes,
            MediaKind::Image => &self.read.images,
        }
    }

    /// `(id, updated_at)` of every record of `kind`.
    fn stamps(&self, kind: Kind) -> Vec<(&str, &str)> {
        match kind {
            Kind::Tag => self
                .read
                .tags
                .iter()
                .map(|t| (t.id.as_str(), t.updated_at.as_str()))
                .collect(),
            Kind::Performer => self
                .read
                .performers
                .iter()
                .map(|p| (p.id.as_str(), p.updated_at.as_str()))
                .collect(),
            Kind::Scene => self
                .media(MediaKind::Scene)
                .iter()
                .map(|m| (m.id.as_str(), m.updated_at.as_str()))
                .collect(),
            Kind::Image => self
                .media(MediaKind::Image)
                .iter()
                .map(|m| (m.id.as_str(), m.updated_at.as_str()))
                .collect(),
        }
    }
}

fn pick<T: Clone>(all: &[T], ids: &[String], id: impl Fn(&T) -> &str) -> RecordBatch<T> {
    let wanted: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let mut found: Vec<T> = all
        .iter()
        .filter(|r| wanted.contains(id(r)))
        .cloned()
        .collect();
    found.sort_by_key(|r| id_order(id(r)));
    let have: BTreeSet<&str> = found.iter().map(&id).collect();
    let missing = sorted_ids(
        wanted
            .into_iter()
            .filter(|w| !have.contains(w))
            .map(String::from)
            .collect(),
    );
    RecordBatch { found, missing }
}

impl LibraryReader for MemoryReader {
    fn full_read(
        &self,
        should_stop: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<LibraryRead, Error> {
        self.check()?;
        let r = &self.read;
        let counts = [
            r.tags.len(),
            r.performers.len(),
            r.scenes.len(),
            r.images.len(),
        ];
        let total: usize = counts.iter().map(|n| n.div_ceil(RECORD_BATCH)).sum();
        let mut done = 0;
        for n in counts {
            for _ in 0..n.div_ceil(RECORD_BATCH) {
                if should_stop() {
                    return Err(Error::Stopped);
                }
                done += 1;
                progress(Progress {
                    phase: Phase::Records,
                    done,
                    total,
                });
            }
        }
        if should_stop() {
            return Err(Error::Stopped);
        }
        progress(Progress {
            phase: Phase::Done,
            done: 0,
            total: 0,
        });
        Ok(LibraryRead {
            requests: 0,
            missing: BTreeMap::new(),
            ..r.clone()
        })
    }

    fn tag_records(&self, ids: &[String]) -> Result<RecordBatch<SemanticTag>, Error> {
        self.check()?;
        Ok(pick(&self.read.tags, ids, |t| &t.id))
    }

    fn performer_records(&self, ids: &[String]) -> Result<RecordBatch<SemanticPerformer>, Error> {
        self.check()?;
        Ok(pick(&self.read.performers, ids, |p| &p.id))
    }

    fn media_records(
        &self,
        kind: MediaKind,
        ids: &[String],
    ) -> Result<RecordBatch<MediaRecord>, Error> {
        self.check()?;
        Ok(pick(self.media(kind), ids, |m| &m.id))
    }

    fn changed_since(&self, kind: Kind, threshold: &str) -> Result<Vec<String>, Error> {
        self.check()?;
        Ok(sorted_ids(
            self.stamps(kind)
                .into_iter()
                .filter(|(_, t)| *t > threshold)
                .map(|(id, _)| id.to_string())
                .collect(),
        ))
    }

    fn all_ids(&self, kind: Kind) -> Result<Vec<String>, Error> {
        self.check()?;
        let mut ids: Vec<String> = self
            .stamps(kind)
            .into_iter()
            .map(|(id, _)| id.to_string())
            .collect();
        ids.extend(self.extra_ids.get(&kind).into_iter().flatten().cloned());
        Ok(sorted_ids(ids))
    }

    fn latest_updated_at(&self, kind: Kind) -> Result<Option<String>, Error> {
        self.check()?;
        Ok(self.read.start.get(&kind).cloned())
    }
}
