//! Measurement records written to `specs/003-engine-design-spike/results/`
//! (data-model "Measurement", "Option", "Hook observation").

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The seven Phase 1 roadmap items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Question {
    Process,
    Persistence,
    Freshness,
    Limits,
    Snapshot,
    Build,
    Decision,
}

/// What a figure is, which sets how many runs it needs: latency series need 100 (spec FR-003),
/// build and size figures 3, observations (such as first calls after a reload) at least 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Latency,
    Build,
    Size,
    Observation,
}

impl Kind {
    pub fn min_runs(self) -> usize {
        match self {
            Kind::Latency => 100,
            Kind::Build | Kind::Size => 3,
            Kind::Observation => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub stash_version: Option<String>,
    /// The test server URL, or `local` when no Stash was involved.
    pub host: String,
}

impl Server {
    pub fn local() -> Server {
        Server {
            stash_version: None,
            host: "local".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    /// `test` or `synthetic`.
    pub kind: String,
    pub scenes: u64,
    pub images: u64,
    pub performers: u64,
    pub tags: u64,
    pub triples: u64,
}

impl Library {
    pub fn test(scenes: u64, images: u64, performers: u64, tags: u64, triples: u64) -> Library {
        Library {
            kind: "test".into(),
            scenes,
            images,
            performers,
            tags,
            triples,
        }
    }
    pub fn synthetic(
        scenes: u64,
        images: u64,
        performers: u64,
        tags: u64,
        triples: u64,
    ) -> Library {
        Library {
            kind: "synthetic".into(),
            ..Library::test(scenes, images, performers, tags, triples)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    pub run: u64,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    /// `<question>-<variant>-<scale>`, for example `persist-B-250k`.
    pub id: String,
    pub question: Question,
    #[serde(default)]
    pub variant: String,
    pub kind: Kind,
    /// The exact `spike-bench` command, with no credentials.
    pub command: String,
    pub server: Server,
    pub library: Library,
    pub runs: usize,
    pub samples: Vec<f64>,
    pub median: f64,
    pub p95: f64,
    pub min: f64,
    pub max: f64,
    /// Optional breakdown, for example `{ processStart, storeOpen, query, serialise }`; each part
    /// is an array of per-run values or a summary object.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub parts: Map<String, Value>,
    #[serde(default)]
    pub failures: Vec<Failure>,
    pub taken_at: String,
    pub engine_commit: String,
    /// Free-form extra results (curves, proposed page sizes, observed failures).
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

#[derive(Debug)]
pub enum MeasureError {
    TooFewRuns { runs: usize, needed: usize },
    Io(io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for MeasureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeasureError::TooFewRuns { runs, needed } => {
                write!(f, "{runs} runs recorded, at least {needed} needed")
            }
            MeasureError::Io(e) => write!(f, "{e}"),
            MeasureError::Json(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MeasureError {}

impl From<io::Error> for MeasureError {
    fn from(e: io::Error) -> Self {
        MeasureError::Io(e)
    }
}
impl From<serde_json::Error> for MeasureError {
    fn from(e: serde_json::Error) -> Self {
        MeasureError::Json(e)
    }
}

/// The value at percentile `p` (0 to 100) by linear interpolation between closest ranks.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    match sorted.len() {
        0 => 0.0,
        1 => sorted[0],
        n => {
            let rank = p / 100.0 * (n - 1) as f64;
            let lo = rank.floor() as usize;
            let hi = rank.ceil() as usize;
            sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64)
        }
    }
}

impl Measurement {
    pub fn new(
        id: &str,
        question: Question,
        kind: Kind,
        command: &str,
        server: Server,
        library: Library,
    ) -> Measurement {
        Measurement {
            id: id.into(),
            question,
            variant: String::new(),
            kind,
            command: command.into(),
            server,
            library,
            runs: 0,
            samples: Vec::new(),
            median: 0.0,
            p95: 0.0,
            min: 0.0,
            max: 0.0,
            parts: Map::new(),
            failures: Vec::new(),
            taken_at: now_rfc3339(),
            engine_commit: engine_commit(),
            extra: Map::new(),
        }
    }

    pub fn with_samples(mut self, samples: Vec<f64>) -> Measurement {
        self.samples = samples;
        self.recompute();
        self
    }

    /// Derives `runs`, `median`, `p95`, `min`, and `max` from `samples`, so they can never
    /// disagree with the raw data.
    pub fn recompute(&mut self) {
        let mut s = self.samples.clone();
        s.sort_by(|a, b| a.total_cmp(b));
        self.runs = s.len();
        self.median = percentile(&s, 50.0);
        // Nearest-rank p95, the usual latency convention.
        self.p95 = if s.is_empty() {
            0.0
        } else {
            s[((0.95 * s.len() as f64).ceil() as usize).clamp(1, s.len()) - 1]
        };
        self.min = s.first().copied().unwrap_or(0.0);
        self.max = s.last().copied().unwrap_or(0.0);
    }

    pub fn validate(&self) -> Result<(), MeasureError> {
        let needed = self.kind.min_runs();
        if self.samples.len() < needed {
            return Err(MeasureError::TooFewRuns {
                runs: self.samples.len(),
                needed,
            });
        }
        Ok(())
    }

    pub fn add_failure(&mut self, run: u64, kind: &str, message: &str) {
        self.failures.push(Failure {
            run,
            kind: kind.into(),
            message: message.into(),
        });
    }

    /// Reads a result file, recomputing the summary from `samples` and checking the run count.
    pub fn load(path: &Path) -> Result<Measurement, MeasureError> {
        let mut m: Measurement = serde_json::from_str(&fs::read_to_string(path)?)?;
        m.recompute();
        m.validate()?;
        Ok(m)
    }
}

/// A latency series that keeps the first call after each plugin reload out of the samples
/// (spec Edge Cases): those calls include Stash re-reading plugin configs.
#[derive(Debug, Default)]
pub struct Series {
    samples: Vec<f64>,
    first_calls: Vec<f64>,
    after_reload: bool,
}

impl Series {
    pub fn mark_reload(&mut self) {
        self.after_reload = true;
    }

    pub fn push(&mut self, value: f64) {
        if std::mem::take(&mut self.after_reload) {
            self.first_calls.push(value);
        } else {
            self.samples.push(value);
        }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// The main measurement, plus a `process-first-call` observation when any reload happened.
    pub fn into_measurements(self, base: Measurement) -> (Measurement, Option<Measurement>) {
        let first = (!self.first_calls.is_empty()).then(|| {
            let mut f = base.clone().with_samples(self.first_calls);
            f.id = format!("{}-first-call", question_prefix(base.question));
            f.variant = "first-call".into();
            f.kind = Kind::Observation;
            f.parts = Map::new();
            f
        });
        (base.with_samples(self.samples), first)
    }
}

/// The id prefix for a question (`process`, `persist`, `freshness`, ...).
pub fn question_prefix(q: Question) -> &'static str {
    match q {
        Question::Process => "process",
        Question::Persistence => "persist",
        Question::Freshness => "freshness",
        Question::Limits => "limits",
        Question::Snapshot => "snapshot",
        Question::Build => "build",
        Question::Decision => "decision",
    }
}

/// Replaces every non-empty secret in `text` with `[redacted]`.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for s in secrets.iter().filter(|s| !s.is_empty()) {
        out = out.replace(s, "[redacted]");
    }
    out
}

/// Writes `m` to `dir/<id>.json`, or `dir/<id>-<timestamp>.json` when that exists; never
/// overwrites. Secrets are removed from the whole file. Returns the path written.
pub fn write_result(
    dir: &Path,
    m: &Measurement,
    secrets: &[&str],
) -> Result<PathBuf, MeasureError> {
    let mut m = m.clone();
    m.recompute();
    m.validate()?;
    write_json(dir, &m.id, &serde_json::to_value(&m)?, secrets)
}

/// Writes any JSON document under the same no-overwrite and redaction rules.
pub fn write_json(
    dir: &Path,
    id: &str,
    value: &Value,
    secrets: &[&str],
) -> Result<PathBuf, MeasureError> {
    fs::create_dir_all(dir)?;
    let text = redact(&serde_json::to_string_pretty(value)?, secrets) + "\n";
    let mut path = dir.join(format!("{id}.json"));
    let stamp = timestamp_basic();
    let mut n = 1;
    while path.exists() {
        path = if n == 1 {
            dir.join(format!("{id}-{stamp}.json"))
        } else {
            dir.join(format!("{id}-{stamp}-{n}.json"))
        };
        n += 1;
    }
    fs::write(&path, text)?;
    Ok(path)
}

/// UTC civil date and time from a Unix timestamp (Howard Hinnant's days-from-civil inverse).
fn civil(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        m,
        d,
        (rem / 3600) as u32,
        (rem % 3600 / 60) as u32,
        (rem % 60) as u32,
    )
}

fn now_secs() -> (u64, u32) {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (d.as_secs(), d.subsec_millis())
}

/// The current UTC time in RFC 3339, with milliseconds.
pub fn now_rfc3339() -> String {
    let (s, ms) = now_secs();
    let (y, mo, d, h, mi, se) = civil(s);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{se:02}.{ms:03}Z")
}

/// RFC 3339 for a Unix time in seconds (used for `updated_at` filters).
pub fn rfc3339_from_secs(secs: u64) -> String {
    let (y, mo, d, h, mi, se) = civil(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{se:02}Z")
}

/// A Unix time in seconds as Stash stores `updated_at`: RFC 3339 in the server's local time,
/// for example `2026-10-17T17:57:53-06:00`. Stash compares filter values as text, so a UTC
/// (`Z`) value silently matches the wrong items; thresholds must use this form.
pub fn stash_time(secs: u64, offset_secs: i64) -> String {
    let (y, mo, d, h, mi, se) = civil((secs as i64 + offset_secs) as u64);
    let sign = if offset_secs < 0 { '-' } else { '+' };
    let off = offset_secs.unsigned_abs();
    format!(
        "{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{se:02}{sign}{:02}:{:02}",
        off / 3600,
        off % 3600 / 60
    )
}

/// The UTC offset in seconds of a Stash timestamp such as `2026-10-17T17:57:53-06:00`.
pub fn offset_of(stamp: &str) -> Option<i64> {
    if stamp.ends_with('Z') {
        return Some(0);
    }
    let tail = stamp.get(stamp.len().checked_sub(6)?..)?;
    let sign = match tail.as_bytes()[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let h: i64 = tail.get(1..3)?.parse().ok()?;
    let m: i64 = tail.get(4..6)?.parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

/// The current UTC time in RFC 3339 basic format, for file names.
pub fn timestamp_basic() -> String {
    let (s, _) = now_secs();
    let (y, mo, d, h, mi, se) = civil(s);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{se:02}Z")
}

/// The workspace's git commit, with `-dirty` when there are uncommitted changes.
pub fn engine_commit() -> String {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    match git(&["rev-parse", "HEAD"]) {
        Some(sha) => {
            let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
                .is_some_and(|s| !s.is_empty());
            if dirty {
                format!("{sha}-dirty")
            } else {
                sha
            }
        }
        None => "unknown".into(),
    }
}

/// A candidate answer to one question (data-model "Option"); named so it does not shadow
/// `std::option::Option`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionEvaluated {
    pub question: Question,
    pub name: String,
    pub measurements: Vec<String>,
    pub observed_failures: Vec<String>,
    /// `meets`, `needs-amendment`, or `violates` (with the principle).
    pub constitution: String,
    /// `chosen`, `rejected`, or `fallback`.
    pub verdict: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Entity {
    Tag,
    Scene,
    Image,
    Performer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SavingClient {
    #[serde(rename = "web-ui")]
    WebUi,
    #[serde(rename = "semantic-tagging")]
    SemanticTagging,
    #[serde(rename = "viewer")]
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Scripted,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookReceived {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub id: Option<i64>,
    #[serde(default)]
    pub received_ms: f64,
    #[serde(default)]
    pub handler_ms: f64,
    #[serde(default)]
    pub input_fields: Vec<String>,
    pub input_bytes: u64,
    pub received_at: String,
}

/// One save and the hooks it produced (data-model "Hook observation").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookObservation {
    pub entity: Entity,
    pub client: SavingClient,
    pub mode: Mode,
    pub mutation: String,
    pub items: u64,
    pub hooks_received: Vec<HookReceived>,
    /// From save request sent to last hook received, in ms.
    pub delay: Option<f64>,
    /// Median client-visible save time with the spike's hook enabled, in ms (none for manual
    /// rows and for rows where the client sends no save).
    pub save_latency: Option<f64>,
    pub save_latency_no_hook: Option<f64>,
    pub updated_at_bumped: Option<bool>,
    pub missed: bool,
    pub out_of_order: bool,
    /// Saves timed for the latency figures.
    #[serde(default)]
    pub repeats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
