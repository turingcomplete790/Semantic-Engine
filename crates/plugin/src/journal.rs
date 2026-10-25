//! The change journal, the pending-job marker, and the catch-up watermark (spec 004 research R6,
//! R7; data-model "Journal and run state"). All three are plain files in the plugin's folder: a
//! disposable cache (constitution Principle I).
//!
//! The hook appends each saved item to the journal and starts the "Apply relations" job only
//! when it creates the marker, so a burst of saves starts one job. The job drains the journal,
//! removes the marker, and checks again for entries that arrived meanwhile. A marker left by a
//! crashed job goes stale after [`STALE_AFTER`] and no longer blocks new jobs.

use serde_json::{json, Value};
use stash_adapter::relations::MediaKind;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// How old a pending marker may be before the next hook treats its job as gone.
pub const STALE_AFTER: Duration = Duration::from_secs(15 * 60);

const JOURNAL: &str = "journal.jsonl";
const DRAINING: &str = "journal.draining";
const PENDING: &str = "apply.pending";
const WATERMARK: &str = "apply-watermark";

pub struct Journal {
    dir: PathBuf,
}

fn not_found(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotFound
}

impl Journal {
    pub fn new(dir: impl Into<PathBuf>) -> Journal {
        Journal { dir: dir.into() }
    }

    /// Appends one saved item. A single small append is atomic, so concurrent hooks never
    /// interleave lines.
    pub fn append(&self, kind: MediaKind, id: &str) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let line = format!("{}\n", json!({ "type": kind.as_str(), "id": id }));
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(JOURNAL))?
            .write_all(line.as_bytes())
    }

    /// Creates the pending marker. True when this call created it (or replaced a stale one), so
    /// the caller should start the job; false when a job is already pending.
    pub fn mark_pending(&self) -> io::Result<bool> {
        fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(PENDING);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let age = fs::metadata(&path)?
                    .modified()
                    .ok()
                    .and_then(|m| SystemTime::now().duration_since(m).ok())
                    .unwrap_or_default();
                if age >= STALE_AFTER {
                    fs::File::create(&path)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Takes every journal entry (plus any left by a drain that crashed), each item once in
    /// first-seen order. The journal is taken by renaming it, so hooks keep appending to a new one.
    pub fn drain(&self) -> io::Result<Vec<(MediaKind, String)>> {
        let draining = self.dir.join(DRAINING);
        let mut text = match fs::read_to_string(&draining) {
            Ok(t) => t,
            Err(e) if not_found(&e) => String::new(),
            Err(e) => return Err(e),
        };
        match fs::rename(self.dir.join(JOURNAL), &draining) {
            Ok(()) => text.push_str(&fs::read_to_string(&draining)?),
            Err(e) if not_found(&e) => {}
            Err(e) => return Err(e),
        }
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let (Some(kind), Some(id)) = (
                v["type"].as_str().and_then(MediaKind::parse),
                v["id"]
                    .as_str()
                    .map(String::from)
                    .or_else(|| v["id"].as_i64().map(|n| n.to_string())),
            ) else {
                continue;
            };
            if seen.insert((kind, id.clone())) {
                out.push((kind, id));
            }
        }
        match fs::remove_file(&draining) {
            Ok(()) => {}
            Err(e) if not_found(&e) => {}
            Err(e) => return Err(e),
        }
        Ok(out)
    }

    /// Ends a drain: removes the marker, then checks whether entries arrived meanwhile. If they
    /// did, it re-creates the marker and returns true, and the job must loop.
    pub fn finish(&self) -> io::Result<bool> {
        match fs::remove_file(self.dir.join(PENDING)) {
            Ok(()) => {}
            Err(e) if not_found(&e) => {}
            Err(e) => return Err(e),
        }
        let more = fs::metadata(self.dir.join(JOURNAL)).is_ok_and(|m| m.len() > 0);
        if more {
            fs::File::create(self.dir.join(PENDING))?;
        }
        Ok(more)
    }

    /// The `updated_at` of the newest item of each kind when the last successful run started.
    pub fn watermark(&self) -> io::Result<BTreeMap<MediaKind, String>> {
        match fs::read_to_string(self.dir.join(WATERMARK)) {
            Ok(t) => Ok(serde_json::from_str(&t).unwrap_or_default()),
            Err(e) if not_found(&e) => Ok(BTreeMap::new()),
            Err(e) => Err(e),
        }
    }

    pub fn set_watermark(&self, w: &BTreeMap<MediaKind, String>) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{WATERMARK}.tmp"));
        fs::write(&tmp, serde_json::to_string(w).expect("serializes"))?;
        fs::rename(tmp, self.dir.join(WATERMARK))
    }
}

/// One second before a Stash timestamp; the rule lives with the reader (spec 005).
pub use stash_adapter::reader::time::one_second_earlier;

impl Journal {
    /// Removes the pending marker, for when starting the job failed, so the next save retries.
    pub fn clear_pending(&self) -> io::Result<()> {
        match fs::remove_file(self.dir.join(PENDING)) {
            Err(e) if !not_found(&e) => Err(e),
            _ => Ok(()),
        }
    }
}
