//! Helpers for the incremental update tests (spec 008 T010): a library that changes with a clock,
//! a reader that counts what it is asked, and the facts a store answers.

use super::build::{fixture_library, SERVER};
use engine::build::{run, BuildOptions};
use engine::graph::{Fact, GraphStore};
use serde_json::Value;
use stash_adapter::reader::field::Field;
use stash_adapter::reader::{
    Kind, LibraryRead, LibraryReader, MediaKind, MediaRecord, MemoryReader, Progress, RecordBatch,
    SemanticPerformer, SemanticTag, TagFlag,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// A library whose every change moves the changed item's `updated_at` one second on, in Stash's
/// server-local text, after every time in the fixture.
pub struct Library {
    pub read: LibraryRead,
    clock: u64,
}

fn stamp(n: u64) -> String {
    format!(
        "2026-10-27T{:02}:{:02}:{:02}-06:00",
        n / 3600 % 24,
        n / 60 % 60,
        n % 60
    )
}

impl Library {
    pub fn new(read: LibraryRead) -> Library {
        Library { read, clock: 0 }
    }

    pub fn fixture() -> Library {
        Library::new(fixture_library())
    }

    fn tick(&mut self, kind: Kind) -> String {
        self.clock += 1;
        let t = stamp(self.clock);
        self.read.start.insert(kind, t.clone());
        t
    }

    pub fn reader(&self) -> MemoryReader {
        MemoryReader::new(self.read.clone())
    }

    fn media_mut(&mut self, kind: MediaKind, id: &str) -> &mut MediaRecord {
        let list = match kind {
            MediaKind::Scene => &mut self.read.scenes,
            MediaKind::Image => &mut self.read.images,
        };
        list.iter_mut()
            .find(|m| m.id == id)
            .unwrap_or_else(|| panic!("no {kind:?} {id}"))
    }

    fn tag_mut(&mut self, id: &str) -> &mut SemanticTag {
        self.read
            .tags
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("no tag {id}"))
    }

    fn performer_mut(&mut self, id: &str) -> &mut SemanticPerformer {
        self.read
            .performers
            .iter_mut()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("no performer {id}"))
    }

    pub fn edit_media_field(&mut self, kind: MediaKind, id: &str, key: &str, value: Value) {
        let t = self.tick(kind.into());
        let m = self.media_mut(kind, id);
        m.fields.insert(key.into(), Field::Value(value));
        m.updated_at = t;
    }

    pub fn remove_media_field(&mut self, kind: MediaKind, id: &str, key: &str) {
        let t = self.tick(kind.into());
        let m = self.media_mut(kind, id);
        m.fields.remove(key);
        m.updated_at = t;
    }

    pub fn clear_media_fields(&mut self, kind: MediaKind, id: &str) {
        let t = self.tick(kind.into());
        let m = self.media_mut(kind, id);
        m.fields.clear();
        m.updated_at = t;
    }

    pub fn set_media_performers(&mut self, kind: MediaKind, id: &str, performers: &[&str]) {
        let t = self.tick(kind.into());
        let m = self.media_mut(kind, id);
        m.performers = performers.iter().map(|s| s.to_string()).collect();
        m.updated_at = t;
    }

    pub fn add_media(&mut self, mut m: MediaRecord) {
        m.updated_at = self.tick(m.kind.into());
        match m.kind {
            MediaKind::Scene => self.read.scenes.push(m),
            MediaKind::Image => self.read.images.push(m),
        }
    }

    pub fn delete(&mut self, kind: Kind, id: &str) {
        match kind {
            Kind::Tag => self.read.tags.retain(|t| t.id != id),
            Kind::Performer => self.read.performers.retain(|p| p.id != id),
            Kind::Scene => self.read.scenes.retain(|m| m.id != id),
            Kind::Image => self.read.images.retain(|m| m.id != id),
        }
    }

    pub fn add_performer(&mut self, mut p: SemanticPerformer) {
        p.updated_at = self.tick(Kind::Performer);
        self.read.performers.push(p);
    }

    pub fn set_performer_tags(&mut self, id: &str, tags: &[&str]) {
        let t = self.tick(Kind::Performer);
        let p = self.performer_mut(id);
        p.tags = tags.iter().map(|s| s.to_string()).collect();
        p.updated_at = t;
    }

    pub fn set_archetype(&mut self, id: &str, on: bool) {
        let t = self.tick(Kind::Performer);
        let p = self.performer_mut(id);
        p.archetype = on;
        p.updated_at = t;
    }

    pub fn set_tag_flag(&mut self, id: &str, flag: TagFlag, on: bool) {
        let t = self.tick(Kind::Tag);
        let tag = self.tag_mut(id);
        if on {
            tag.flags.insert(flag);
        } else {
            tag.flags.remove(&flag);
        }
        tag.updated_at = t;
    }

    pub fn edit_tag_field(&mut self, id: &str, key: &str, value: Value) {
        let t = self.tick(Kind::Tag);
        let tag = self.tag_mut(id);
        tag.fields.insert(key.into(), Field::Value(value));
        tag.updated_at = t;
    }

    pub fn remove_tag_field(&mut self, id: &str, key: &str) {
        let t = self.tick(Kind::Tag);
        let tag = self.tag_mut(id);
        tag.fields.remove(key);
        tag.updated_at = t;
    }

    pub fn set_tag_parents(&mut self, id: &str, parents: &[&str]) {
        let t = self.tick(Kind::Tag);
        let tag = self.tag_mut(id);
        tag.parents = parents.iter().map(|s| s.to_string()).collect();
        tag.updated_at = t;
    }

    pub fn add_tag(&mut self, mut tag: SemanticTag) {
        tag.updated_at = self.tick(Kind::Tag);
        self.read.tags.push(tag);
    }

    /// As Stash merges: the source tag is gone, and the destination is saved. Stash also moves
    /// the source's native tag links to the destination; performers carrying the source then
    /// carry the destination, and their `updated_at` moves.
    pub fn merge_tags(&mut self, source: &str, destination: &str) {
        self.delete(Kind::Tag, source);
        let t = self.tick(Kind::Tag);
        self.tag_mut(destination).updated_at = t;
        let carriers: Vec<String> = self
            .read
            .performers
            .iter()
            .filter(|p| p.tags.iter().any(|x| x == source))
            .map(|p| p.id.clone())
            .collect();
        for id in carriers {
            let t = self.tick(Kind::Performer);
            let p = self.performer_mut(&id);
            for x in p.tags.iter_mut() {
                if x == source {
                    *x = destination.to_string();
                }
            }
            p.tags.dedup();
            p.updated_at = t;
        }
    }
}

