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

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A Stash timestamp (`2026-10-21T10:00:00-06:00`) one second earlier, in the same text form and
/// offset. Stash compares `updated_at` filters as text to the second, so a catch-up threshold one
/// second early also catches items saved in the same second (003 findings).
pub fn one_second_earlier(stamp: &str) -> Option<String> {
    let (dt, offset) = stamp.split_at_checked(19)?;
    let n = |r: std::ops::Range<usize>| dt.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, s) = (n(11..13)?, n(14..16)?, n(17..19)?);
    if dt.as_bytes().get(10) != Some(&b'T') || !(offset == "Z" || offset.len() == 6) {
        return None;
    }
    let secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s - 1;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    Some(format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}{offset}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    ))
}

impl Journal {
    /// Removes the pending marker, for when starting the job failed, so the next save retries.
    pub fn clear_pending(&self) -> io::Result<()> {
        match fs::remove_file(self.dir.join(PENDING)) {
            Err(e) if !not_found(&e) => Err(e),
            _ => Ok(()),
        }
    }
}
