//! The one writer (spec 006 FR-004 to FR-009, FR-017; research R3, R5).
//!
//! A writer holds `writer.lock` for its whole life; the operating system releases the lock
//! when the process ends, so a crashed writer never blocks the next one. Before its first
//! change it creates `work.dirty`. A writer that finds the marker knows the last one stopped
//! before publishing, and resets the working store to the current generation first, so a
//! half-applied change is never published.

use super::error::GraphError;
use super::layout::{self, Generation};
use super::term::{to_ox_owned_quad, to_ox_pattern, to_ox_quad, Fact, Owned, Pattern, Replaced};
use super::{GENERATION_GRACE, STORE_FORMAT};
use oxigraph::model::{GraphName, GraphNameRef, NamedNode, NamedOrBlankNode, Quad, Term};
use oxigraph::store::Store;
use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Batches at least this large use the bulk loader (research R3).
const BULK_BATCH: usize = 10_000;

pub struct Writer {
    dir: PathBuf,
    server_id: String,
    store: Store,
    dirty: bool,
    /// The store's fact count, kept as the writer goes (spec 008 research R2, rule G10). `None`
    /// after a default-graph change, when `publish` counts the store instead.
    count: Option<u64>,
    /// An in-memory store that puts facts in the form the store keeps them in, for comparing
    /// (Oxigraph stores integer-derived types as `xsd:integer`, and values canonically).
    scratch: Option<Store>,
    /// The generation the working store started from, when it started from one.
    base: Option<u64>,
    /// How publishing writes the generation's query index (spec 009 research R12).
    index: crate::qindex::Plan,
    _lock: File,
}

fn storage(e: impl std::fmt::Display) -> GraphError {
    GraphError::internal(e)
}

/// Takes `writer.lock` without waiting.
pub(crate) fn try_lock(dir: &Path) -> Result<File, GraphError> {
    fs::create_dir_all(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(layout::lock_path(dir))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(GraphError::Busy),
        Err(TryLockError::Error(e)) => Err(e.into()),
    }
}

/// True when another process holds the lock now. Never waits.
pub(crate) fn locked(dir: &Path) -> bool {
    let Ok(file) = OpenOptions::new().write(true).open(layout::lock_path(dir)) else {
        return false;
    };
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            false
        }
        Err(_) => true,
    }
}

impl Writer {
    /// Opens the working store with the lock already held. The caller has checked the stamp.
    pub(crate) fn open(dir: &Path, server_id: &str, lock: File) -> Result<Writer, GraphError> {
        let current = layout::read_current(dir).map_err(GraphError::internal)?;
        let work = layout::work_dir(dir);
        let needs_reset = layout::dirty_path(dir).exists() || (current.is_some() && !work.is_dir());
        if needs_reset {
            layout::remove_if_present(&work)?;
            if let Some(n) = current {
                layout::copy_generation(dir, n, &work)?;
            }
            layout::remove_if_present(&layout::dirty_path(dir))?;
        }
        let store = Store::open(&work).map_err(storage)?;
        let count = match current {
            Some(n) => layout::read_generation(dir, n).ok().map(|g| g.facts),
            None => Some(0),
        };
        Ok(Writer {
            dir: dir.to_path_buf(),
            server_id: server_id.to_string(),
            store,
            dirty: false,
            count,
            scratch: None,
            base: current,
            index: crate::qindex::Plan::Delta(BTreeSet::new()),
            _lock: lock,
        })
    }

    /// Opens an empty working store with the lock already held. The dirty marker is written
    /// first, so a crash before publish makes the next writer reset from the current generation.
    pub(crate) fn open_empty(
        dir: &Path,
        server_id: &str,
        lock: File,
    ) -> Result<Writer, GraphError> {
        fs::write(layout::dirty_path(dir), b"")?;
        let work = layout::work_dir(dir);
        layout::remove_if_present(&work)?;
        let store = Store::open(&work).map_err(storage)?;
        Ok(Writer {
            dir: dir.to_path_buf(),
            server_id: server_id.to_string(),
            store,
            dirty: true,
            count: Some(0),
            scratch: None,
            base: None,
            index: crate::qindex::Plan::Collect(crate::qindex::Builder::default()),
            _lock: lock,
        })
    }

    /// Notes, for the index, that `owner`'s facts changed (`None`: the default graph). A writer
    /// collecting every fact cannot follow a change to facts it already has, and rescans.
    fn changed(&mut self, owner: Option<&str>) {
        use crate::qindex::Plan;
        match &mut self.index {
            Plan::Delta(touched) => {
                touched.insert(owner.map(str::to_string));
            }
            Plan::Collect(_) => self.index = Plan::Rescan,
            Plan::Rescan => {}
        }
    }