/// Counts the reader calls an update makes, and the ids it asks for.
pub struct CountingReader<'r> {
    inner: &'r dyn LibraryReader,
    pub calls: RefCell<BTreeMap<&'static str, usize>>,
    pub ids: RefCell<BTreeMap<&'static str, Vec<String>>>,
}

impl<'r> CountingReader<'r> {
    pub fn new(inner: &'r dyn LibraryReader) -> CountingReader<'r> {
        CountingReader {
            inner,
            calls: RefCell::default(),
            ids: RefCell::default(),
        }
    }

    fn note(&self, method: &'static str, ids: &[String]) {
        *self.calls.borrow_mut().entry(method).or_default() += 1;
        self.ids
            .borrow_mut()
            .entry(method)
            .or_default()
            .extend(ids.iter().cloned());
    }

    pub fn count(&self, method: &str) -> usize {
        self.calls.borrow().get(method).copied().unwrap_or(0)
    }

    pub fn asked(&self, method: &str) -> BTreeSet<String> {
        self.ids
            .borrow()
            .get(method)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl LibraryReader for CountingReader<'_> {
    fn full_read(
        &self,
        should_stop: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<LibraryRead, stash_adapter::Error> {
        self.note("full_read", &[]);
        self.inner.full_read(should_stop, progress)
    }
    fn tag_records(
        &self,
        ids: &[String],
    ) -> Result<RecordBatch<SemanticTag>, stash_adapter::Error> {
        self.note("tag_records", ids);
        self.inner.tag_records(ids)
    }
    fn performer_records(
        &self,
        ids: &[String],
    ) -> Result<RecordBatch<SemanticPerformer>, stash_adapter::Error> {
        self.note("performer_records", ids);
        self.inner.performer_records(ids)
    }
    fn media_records(
        &self,
        kind: MediaKind,
        ids: &[String],
    ) -> Result<RecordBatch<MediaRecord>, stash_adapter::Error> {
        self.note(
            match kind {
                MediaKind::Scene => "scene_records",
                MediaKind::Image => "image_records",
            },
            ids,
        );
        self.inner.media_records(kind, ids)
    }
    fn changed_since(
        &self,
        kind: Kind,
        threshold: &str,
    ) -> Result<Vec<String>, stash_adapter::Error> {
        self.note("changed_since", &[]);
        self.inner.changed_since(kind, threshold)
    }
    fn all_ids(&self, kind: Kind) -> Result<Vec<String>, stash_adapter::Error> {
        self.note("all_ids", &[]);
        self.inner.all_ids(kind)
    }
    fn latest_updated_at(&self, kind: Kind) -> Result<Option<String>, stash_adapter::Error> {
        self.note("latest_updated_at", &[]);
        self.inner.latest_updated_at(kind)
    }
    fn semantic_tags(&self) -> Result<Vec<SemanticTag>, stash_adapter::Error> {
        self.note("semantic_tags", &[]);
        self.inner.semantic_tags()
    }
    fn performers_with_tags(
        &self,
        tag_ids: &[String],
    ) -> Result<Vec<String>, stash_adapter::Error> {
        self.note("performers_with_tags", tag_ids);
        self.inner.performers_with_tags(tag_ids)
    }
}

/// A fresh, empty store folder.
pub fn new_store(name: &str) -> GraphStore {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("se-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    GraphStore::new(dir.join("graph"), SERVER)
}

/// Every fact the store's published generation answers.
pub fn all_facts(store: &GraphStore) -> BTreeSet<Fact> {
    store
        .reader()
        .unwrap()
        .select("SELECT ?s ?p ?o WHERE { ?s ?p ?o }")
        .unwrap()
        .rows
        .into_iter()
        .map(|row| {
            let mut it = row.into_iter().map(|t| t.expect("bound"));
            Fact::new(it.next().unwrap(), it.next().unwrap(), it.next().unwrap())
        })
        .collect()
}

/// A full build of the library's current state into a fresh store, read back.
pub fn rebuilt_facts(lib: &Library, name: &str) -> BTreeSet<Fact> {
    let store = new_store(&format!("rebuilt-{name}"));
    run(
        &lib.reader(),
        &store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
    all_facts(&store)
}

/// Facts in one set and not the other, as N-Triples lines, for failure messages.
pub fn differences(a: &BTreeSet<Fact>, b: &BTreeSet<Fact>) -> String {
    let lines = |x: &BTreeSet<Fact>, y: &BTreeSet<Fact>| -> Vec<String> {
        x.difference(y).take(15).map(Fact::to_ntriples).collect()
    };
    format!(
        "only in the update ({}):\n{}\nonly in the rebuild ({}):\n{}",
        a.difference(b).count(),
        lines(a, b).join("\n"),
        b.difference(a).count(),
        lines(b, a).join("\n")
    )
}

/// The owners per kind the store holds, counted with the writer (spec 009 T036).
pub fn owner_counts(store: &GraphStore) -> engine::build::stamp::Items {
    let mut items = engine::build::stamp::Items::default();
    for o in store.writer().unwrap().owners().unwrap() {
        if let Some(kind) = engine::build::update::owner_kind(&o) {
            items.add(kind, 1);
        }
    }
    items
}

/// The `items` of the current generation's build stamp.
pub fn stamp_items(store: &GraphStore) -> Option<engine::build::stamp::Items> {
    let generation = store.reader().unwrap().generation().clone();
    engine::build::stamp::BuildStamp::read(&generation)
        .unwrap()
        .items
}
