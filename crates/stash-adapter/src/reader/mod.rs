//! The library reader: the one way the engine learns what a Stash library holds (spec 005,
//! contracts/reader-interface.md).
//!
//! A full read takes where it starts (the newest `updated_at` of each kind), finds the ids of
//! every semantic record with one "is not null" query per key (custom-field criteria cannot be
//! OR-ed in Stash), then reads the records by id in batches of [`RECORD_BATCH`]. Fields are parsed
//! from one key table ([`keys`]); a bad field is kept as unreadable and never fails the read.

pub mod client;
pub mod field;
pub mod keys;
pub mod measurements;
pub mod memory;
pub mod queries;
pub mod records;
pub mod time;

pub use memory::MemoryReader;
pub use records::{
    Kind, LibraryRead, MediaKind, MediaRecord, Phase, Progress, ReadStart, RecordBatch,
    SemanticPerformer, SemanticTag, TagFlag,
};

use crate::Error;

/// How many records one request reads (research R4).
pub const RECORD_BATCH: usize = 250;
/// How many tag ids one "performers with these tags" query carries (research R3).
pub const PERFORMER_TAG_CHUNK: usize = 500;

/// The reads of spec 005 FR-001 to FR-012. Implementations keep rules B1 to B9 of
/// contracts/reader-interface.md, and never write.
pub trait LibraryReader {
    /// The whole semantic library (FR-001 to FR-008). A stop gives `Err(Error::Stopped)`, never
    /// a partial read.
    fn full_read(
        &self,
        should_stop: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<LibraryRead, Error>;

    /// Records of named tag ids, in the full read's shape (FR-010).
    fn tag_records(&self, ids: &[String]) -> Result<RecordBatch<SemanticTag>, Error>;
    fn performer_records(&self, ids: &[String]) -> Result<RecordBatch<SemanticPerformer>, Error>;
    fn media_records(
        &self,
        kind: MediaKind,
        ids: &[String],
    ) -> Result<RecordBatch<MediaRecord>, Error>;

    /// Ids of `kind` with `updated_at` after `threshold`, in the server's own text (FR-009).
    fn changed_since(&self, kind: Kind, threshold: &str) -> Result<Vec<String>, Error>;

    /// Every id of `kind`, for the deletion check (FR-011).
    fn all_ids(&self, kind: Kind) -> Result<Vec<String>, Error>;

    /// The newest `updated_at` of `kind`, exactly as Stash stores it (FR-012).
    fn latest_updated_at(&self, kind: Kind) -> Result<Option<String>, Error>;
}
