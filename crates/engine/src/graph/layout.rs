//! The store's files (spec 006 research R2):
//!
//! ```text
//! store.json           stamp: format, serverId, createdAt
//! work/                the working store (only the lock holder opens it)
//! work.dirty           present while the working store has unpublished changes
//! snapshots/<n>/       published generation n (an Oxigraph checkpoint), never written again
//! snapshots/<n>.json   generation n's record: number, publication time, facts, format, server
//! current              the published generation number, replaced by atomic rename
//! writer.lock          advisory lock held for the length of a write
//! ```

use super::error::GraphError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn stamp_path(dir: &Path) -> PathBuf {
    dir.join("store.json")
}
pub(crate) fn work_dir(dir: &Path) -> PathBuf {
    dir.join("work")
}
pub(crate) fn dirty_path(dir: &Path) -> PathBuf {
    dir.join("work.dirty")
}
pub(crate) fn snapshots_dir(dir: &Path) -> PathBuf {
    dir.join("snapshots")
}
pub(crate) fn generation_dir(dir: &Path, n: u64) -> PathBuf {
    snapshots_dir(dir).join(n.to_string())
}
pub(crate) fn generation_file(dir: &Path, n: u64) -> PathBuf {
    snapshots_dir(dir).join(format!("{n}.json"))
}
pub(crate) fn current_path(dir: &Path) -> PathBuf {
    dir.join("current")
}
pub(crate) fn lock_path(dir: &Path) -> PathBuf {
    dir.join("writer.lock")
}

/// The stamp checked before every read (FR-014).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Stamp {
    pub format: u32,
    pub server_id: String,
    pub created_at: String,
}

/// One published generation (data-model "Generation").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Generation {
    #[serde(rename = "generation")]
    pub number: u64,
    /// UTC RFC 3339 text.
    pub published_at: String,
    pub facts: u64,
    pub format: u32,
    pub server_id: String,
    /// What cleanup could not do at this publish (research R5); empty when all went well.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// What made this generation (spec 008 research R7). Opaque here: the builder defines it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<serde_json::Value>,
}

pub(crate) fn now() -> String {
    stash_adapter::reader::time::rfc3339_utc(std::time::SystemTime::now())
}

/// Writes `text` to `path` through a temporary file and a rename, so readers see the old or
/// the new content, never part of it.
pub(crate) fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

pub(crate) fn write_stamp(dir: &Path, stamp: &Stamp) -> Result<(), GraphError> {
    let text = serde_json::to_string_pretty(stamp).map_err(GraphError::internal)?;
    Ok(write_atomic(&stamp_path(dir), &(text + "\n"))?)
}

/// `Ok(None)` when there is no stamp; `Err(detail)` when it cannot be read.
pub(crate) fn read_stamp(dir: &Path) -> Result<Option<Stamp>, String> {
    match fs::read_to_string(stamp_path(dir)) {
        Ok(t) => serde_json::from_str(&t)
            .map(Some)
            .map_err(|e| format!("store.json: {e}")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("store.json: {e}")),
    }
}

/// `Ok(None)` when nothing has been published; `Err(detail)` when `current` is unreadable.
pub(crate) fn read_current(dir: &Path) -> Result<Option<u64>, String> {
    match fs::read_to_string(current_path(dir)) {
        Ok(t) => t
            .trim()
            .parse()
            .map(Some)
            .map_err(|_| format!("`current` holds {:?}", t.trim())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("current: {e}")),
    }
}

pub(crate) fn write_current(dir: &Path, n: u64) -> io::Result<()> {
    write_atomic(&current_path(dir), &format!("{n}\n"))
}

pub(crate) fn write_generation(dir: &Path, g: &Generation) -> Result<(), GraphError> {
    let mut record = g.clone();
    record.notes.clear();
    let text = serde_json::to_string_pretty(&record).map_err(GraphError::internal)?;
    Ok(write_atomic(
        &generation_file(dir, g.number),
        &(text + "\n"),
    )?)
}

pub(crate) fn read_generation(dir: &Path, n: u64) -> Result<Generation, String> {
    let t = fs::read_to_string(generation_file(dir, n))
        .map_err(|e| format!("generation {n} record: {e}"))?;
    serde_json::from_str(&t).map_err(|e| format!("generation {n} record: {e}"))
}

/// The generation numbers with a folder in `snapshots/`, ascending.
pub(crate) fn generations(dir: &Path) -> io::Result<Vec<u64>> {
    let mut out = Vec::new();
    match fs::read_dir(snapshots_dir(dir)) {
        Ok(rd) => {
            for e in rd {
                let e = e?;
                if e.file_type()?.is_dir() {
                    if let Some(n) = e.file_name().to_str().and_then(|s| s.parse().ok()) {
                        out.push(n);
                    }
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    out.sort_unstable();
    Ok(out)
}

/// Total size in bytes of the files under `path`.
pub(crate) fn disk_size(path: &Path) -> io::Result<u64> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    if meta.is_dir() {
        let mut total = 0;
        for e in fs::read_dir(path)? {
            total += disk_size(&e?.path())?;
        }
        Ok(total)
    } else {
        Ok(meta.len())
    }
}

/// Copies generation `n` into `dest` for a writer to continue from. Table files (`.sst`) are
/// never modified after RocksDB writes them, so they are hard-linked when possible; every other
/// file is copied, so the working store can never change the published generation.
pub(crate) fn copy_generation(dir: &Path, n: u64, dest: &Path) -> io::Result<()> {
    let src = generation_dir(dir, n);
    fs::create_dir_all(dest)?;
    for e in fs::read_dir(&src)? {
        let e = e?;
        let to = dest.join(e.file_name());
        let is_table = e.path().extension().is_some_and(|x| x == "sst");
        if !(is_table && fs::hard_link(e.path(), &to).is_ok()) {
            fs::copy(e.path(), &to)?;
        }
    }
    Ok(())
}

pub(crate) fn remove_if_present(path: &Path) -> io::Result<()> {
    let r = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match r {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}
