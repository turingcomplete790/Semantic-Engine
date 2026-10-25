//! The one writer (spec 006 FR-004 to FR-009, FR-017; research R3, R5).
//!
//! A writer holds `writer.lock` for its whole life; the operating system releases the lock
//! when the process ends, so a crashed writer never blocks the next one. Before its first
//! change it creates `work.dirty`. A writer that finds the marker knows the last one stopped
//! before publishing, and resets the working store to the current generation first, so a
//! half-applied change is never published.

use super::error::GraphError;
use super::layout::{self, Generation};
use super::term::{to_ox_pattern, to_ox_quad, Fact, Pattern};
use super::{GENERATION_GRACE, STORE_FORMAT};
use oxigraph::model::GraphNameRef;
use oxigraph::store::Store;
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
        Ok(Writer {
            dir: dir.to_path_buf(),
            server_id: server_id.to_string(),
            store,
            dirty: false,
            _lock: lock,
        })
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
        for q in &quads {
            self.store.remove(q).map_err(storage)?;
        }
        Ok(())
    }

    /// Removes every fact matching `pattern`; returns how many were removed.
    pub fn remove_matching(&mut self, pattern: &Pattern) -> Result<u64, GraphError> {
        let (s, p, o) = to_ox_pattern(pattern)?;
        self.touch()?;
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
        self.store.clear().map_err(storage)
    }

    /// Publishes the working store as the next generation, then cleans up old ones.
    pub fn publish(self) -> Result<Generation, GraphError> {
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
        let facts = self.store.len().map_err(storage)? as u64;
        let mut generation = Generation {
            number: n,
            published_at: layout::now(),
            facts,
            format: STORE_FORMAT,
            server_id: self.server_id.clone(),
            notes: Vec::new(),
        };
        layout::write_generation(&dir, &generation)?;
        layout::write_current(&dir, n)?;
        layout::remove_if_present(&layout::dirty_path(&dir))?;
        generation.notes = cleanup(&dir, n);
        Ok(generation)
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
    }
    notes
}
