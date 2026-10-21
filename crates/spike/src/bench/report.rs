//! `report`: a Markdown summary of `results/` to paste into the decision record (spec FR-001,
//! SC-002, tasks T041).
//!
//! Every Measurement file is read with its summary recomputed from `samples`. For each id the
//! newest run (by `takenAt`) is shown, with its difference from the previous run of the same id;
//! a median more than [`DRIFT`] away is flagged. Files that are not Measurements (`hooks.json`,
//! `seed-manifest.json`) are summarised on their own lines.

use super::{Bench, Exit, Failure, Outcome};
use crate::measure::{question_prefix, Measurement, Question};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The largest change in median between two runs of the same id that SC-002 allows.
pub const DRIFT: f64 = 0.25;

pub const QUESTIONS: [Question; 7] = [
    Question::Process,
    Question::Persistence,
    Question::Freshness,
    Question::Limits,
    Question::Snapshot,
    Question::Build,
    Question::Decision,
];

/// One id's runs, oldest first, with the file each came from.
pub struct Runs {
    pub runs: Vec<(String, Measurement)>,
}

impl Runs {
    pub fn newest(&self) -> &Measurement {
        &self.runs.last().expect("at least one run").1
    }
    pub fn previous(&self) -> Option<&Measurement> {
        self.runs.len().checked_sub(2).map(|i| &self.runs[i].1)
    }
    /// The relative change of the newest median from the previous one.
    pub fn drift(&self) -> Option<f64> {
        let (p, n) = (self.previous()?.median, self.newest().median);
        (p != 0.0).then(|| (n - p) / p)
    }
    pub fn flagged(&self) -> bool {
        self.drift().is_some_and(|d| d.abs() > DRIFT)
    }
}

/// Measurements grouped by id, and the skipped files with the reason.
pub type Loaded = (BTreeMap<String, Runs>, Vec<(String, String)>);

/// Every Measurement in `dir`, grouped by id. Files that fail to parse or have too few runs are
/// returned separately with the reason.
pub fn load(dir: &Path) -> Result<Loaded, std::io::Error> {
    let mut by_id: BTreeMap<String, Runs> = BTreeMap::new();
    let mut rejected = Vec::new();
    let mut names: Vec<String> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    for name in names {
        let path = dir.join(&name);
        let is_measurement = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| v.get("samples").is_some());
        if !is_measurement {
            continue;
        }
        match Measurement::load(&path) {
            Ok(m) => by_id
                .entry(m.id.clone())
                .or_insert_with(|| Runs { runs: Vec::new() })
                .runs
                .push((name, m)),
            Err(e) => rejected.push((name, e.to_string())),
        }
    }
    for r in by_id.values_mut() {
        r.runs.sort_by(|a, b| a.1.taken_at.cmp(&b.1.taken_at));
    }
    Ok((by_id, rejected))
}

fn fmt_ms(v: f64) -> String {
    match v {
        v if v >= 10_000.0 => format!("{:.1} s", v / 1000.0),
        v if v >= 100.0 => format!("{v:.0} ms"),
        v => format!("{v:.2} ms"),
    }
}

/// The Markdown summary.
pub fn render(dir: &Path) -> Result<String, std::io::Error> {
    let (by_id, rejected) = load(dir)?;
    let mut out = String::from("# Spike results summary\n\n");
    out.push_str(&format!(
        "From `{}`. Newest run per id; summaries recomputed from samples. A median that moved more than {:.0}% from the previous run is flagged (SC-002).\n",
        dir.display(),
        DRIFT * 100.0
    ));
    for q in QUESTIONS {
        let rows: Vec<&Runs> = by_id
            .values()
            .filter(|r| r.newest().question == q)
            .collect();
        if rows.is_empty() {
            continue;
        }
        out.push_str(&format!("\n## {}\n\n", question_prefix(q)));
        out.push_str("| id | kind | runs | median | p95 | previous median | change | file |\n");
        out.push_str("|---|---|---|---|---|---|---|---|\n");
        for r in rows {
            let m = r.newest();
            let file = &r.runs.last().expect("a run").0;
            let (prev, change) = match (r.previous(), r.drift()) {
                (Some(p), Some(d)) => (
                    fmt_ms(p.median),
                    format!(
                        "{:+.1}%{}",
                        d * 100.0,
                        if r.flagged() { " **over 25%**" } else { "" }
                    ),
                ),
                _ => ("—".into(), "—".into()),
            };
            out.push_str(&format!(
                "| {} | {:?} | {} | {} | {} | {} | {} | [{file}](results/{file}) |\n",
                m.id,
                m.kind,
                m.runs,
                fmt_ms(m.median),
                fmt_ms(m.p95),
                prev,
                change,
            ));
        }
    }

    let mut other = Vec::new();
    if let Some(h) = read(dir, "hooks.json") {
        other.push(format!(
            "- `hooks.json`: {} hook observation rows.",
            h["rows"].as_array().map_or(0, Vec::len)
        ));
    }
    if let Some(s) = read(dir, "seed-manifest.json") {
        other.push(format!(
            "- `seed-manifest.json`: state `{}`.",
            s["state"].as_str().unwrap_or("unknown")
        ));
    }
    for (name, why) in &rejected {
        other.push(format!("- `{name}` was skipped: {why}."));
    }
    if !other.is_empty() {
        out.push_str("\n## Other files\n\n");
        out.push_str(&other.join("\n"));
        out.push('\n');
    }
    let flagged: Vec<&str> = by_id
        .values()
        .filter(|r| r.flagged())
        .map(|r| r.newest().id.as_str())
        .collect();
    out.push_str(&format!(
        "\n**Reruns over 25%:** {}\n",
        if flagged.is_empty() {
            "none".to_string()
        } else {
            flagged.join(", ")
        }
    ));
    Ok(out)
}

fn read(dir: &Path, name: &str) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(dir.join(name)).ok()?).ok()
}

pub fn command(b: &mut Bench) -> Outcome {
    let text = render(&b.global.out)
        .map_err(|e| Failure::new(Exit::Usage, format!("{}: {e}", b.global.out.display())))?;
    b.say(text.trim_end());
    Ok(())
}
