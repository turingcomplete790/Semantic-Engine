//! The spike's on-disk store and the three sharing options of research R2.
//!
//! Layout under the store directory (data-model "Snapshot generation"):
//!
//! ```text
//! work/            writer's read-write store (only the lock holder opens it)
//! snapshots/<n>/   published checkpoints, never written after publication
//! current          the published generation number, replaced by atomic rename
//! writer.lock      advisory lock held for the length of a write
//! assets/          snapshot files served by Stash
//! ```
//!
//! The writer lock uses the standard library's advisory file locks (stable since Rust 1.89),
//! which behave the same on Linux, macOS, and Windows, so no locking crate is needed.

use oxigraph::store::Store;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    /// Fail at once with [`StoreError::Busy`] when another writer holds the lock.
    Fail,
    /// Wait up to the given time, then fail with [`StoreError::LockTimeout`].
    Wait(Duration),
}

#[derive(Debug)]
pub enum StoreError {
    Busy,
    LockTimeout,
    /// No generation has been published yet.
    NoCurrent,
    /// `current` is unreadable or names a generation that cannot be opened.
    DamagedCurrent(String),
    Io(io::Error),
    Storage(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Busy => write!(f, "another writer holds the store lock"),
            StoreError::LockTimeout => write!(f, "timed out waiting for the store lock"),
            StoreError::NoCurrent => write!(f, "no snapshot has been published yet"),
            StoreError::DamagedCurrent(m) => write!(f, "the published snapshot is damaged: {m}"),
            StoreError::Io(e) => write!(f, "{e}"),
            StoreError::Storage(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

fn storage<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Storage(e.to_string())
}

pub fn work_dir(dir: &Path) -> PathBuf {
    dir.join("work")
}
pub fn snapshots_dir(dir: &Path) -> PathBuf {
    dir.join("snapshots")
}
pub fn assets_dir(dir: &Path) -> PathBuf {
    dir.join("assets")
}

/// The holder of the writer lock and the read-write working store. Dropping it releases the
/// lock (the lock file is closed).
pub struct Writer {
    dir: PathBuf,
    store: Store,
    _lock: File,
}

impl Writer {
    pub fn store(&self) -> &Store {
        &self.store
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Takes `writer.lock` (by `mode`) and opens `work/` read-write.
pub fn open_writer(dir: &Path, mode: LockMode) -> Result<Writer, StoreError> {
    let lock = lock_writer(dir, mode)?;
    let store = Store::open(work_dir(dir)).map_err(storage)?;
    Ok(Writer {
        dir: dir.to_path_buf(),
        store,
        _lock: lock,
    })
}

/// Takes only the writer lock, for option C, where every call (reads included) serialises on
/// it. The lock is released when the returned file is dropped.
pub fn lock_writer(dir: &Path, mode: LockMode) -> Result<File, StoreError> {
    fs::create_dir_all(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("writer.lock"))?;
    let start = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) => match mode {
                LockMode::Fail => return Err(StoreError::Busy),
                LockMode::Wait(max) if start.elapsed() >= max => {
                    return Err(StoreError::LockTimeout)
                }
                LockMode::Wait(_) => thread::sleep(Duration::from_millis(5)),
            },
            Err(TryLockError::Error(e)) => return Err(StoreError::Io(e)),
        }
    }
}

fn generations(dir: &Path) -> Result<Vec<u64>, StoreError> {
    let mut out = Vec::new();
    match fs::read_dir(snapshots_dir(dir)) {
        Ok(rd) => {
            for e in rd {
                if let Some(n) = e?.file_name().to_str().and_then(|s| s.parse().ok()) {
                    out.push(n);
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    out.sort_unstable();
    Ok(out)
}

/// The published generation number, if any.
pub fn current_generation(dir: &Path) -> Result<u64, StoreError> {
    let text = match fs::read_to_string(dir.join("current")) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(StoreError::NoCurrent),
        Err(e) => return Err(StoreError::DamagedCurrent(e.to_string())),
    };
    text.trim()
        .parse()
        .map_err(|_| StoreError::DamagedCurrent(format!("`current` holds {:?}", text.trim())))
}

/// Publishes the working store as a new generation: flush, checkpoint into
/// `snapshots/<n+1>/` (hard links on the same filesystem), then atomically replace `current`.
/// Returns the new generation number. The caller holds the lock for the whole write.
pub fn publish(w: &Writer) -> Result<u64, StoreError> {
    w.store.flush().map_err(storage)?;
    let last = generations(&w.dir)?.last().copied().unwrap_or(0);
    let next = last.max(current_generation(&w.dir).unwrap_or(0)) + 1;
    fs::create_dir_all(snapshots_dir(&w.dir))?;
    w.store
        .backup(snapshots_dir(&w.dir).join(next.to_string()))
        .map_err(storage)?;
    let tmp = w.dir.join("current.tmp");
    fs::write(&tmp, format!("{next}\n"))?;
    fs::rename(&tmp, w.dir.join("current"))?;
    Ok(next)
}

/// Opens the published generation read-only (option B readers). Returns its number too.
pub fn open_current_read_only(dir: &Path) -> Result<(u64, Store), StoreError> {
    let n = current_generation(dir)?;
    let path = snapshots_dir(dir).join(n.to_string());
    if !path.is_dir() {
        return Err(StoreError::DamagedCurrent(format!(
            "generation {n} is missing from snapshots/"
        )));
    }
    let store = Store::open_read_only(&path)
        .map_err(|e| StoreError::DamagedCurrent(format!("generation {n}: {e}")))?;
    Ok((n, store))
}

/// Opens the live working store read-only (option A). Oxigraph documents this as undefined
/// behaviour while another process writes the store (research R2); the spike measures what
/// actually happens and does not rely on it.
pub fn open_live_read_only(dir: &Path) -> Result<Store, StoreError> {
    Store::open_read_only(work_dir(dir)).map_err(storage)
}

/// Removes generations older than `grace`, always keeping the newest two and the current one.
/// Returns the removed generation numbers in ascending order.
pub fn gc(dir: &Path, grace: Duration) -> Result<Vec<u64>, StoreError> {
    let gens = generations(dir)?;
    let current = current_generation(dir).ok();
    let keep_from = gens.len().saturating_sub(2);
    let now = SystemTime::now();
    let mut removed = Vec::new();
    for &g in &gens[..keep_from] {
        if Some(g) == current {
            continue;
        }
        let path = snapshots_dir(dir).join(g.to_string());
        let age = fs::metadata(&path)?
            .modified()
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .unwrap_or_default();
        if age >= grace {
            fs::remove_dir_all(&path)?;
            removed.push(g);
        }
    }
    Ok(removed)
}

/// Total size in bytes of the files under `path` (hard-linked files are counted once per link).
pub fn disk_size(path: &Path) -> io::Result<u64> {
    let mut total = 0;
    if path.is_dir() {
        for e in fs::read_dir(path)? {
            total += disk_size(&e?.path())?;
        }
    } else if path.exists() {
        total += fs::metadata(path)?.len();
    }
    Ok(total)
}