    /// Adds facts to the index being collected, in the form the store keeps them.
    fn collect<'f>(&mut self, owner: Option<&str>, facts: impl IntoIterator<Item = &'f Fact>) {
        use super::term::canonical;
        match &mut self.index {
            crate::qindex::Plan::Collect(b) => {
                for f in facts {
                    b.add(&f.subject, &f.predicate, &canonical(&f.object), owner);
                }
            }
            crate::qindex::Plan::Delta(touched) => {
                touched.insert(owner.map(str::to_string));
            }
            crate::qindex::Plan::Rescan => {}
        }
    }

    /// Marks the working store as having unpublished changes, before the first one.
    fn touch(&mut self) -> Result<(), GraphError> {
        if !self.dirty {
            fs::write(layout::dirty_path(&self.dir), b"")?;
            self.dirty = true;
        }
        Ok(())
    }

    pub fn add(&mut self, facts: &[Fact]) -> Result<(), GraphError> {
        let quads = facts
            .iter()
            .map(to_ox_quad)
            .collect::<Result<Vec<_>, _>>()?;
        self.touch()?;
        self.count = None;
        self.collect(None, facts);
        // The bulk loader is not transactional, but the dirty marker makes that safe: a crash
        // leaves `work.dirty`, and the next writer starts over from the current generation.
        // Small batches go through a transaction, which is faster for a handful of facts.
        if quads.len() >= BULK_BATCH || self.store.is_empty().map_err(storage)? {
            self.store.bulk_loader().load_quads(quads).map_err(storage)
        } else {
            self.store.extend(quads).map_err(storage)
        }
    }

    pub fn remove(&mut self, facts: &[Fact]) -> Result<(), GraphError> {
        let quads = facts
            .iter()
            .map(to_ox_quad)
            .collect::<Result<Vec<_>, _>>()?;
        self.touch()?;
        self.count = None;
        self.changed(None);
        for q in &quads {
            self.store.remove(q).map_err(storage)?;
        }
        Ok(())
    }

    /// Removes every fact matching `pattern`; returns how many were removed.
    pub fn remove_matching(&mut self, pattern: &Pattern) -> Result<u64, GraphError> {
        let (s, p, o) = to_ox_pattern(pattern)?;
        self.touch()?;
        self.count = None;
        self.changed(None);
        let found = self
            .store
            .quads_for_pattern(
                s.as_ref().map(Into::into),
                p.as_ref().map(Into::into),
                o.as_ref().map(Into::into),
                Some(GraphNameRef::DefaultGraph),
            )
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        for q in &found {
            self.store.remove(q).map_err(storage)?;
        }
        Ok(found.len() as u64)
    }

    pub fn clear(&mut self) -> Result<(), GraphError> {
        self.touch()?;
        self.count = None;
        self.index = crate::qindex::Plan::Collect(crate::qindex::Builder::default());
        self.store.clear().map_err(storage)
    }

    fn graph(owner: &str) -> Result<NamedNode, GraphError> {
        NamedNode::new(owner).map_err(|e| GraphError::InvalidFact {
            message: format!("{owner:?} is not an absolute IRI: {e}"),
        })
    }

    /// The facts an owner's graph holds now.
    fn owned_quads(&self, graph: &NamedNode) -> Result<HashSet<Quad>, GraphError> {
        self.store
            .quads_for_pattern(None, None, None, Some(graph.as_ref().into()))
            .collect::<Result<HashSet<_>, _>>()
            .map_err(storage)
    }

    /// The quads as the store will hold them: written to the scratch store and read back.
    fn as_stored(&mut self, quads: Vec<Quad>) -> Result<HashSet<Quad>, GraphError> {
        if self.scratch.is_none() {
            self.scratch = Some(Store::new().map_err(storage)?);
        }
        let scratch = self.scratch.as_ref().expect("just made");
        scratch.extend(quads).map_err(storage)?;
        let out = scratch
            .iter()
            .collect::<Result<HashSet<_>, _>>()
            .map_err(storage)?;
        scratch.clear().map_err(storage)?;
        Ok(out)
    }

    /// Adds whole owners' facts, for builds; the owners' graphs must be empty, and each
    /// owner's facts distinct (contracts/graph-store-format-2.md).
    pub fn add_owned(&mut self, batch: &[Owned]) -> Result<(), GraphError> {
        let mut quads = Vec::new();
        for o in batch {
            let graph = Self::graph(&o.owner)?;
            for f in &o.facts {
                quads.push(to_ox_owned_quad(f, &graph)?);
            }
        }
        if quads.is_empty() {
            return Ok(());
        }
        self.touch()?;
        for o in batch {
            self.collect(Some(&o.owner), &o.facts);
        }
        let n = quads.len() as u64;
        if quads.len() >= BULK_BATCH || self.store.is_empty().map_err(storage)? {
            self.store
                .bulk_loader()
                .load_quads(quads)
                .map_err(storage)?;
        } else {
            self.store.extend(quads).map_err(storage)?;
        }
        self.count = self.count.map(|c| c + n);
        Ok(())
    }

    /// Makes the owner's graph hold exactly `facts`; writes nothing when it already does (G11).
    pub fn replace_owned(&mut self, owner: &str, facts: &[Fact]) -> Result<Replaced, GraphError> {
        let graph = Self::graph(owner)?;
        let old = self.owned_quads(&graph)?;
        let new = self.as_stored(
            facts
                .iter()
                .map(|f| to_ox_owned_quad(f, &graph))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        if old == new {
            return Ok(Replaced::Unchanged);
        }
        self.touch()?;
        self.changed(Some(owner));
        let removed: Vec<&Quad> = old.difference(&new).collect();
        let added: Vec<Quad> = new.difference(&old).cloned().collect();
        if new.is_empty() {
            self.store
                .remove_named_graph(graph.as_ref())
                .map_err(storage)?;
        } else {
            for q in &removed {
                self.store.remove(*q).map_err(storage)?;
            }
            self.store.extend(added.iter().cloned()).map_err(storage)?;
        }
        let (r, a) = (removed.len() as u64, added.len() as u64);
        self.count = self.count.map(|c| c - r + a);
        Ok(Replaced::Changed {
            removed: r,
            added: a,
        })
    }

    /// Removes the owner's graph; returns how many facts it held.
    pub fn remove_owned(&mut self, owner: &str) -> Result<u64, GraphError> {
        let graph = Self::graph(owner)?;
        let n = self.owned_quads(&graph)?.len() as u64;
        let known = self
            .store
            .contains_named_graph(graph.as_ref())
            .map_err(storage)?;
        if n == 0 && !known {
            return Ok(0);
        }
        self.touch()?;
        self.changed(Some(owner));
        self.store
            .remove_named_graph(graph.as_ref())
            .map_err(storage)?;
        self.count = self.count.map(|c| c - n);
        Ok(n)
    }

    /// Every owner in the working store, sorted (G13).
    pub fn owners(&self) -> Result<Vec<String>, GraphError> {
        let mut out: Vec<String> = self
            .store
            .named_graphs()
            .filter_map(|g| match g {
                Ok(NamedOrBlankNode::NamedNode(n)) => Some(Ok(n.into_string())),
                Ok(_) => None,
                Err(e) => Some(Err(storage(e))),
            })
            .collect::<Result<_, _>>()?;
        out.sort();
        Ok(out)
    }

    /// Owners whose graphs hold a fact with `object` as its object, sorted (G13).
    pub fn owners_referencing(&self, object: &str) -> Result<Vec<String>, GraphError> {
        let object: Term = Self::graph(object)?.into();
        let mut out = BTreeSet::new();
        for q in self
            .store
            .quads_for_pattern(None, None, Some(object.as_ref()), None)
        {
            if let GraphName::NamedNode(g) = q.map_err(storage)?.graph_name {
                out.insert(g.into_string());
            }
        }
        Ok(out.into_iter().collect())
    }

    /// Publishes the working store as the next generation, then cleans up old ones.
    pub fn publish(self) -> Result<Generation, GraphError> {
        self.publish_inner(None)
    }

    /// `publish`, also recording `build` in the generation file (spec 008 research R7).
    pub fn publish_with(self, build: serde_json::Value) -> Result<Generation, GraphError> {
        self.publish_inner(Some(build))
    }

    fn publish_inner(mut self, build: Option<serde_json::Value>) -> Result<Generation, GraphError> {
        let dir = self.dir.clone();
        self.store.flush().map_err(storage)?;
        let current = layout::read_current(&dir).map_err(GraphError::internal)?;
        let n = current.unwrap_or(0) + 1;
        // A crash between the checkpoint and the switch can leave this folder behind; the
        // number was never published, so it is reused (FR-007: no gaps).
        layout::remove_if_present(&layout::generation_dir(&dir, n))?;
        layout::remove_if_present(&layout::generation_file(&dir, n))?;
        fs::create_dir_all(layout::snapshots_dir(&dir))?;
        self.store
            .backup(layout::generation_dir(&dir, n))
            .map_err(storage)?;
        let facts = match self.count {
            Some(n) => {
                debug_assert_eq!(
                    Some(n as usize),
                    self.store.len().ok(),
                    "the kept fact count (G10)"
                );
                n
            }
            None => self.store.len().map_err(storage)? as u64,
        };
        let mut generation = Generation {
            number: n,
            published_at: layout::now(),
            facts,
            format: STORE_FORMAT,
            server_id: self.server_id.clone(),
            notes: Vec::new(),
            build,
        };
        // The query index, before the generation is switched to, so a published generation
        // has its index from the start. One that cannot be written is noted, and the generation
        // is answered by the store instead.
        let mut notes = Vec::new();
        if let Err(e) = self.write_index(n) {
            let _ = layout::remove_if_present(&crate::qindex::path(&dir, n));
            notes.push(format!("query index of generation {n} not written: {e}"));
        }
        layout::write_generation(&dir, &generation)?;
        layout::write_current(&dir, n)?;
        layout::remove_if_present(&layout::dirty_path(&dir))?;
        notes.extend(cleanup(&dir, n));
        generation.notes = notes;
        Ok(generation)
    }

    /// Writes generation `n`'s query index (spec 009 research R12).
    fn write_index(&mut self, n: u64) -> std::io::Result<()> {
        use crate::qindex::{self, Plan};
        let path = qindex::path(&self.dir, n);
        let base = self
            .base
            .and_then(|b| qindex::QIndex::open(&qindex::path(&self.dir, b)).ok());
        let quads = |store: &Store, graph: Option<GraphNameRef>| {
            store
                .quads_for_pattern(None, None, None, graph)
                .map(|q| q.map_err(std::io::Error::other))
                .collect::<Result<Vec<_>, _>>()
        };
        let owner = |q: &Quad| match &q.graph_name {
            GraphName::NamedNode(g) => Some(g.as_str().to_string()),
            _ => None,
        };
        let plan = std::mem::replace(&mut self.index, Plan::Rescan);
        match (plan, base) {
            (Plan::Collect(b), _) => b.write(&path),
            (Plan::Delta(touched), Some(base)) => {
                let mut added = Vec::new();
                for o in &touched {
                    let found = match o {
                        Some(o) => {
                            let g = NamedNode::new(o).map_err(std::io::Error::other)?;
                            quads(&self.store, Some(g.as_ref().into()))?
                        }
                        None => quads(&self.store, Some(GraphNameRef::DefaultGraph))?,
                    };
                    for q in &found {
                        let f = super::term::from_ox_quad(q);
                        added.push((f.subject, f.predicate, f.object, owner(q)));
                    }
                }
                qindex::merge(&base, &touched, added, &path)
            }
            // No base index to merge into, or changes the writer did not follow: every fact.
            (Plan::Delta(_), None) | (Plan::Rescan, _) => {
                let mut b = qindex::Builder::default();
                for q in quads(&self.store, None)? {
                    let f = super::term::from_ox_quad(&q);
                    b.add(&f.subject, &f.predicate, &f.object, owner(&q).as_deref());
                }
                b.write(&path)
            }
        }
    }
}

/// Removes generations no reader can still be using: every one except the current, the one
/// before it, and those younger than [`GENERATION_GRACE`]. A removal that fails (on Windows a
/// reader may still have the files open) is noted and retried at the next publish.
pub(crate) fn cleanup(dir: &Path, current: u64) -> Vec<String> {
    let mut notes = Vec::new();
    let gens = match layout::generations(dir) {
        Ok(g) => g,
        Err(e) => return vec![format!("could not list generations: {e}")],
    };
    let now = SystemTime::now();
    for g in gens {
        if g == current || g + 1 == current {
            continue;
        }
        let path = layout::generation_dir(dir, g);
        let age = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .unwrap_or_default();
        if age < GENERATION_GRACE {
            continue;
        }
        if let Err(e) = layout::remove_if_present(&path) {
            notes.push(format!("generation {g} kept for now: {e}"));
            continue;
        }
        let _ = layout::remove_if_present(&layout::generation_file(dir, g));
        let _ = layout::remove_if_present(&crate::qindex::path(dir, g));
    }
    notes
}
