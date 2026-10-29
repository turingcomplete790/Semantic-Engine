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

/// What [`GraphStore::describe`] finds (spec 009 research R9).
#[derive(Debug, Clone, PartialEq)]
pub enum Description {
    Ready(Generation),
    /// No store, or a store stamped but never published.
    NotBuilt,
    NeedsRebuild(RebuildReason),
}

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

    /// The store in `dir`, for the server its own stamp names. Queries use it, so they never
    /// ask Stash for the server id (spec 009 research R2): the folder is the plugin's own, and
    /// the stamp is checked on every read as usual. A missing or unreadable stamp is the
    /// rebuild reason it gives.
    pub fn stamped(dir: impl Into<PathBuf>) -> Result<GraphStore, GraphError> {
        let dir = dir.into();
        match layout::read_stamp(&dir) {
            Ok(Some(s)) => Ok(GraphStore::new(dir, &s.server_id)),
            Ok(None) => Err(GraphError::NeedsRebuild(RebuildReason::Missing)),
            Err(detail) => Err(GraphError::NeedsRebuild(RebuildReason::Damaged { detail })),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn server_id(&self) -> &str {
        &self.server_id
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

    /// Opens published generation `n` read-only, while it is kept (G7), for the later pages of
    /// one query (spec 009 research R6). The stamp is checked as for [`GraphStore::reader`];
    /// a generation that is not kept, or not published, is [`GraphError::GenerationGone`]. It
    /// never opens `work/`.
    pub fn reader_at(&self, n: u64) -> Result<Reader, GraphError> {
        self.need_server_id()?;
        if let Some(r) = health::stamp_problem(&self.dir, &self.server_id) {
            return Err(GraphError::NeedsRebuild(r));
        }
        let gone = GraphError::GenerationGone { generation: n };
        let published = layout::read_current(&self.dir)
            .ok()
            .flatten()
            .is_some_and(|current| n <= current);
        let path = layout::generation_dir(&self.dir, n);
        if !published || !path.is_dir() {
            return Err(gone);
        }
        let generation = layout::read_generation(&self.dir, n).map_err(|_| gone.clone())?;
        let store = oxigraph::store::Store::open_read_only(&path).map_err(|_| gone)?;
        Ok(Reader { generation, store })
    }

    /// The record of published generation `n` while it is kept, with the stamp checked, without
    /// opening the store: for queries answered on the generation's query index (spec 009
    /// research R12). Not kept or not published: [`GraphError::GenerationGone`].
    pub fn kept_generation(&self, n: u64) -> Result<Generation, GraphError> {
        self.need_server_id()?;
        if let Some(r) = health::stamp_problem(&self.dir, &self.server_id) {
            return Err(GraphError::NeedsRebuild(r));
        }
        let gone = GraphError::GenerationGone { generation: n };
        let published = layout::read_current(&self.dir)
            .ok()
            .flatten()
            .is_some_and(|current| n <= current);
        if !published || !layout::generation_dir(&self.dir, n).is_dir() {
            return Err(gone);
        }
        layout::read_generation(&self.dir, n).map_err(|_| gone)
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

    /// The store's state from its small files only: the stamp, `current` and the current
    /// generation's record (spec 009 research R9). It never opens a generation and never waits
    /// for a writer, so `status` stays as cheap as it was. With an empty server id the stamp's
    /// server is not checked. A missing stamp is `NotBuilt`.
    pub fn describe(&self) -> Description {
        let stamp = match layout::read_stamp(&self.dir) {
            Ok(None) => return Description::NotBuilt,
            Ok(Some(s)) => s,
            Err(detail) => return Description::NeedsRebuild(RebuildReason::Damaged { detail }),
        };
        if stamp.format != STORE_FORMAT {
            return Description::NeedsRebuild(RebuildReason::OtherFormat {
                found: stamp.format,
                expected: STORE_FORMAT,
            });
        }
        if !self.server_id.is_empty() && stamp.server_id != self.server_id {
            return Description::NeedsRebuild(RebuildReason::OtherServer);
        }
        let damaged = |detail: String| Description::NeedsRebuild(RebuildReason::Damaged { detail });
        let n = match layout::read_current(&self.dir) {
            Ok(None) => return Description::NotBuilt,
            Ok(Some(n)) => n,
            Err(detail) => return damaged(detail),
        };
        if !layout::generation_dir(&self.dir, n).is_dir() {
            return damaged(format!("generation {n} is missing from snapshots/"));
        }
        match layout::read_generation(&self.dir, n) {
            Ok(g) => Description::Ready(g),
            Err(detail) => damaged(detail),
        }
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
