//! The engine's graph store: Oxigraph embedded, on the RocksDB it bundles, behind an interface
//! the engine owns (spec 006; constitution Principles IV and V; 003 decision record rules 1–3).
//!
//! One writer holds `writer.lock` and changes a working store; publishing turns its state into
//! a numbered, read-only generation and switches `current` with one rename. Readers open only
//! the current generation and keep it for their whole call. The files are described in
//! [`layout`]. The rules every caller can rely on (G1 to G9 of
//! `specs/006-graph-store/contracts/graph-store-interface.md`):
//!
//! - G1: readers only ever open a published generation, never `work/`;
//! - G2: a [`Reader`] answers from one generation for its whole life;
//! - G3: a [`Writer`] exists only while its process holds the lock; a second gets `Busy`;
//! - G4: publishing is atomic for readers;
//! - G5: a writer dropped without publishing, or a crashed process, changes nothing readers
//!   see, and the next writer starts from the current generation;
//! - G6: every entry point checks the stamp and the current generation first, and a store that
//!   needs a rebuild is never read or written; [`GraphStore::clear_for_rebuild`] is the way out;
//! - G7: publishing keeps the current generation, the one before it, and those younger than
//!   [`GENERATION_GRACE`];
//! - G8: nothing here contacts a host or writes a credential;
//! - G9: no Oxigraph type appears in this module's public interface.

mod error;
mod health;
mod layout;
mod reader;
mod term;
mod writer;

pub use error::{GraphError, Health, RebuildReason};
pub use layout::Generation;
pub use reader::Reader;
pub(crate) use term::from_ox_triple;
pub use term::{round_trip, validate, Fact, Owned, Pattern, Replaced, Solutions, Term};
pub use writer::Writer;

use health::Inspection;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The store layout version (research R2). A store of any other format needs a rebuild.
pub const STORE_FORMAT: u32 = 2;
/// The minimum age before an old generation is removed (research R5).
pub const GENERATION_GRACE: Duration = Duration::from_secs(15 * 60);
/// The store's folder under the plugin's folder.
pub const STORE_DIR: &str = "graph";

/// The graph store of one Stash server.
pub struct GraphStore {
    dir: PathBuf,
    server_id: String,
}

/// The report of spec 006 US4 (FR-018).
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub health: Health,
    /// The current generation, when there is one.
    pub generation: Option<Generation>,
    pub format: u32,
    pub server_id: Option<String>,
    /// The whole store folder.
    pub disk_bytes: u64,
    /// Whether a writer holds the lock now.
    pub writing: bool,
}

impl GraphStore {
    /// `dir` is `{PluginDir}/graph`; `server_id` is the plugin setting `serverId` (spec 002).
    pub fn new(dir: impl Into<PathBuf>, server_id: &str) -> GraphStore {
        GraphStore {
            dir: dir.into(),
            server_id: server_id.to_string(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn need_server_id(&self) -> Result<(), GraphError> {
        if self.server_id.is_empty() {
            Err(GraphError::NoServerId)
        } else {
            Ok(())
        }
    }

    /// Is the store usable, and if not, why (research R4)? Never writes.
    pub fn check(&self) -> Result<Health, GraphError> {
        health::health(&self.dir, &self.server_id)
    }

    /// Opens the current generation read-only, for one call (FR-010, FR-011).
    pub fn reader(&self) -> Result<Reader, GraphError> {
        self.need_server_id()?;
        match health::inspect(&self.dir, &self.server_id) {
            Inspection::Ready(generation, store) => Ok(Reader {
                generation: *generation,
                store,
            }),
            Inspection::NotBuilt => Err(GraphError::NotBuilt),
            Inspection::NeedsRebuild(r) => Err(GraphError::NeedsRebuild(r)),
        }
    }

    /// Takes the writer lock without waiting (FR-004). A store that needs a rebuild is refused;
    /// a store that is built, or stamped but never published, can be written.
    pub fn writer(&self) -> Result<Writer, GraphError> {
        self.need_server_id()?;
        match health::inspect(&self.dir, &self.server_id) {
            Inspection::NeedsRebuild(r) => return Err(GraphError::NeedsRebuild(r)),
            Inspection::Ready(..) | Inspection::NotBuilt => {}
        }
        let lock = writer::try_lock(&self.dir)?;
        Writer::open(&self.dir, &self.server_id, lock)
    }

    /// Like [`GraphStore::writer`], but starting from an empty working store, for a full
    /// rebuild (spec 007 research R3). The published generations are untouched until
    /// `publish()`; a writer dropped before then leaves the store dirty, so the next writer
    /// starts again from the current generation.
    pub fn rebuild_writer(&self) -> Result<Writer, GraphError> {
        self.need_server_id()?;
        match health::inspect(&self.dir, &self.server_id) {
            Inspection::NeedsRebuild(r) => return Err(GraphError::NeedsRebuild(r)),
            Inspection::Ready(..) | Inspection::NotBuilt => {}
        }
        let lock = writer::try_lock(&self.dir)?;
        Writer::open_empty(&self.dir, &self.server_id, lock)
    }

    /// Empties the store, whatever its state, and stamps it for this server and format
    /// (FR-016). It takes the writer lock, so it gives `Busy` while a writer works.
    pub fn clear_for_rebuild(&self) -> Result<(), GraphError> {
        self.need_server_id()?;
        let _lock = writer::try_lock(&self.dir)?;
        for path in [
            layout::current_path(&self.dir),
            layout::dirty_path(&self.dir),
            layout::work_dir(&self.dir),
            layout::snapshots_dir(&self.dir),
            layout::stamp_path(&self.dir),
        ] {
            layout::remove_if_present(&path)?;
        }
        layout::write_stamp(
            &self.dir,
            &layout::Stamp {
                format: STORE_FORMAT,
                server_id: self.server_id.clone(),
                created_at: layout::now(),
            },
        )
    }

    /// What the store holds (FR-018). It reads only small files and never waits for a writer.
    pub fn summary(&self) -> Result<Summary, GraphError> {
        let health = self.check()?;
        let stamp = layout::read_stamp(&self.dir).ok().flatten();
        let generation = match health {
            Health::Ready { generation } => layout::read_generation(&self.dir, generation).ok(),
            _ => None,
        };
        Ok(Summary {
            generation,
            format: stamp.as_ref().map_or(STORE_FORMAT, |s| s.format),
            server_id: stamp
                .filter(|s| s.server_id == self.server_id)
                .map(|s| s.server_id),
            disk_bytes: layout::disk_size(&self.dir)?,
            writing: writer::locked(&self.dir),
            health,
        })
    }
}
