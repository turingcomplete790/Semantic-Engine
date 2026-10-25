//! Measures the graph store's budgets (spec 006 research R8; quickstart step 2). Development only:
//! `scripts/package.sh` packages only the plugin binary, and CI rejects this one in the package.
//!
//! usage: graph-bench build  --scenes N --dir DIR
//!        graph-bench open   --dir DIR [--runs N]
//!        graph-bench query  --dir DIR [--runs N]
//!        graph-bench disk   --dir DIR
//!        graph-bench stress --dir DIR [--readers R] [--seconds S]
//!        [--out DIR]   where results go (default specs/006-graph-store/results)
//!
//! The synthetic graph has the shape of the spike's (`crates/spike/src/synth.rs`), so the
//! figures compare with its 7–8 ms open and 88.7 ms query p95 at 250k scenes. Per scene:
//! 2 performers; per performer a role and an action instance (with one variation), and a
//! pluggable instance for the first. Images: 1.3 per scene, each with 1 performer and 1 role
//! instance. Performers: one per 50 scenes, at least 6, each with a Trait and a Label.

use engine::graph::{Fact, GraphStore, Term};
use semantic_vocabulary::iri::ItemIris;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

const VOCAB: &str = "urn:semantic-stash:vocab:";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
const SERVER_ID: &str = "00000000-0000-4000-8000-000000000001";
const TAGS_PER_TYPE: u64 = 6;
const TAG_TYPES: [&str; 7] = [
    "Pluggable",
    "Action",
    "Role",
    "Trait",
    "Label",
    "MiseEnScene",
    "Variation",
];
const VARIATIONS: [&str; 4] = ["slow", "fast", "playful", "intense"];
/// Scenes written per batch while building.
const BATCH_SCENES: u64 = 20_000;
/// The stress check's block: facts every generation rewrites.
const STRESS_BLOCK: usize = 200;

fn fail(msg: &str) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(1)
}

// ---- The synthetic graph

/// SplitMix64, so the same scene count always gives the same graph.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn v(local: &str) -> Term {
    Term::iri(format!("{VOCAB}{local}"))
}

fn f(s: &str, p: Term, o: Term) -> Fact {
    Fact::new(Term::iri(s), p, o)
}

fn tag_id(class: &str, i: u64) -> String {
    let t = TAG_TYPES.iter().position(|c| *c == class).expect("class") as u64;
    (1001 + t * TAGS_PER_TYPE + i % TAGS_PER_TYPE).to_string()
}

fn random_tag(rng: &mut Rng, class: &str) -> String {
    tag_id(class, rng.below(TAGS_PER_TYPE))
}

fn performers_for(scenes: u64) -> u64 {
    (scenes / 50).max(6)
}

fn tag_facts(iris: &ItemIris) -> Vec<Fact> {
    let mut out = Vec::new();
    for class in TAG_TYPES {
        for i in 0..TAGS_PER_TYPE {
            let tag = iris.tag(&tag_id(class, i));
            out.push(f(&tag, Term::iri(RDF_TYPE), v(class)));
            out.push(f(
                &tag,
                Term::iri(RDFS_LABEL),
                Term::string(format!("{class} {i}")),
            ));
        }
    }
    out
}

fn performer_facts(iris: &ItemIris, rng: &mut Rng, performers: u64) -> Vec<Fact> {
    let mut out = Vec::new();
    for p in 1..=performers {
        let perf = iris.performer(&p.to_string());
        out.push(f(&perf, Term::iri(RDF_TYPE), v("Performer")));
        out.push(f(
            &perf,
            Term::iri(RDFS_LABEL),
            Term::string(format!("Performer {p}")),
        ));
        out.push(f(
            &perf,
            v("hasTrait"),
            Term::iri(iris.tag(&random_tag(rng, "Trait"))),
        ));
        out.push(f(
            &perf,
            v("hasLabel"),
            Term::iri(iris.tag(&random_tag(rng, "Label"))),
        ));
    }
    out
}

fn instance(
    out: &mut Vec<Fact>,
    iris: &ItemIris,
    media: &str,
    pid: &str,
    kind: &str,
    tag: &str,
    variation: Option<&str>,
) {
    let perf = iris.performer(pid);
    let (inst, class, link) = match kind {
        "role" => (
            iris.role_instance(media, pid, tag),
            "RoleInstance",
            "hasRole",
        ),
        "action" => (
            iris.action_instance(media, pid, tag),
            "ActionInstance",
            "hasAction",
        ),
        _ => (
            iris.pluggable_instance(media, pid, tag),
            "PluggableInstance",
            "hasPluggable",
        ),
    };
    out.push(f(&perf, v(link), Term::iri(inst.clone())));
    out.push(f(&inst, Term::iri(RDF_TYPE), v(class)));
    out.push(f(&inst, v("inMedia"), Term::iri(media)));
    out.push(f(&inst, v("forPerformer"), Term::iri(perf)));
    out.push(f(&inst, v("ofTag"), Term::iri(iris.tag(tag))));
    if let Some(var) = variation {
        out.push(f(&inst, v("hasVariation"), Term::string(var)));
    }
}

fn scene_facts(
    iris: &ItemIris,
    rng: &mut Rng,
    range: std::ops::Range<u64>,
    performers: u64,
) -> Vec<Fact> {
    let mut out = Vec::new();
    for s in range {
        let scene = iris.scene(&(s + 1).to_string());
        out.push(f(&scene, Term::iri(RDF_TYPE), v("Scene")));
        let a = rng.below(performers) + 1;
        let b = (a % performers) + 1;
        for (k, p) in [a, b].into_iter().enumerate() {
            let pid = p.to_string();
            out.push(f(
                &scene,
                v("hasPerformer"),
                Term::iri(iris.performer(&pid)),
            ));
            let role = random_tag(rng, "Role");
            instance(&mut out, iris, &scene, &pid, "role", &role, None);
            let action = random_tag(rng, "Action");
            let var = VARIATIONS[rng.below(VARIATIONS.len() as u64) as usize];
            instance(&mut out, iris, &scene, &pid, "action", &action, Some(var));
            if k == 0 {
                let plug = random_tag(rng, "Pluggable");
                instance(&mut out, iris, &scene, &pid, "pluggable", &plug, None);
            }
        }
    }
    out
}

fn image_facts(
    iris: &ItemIris,
    rng: &mut Rng,
    range: std::ops::Range<u64>,
    performers: u64,
) -> Vec<Fact> {
    let mut out = Vec::new();
    for i in range {
        let image = iris.image(&(i + 1).to_string());
        let pid = (rng.below(performers) + 1).to_string();
        out.push(f(&image, Term::iri(RDF_TYPE), v("Image")));
        out.push(f(
            &image,
            v("hasPerformer"),
            Term::iri(iris.performer(&pid)),
        ));
        let role = random_tag(rng, "Role");
        instance(&mut out, iris, &image, &pid, "role", &role, None);
    }
    out
}

// ---- Measuring

fn summary(mut ms: Vec<f64>) -> Value {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| ms[((ms.len() - 1) as f64 * q).round() as usize];
    json!({ "runs": ms.len(), "min": ms[0], "median": at(0.5), "p95": at(0.95), "max": ms[ms.len() - 1] })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Peak resident memory in KiB (`VmHWM`, Linux only).
fn peak_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// Bytes on disk under `path`, counting each hard-linked file once (Linux and macOS).
fn disk_bytes(path: &Path, seen: &mut std::collections::BTreeSet<(u64, u64)>) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_dir() {
        let mut total = 0;
        if let Ok(rd) = std::fs::read_dir(path) {
            for e in rd.flatten() {
                total += disk_bytes(&e.path(), seen);
            }
        }
        return total;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if !seen.insert((meta.dev(), meta.ino())) {
            return 0;
        }
    }
    meta.len()
}

fn scenes_of(dir: &Path) -> Option<u64> {
    let text = std::fs::read_to_string(dir.join("bench.json")).ok()?;
    serde_json::from_str::<Value>(&text).ok()?["scenes"].as_u64()
}

fn queries() -> Vec<(&'static str, String)> {
    let iris = ItemIris::new(SERVER_ID).expect("example server id");
    let t = |class: &str| format!("<{}>", iris.tag(&tag_id(class, 0)));
    let (role, action, tr) = (t("Role"), t("Action"), t("Trait"));
    let p = "PREFIX sem: <urn:semantic-stash:vocab:>\n";
    vec![
        ("1 pattern: scenes", format!("{p}SELECT ?scene WHERE {{ ?scene a sem:Scene }} LIMIT 100")),
        ("2 patterns: media with role", format!("{p}SELECT ?media WHERE {{ ?r sem:ofTag {role} . ?r sem:inMedia ?media }} LIMIT 100")),
        ("3 patterns: performers with role", format!("{p}SELECT ?p ?media WHERE {{ ?p sem:hasRole ?r . ?r sem:ofTag {role} . ?r sem:inMedia ?media }} LIMIT 100")),
        ("4 patterns: role and action in one scene", format!("{p}SELECT ?scene WHERE {{ ?r sem:ofTag {role} . ?r sem:inMedia ?scene . ?a sem:ofTag {action} . ?a sem:inMedia ?scene }} LIMIT 100")),
        ("5 patterns: trait holder with role", format!("{p}SELECT ?p ?scene WHERE {{ ?p sem:hasTrait {tr} . ?p sem:hasRole ?r . ?r sem:ofTag {role} . ?r sem:inMedia ?scene . ?scene a sem:Scene }} LIMIT 100")),
    ]
}

fn build(dir: &Path, scenes: u64) -> Result<Value, String> {
    let store = GraphStore::new(dir, SERVER_ID);
    store.clear_for_rebuild().map_err(|e| e.to_string())?;
    let iris = ItemIris::new(SERVER_ID).expect("example server id");
    let performers = performers_for(scenes);
    let images = scenes * 1327 / 1021;
    let start = Instant::now();
    let mut w = store.writer().map_err(|e| e.to_string())?;
    let mut written = 0u64;
    let mut add = |w: &mut engine::graph::Writer, facts: Vec<Fact>| -> Result<(), String> {
        written += facts.len() as u64;
        w.add(&facts).map_err(|e| e.to_string())
    };
    add(&mut w, tag_facts(&iris))?;
    add(
        &mut w,
        performer_facts(&iris, &mut Rng(0x5045_5246), performers),
    )?;
    let mut rng = Rng(0x5343_454E);
    let mut s = 0;
    while s < scenes {
        let end = (s + BATCH_SCENES).min(scenes);
        add(&mut w, scene_facts(&iris, &mut rng, s..end, performers))?;
        s = end;
    }
    let mut rng = Rng(0x494D_4147);
    let mut i = 0;
    let image_batch = BATCH_SCENES * 1327 / 1021;
    while i < images {
        let end = (i + image_batch).min(images);
        add(&mut w, image_facts(&iris, &mut rng, i..end, performers))?;
        i = end;
    }
    let load_ms = ms(start.elapsed());
    let t = Instant::now();
    let g = w.publish().map_err(|e| e.to_string())?;
    let publish_ms = ms(t.elapsed());
    std::fs::write(
        dir.join("bench.json"),
        json!({ "scenes": scenes }).to_string(),
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({
        "scenes": scenes, "images": images, "performers": performers,
        "factsWritten": written, "factsStored": g.facts,
        "loadMs": load_ms, "publishMs": publish_ms, "totalMs": load_ms + publish_ms,
        "generation": g.number, "peakKiB": peak_kib(),
    }))
}

fn open(dir: &Path, runs: usize) -> Result<Value, String> {
    let mut t = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        let store = GraphStore::new(dir, SERVER_ID);
        let r = store.reader().map_err(|e| e.to_string())?;
        t.push(ms(start.elapsed()));
        drop(r);
    }
    Ok(json!({ "openMs": summary(t) }))
}

fn query(dir: &Path, runs: usize) -> Result<Value, String> {
    let mut per = serde_json::Map::new();
    let mut all = Vec::new();
    for (name, q) in queries() {
        let mut t = Vec::new();
        let mut rows = 0;
        for _ in 0..runs {
            // A whole call, as an engine process does it: open, then query.
            let start = Instant::now();
            let r = GraphStore::new(dir, SERVER_ID)
                .reader()
                .map_err(|e| e.to_string())?;
            rows = r.select(&q).map_err(|e| e.to_string())?.rows.len();
            t.push(ms(start.elapsed()));
        }
        all.extend(t.iter().copied());
        per.insert(name.into(), json!({ "rows": rows, "callMs": summary(t) }));
    }
    Ok(json!({ "queries": per, "allCallsMs": summary(all) }))
}

fn disk(dir: &Path) -> Result<Value, String> {
    let store = GraphStore::new(dir, SERVER_ID);
    let s = store.summary().map_err(|e| e.to_string())?;
    let g = s.generation.ok_or("no generation published")?;
    let generation = disk_bytes(
        &dir.join(format!("snapshots/{}", g.number)),
        &mut Default::default(),
    );
    let total = disk_bytes(dir, &mut Default::default());
    let scenes = scenes_of(dir).unwrap_or(0);
    Ok(json!({
        "generationBytes": generation, "storeBytes": total,
        "storeToGeneration": total as f64 / generation.max(1) as f64,
        "bytesPerScenePerGeneration": if scenes > 0 { generation as f64 / scenes as f64 } else { 0.0 },
        "facts": g.facts, "scenes": scenes,
    }))
}

fn stress_block(n: u64) -> Vec<Fact> {
    (0..STRESS_BLOCK)
        .map(|i| {
            f(
                &format!("urn:bench:block/{i}"),
                Term::iri("urn:bench:gen"),
                Term::string(n.to_string()),
            )
        })
        .collect()
}

/// One reader process of `stress`: reads until `stress.done` appears; prints its counts.
fn stress_reader(dir: &Path) -> ExitCode {
    let store = GraphStore::new(dir, SERVER_ID);
    let (mut reads, mut bad, mut errors) = (0u64, 0u64, 0u64);
    while !dir.join("stress.done").exists() {
        match store.reader().and_then(|r| {
            let n = r.generation().number.to_string();
            r.select("SELECT ?v WHERE { ?b <urn:bench:gen> ?v }")
                .map(|s| (n, s))
        }) {
            Ok((n, s)) => {
                reads += 1;
                let ok = s.rows.len() == STRESS_BLOCK
                    && s.rows.iter().all(
                        |row| matches!(&row[0], Some(Term::Literal { value, .. }) if *value == n),
                    );
                if !ok {
                    bad += 1;
                }
            }
            Err(_) => errors += 1,
        }
    }
    println!(
        "{}",
        json!({ "reads": reads, "bad": bad, "errors": errors })
    );
    ExitCode::SUCCESS
}

fn stress(dir: &Path, readers: usize, seconds: u64) -> Result<Value, String> {
    let store = GraphStore::new(dir, SERVER_ID);
    let first = store
        .summary()
        .map_err(|e| e.to_string())?
        .generation
        .map_or(0, |g| g.number);
    let mut w = store.writer().map_err(|e| e.to_string())?;
    w.remove_matching(&engine::graph::Pattern {
        predicate: Some(Term::iri("urn:bench:gen")),
        ..Default::default()
    })
    .map_err(|e| e.to_string())?;
    w.add(&stress_block(first + 1)).map_err(|e| e.to_string())?;
    w.publish().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(dir.join("stress.done"));
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let children: Vec<_> = (0..readers)
        .map(|_| {
            Command::new(&exe)
                .args(["stress-reader", "--dir"])
                .arg(dir)
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut publish_ms = Vec::new();
    let mut last = first + 1;
    while Instant::now() < deadline {
        let mut w = store.writer().map_err(|e| e.to_string())?;
        w.remove(&stress_block(last)).map_err(|e| e.to_string())?;
        w.add(&stress_block(last + 1)).map_err(|e| e.to_string())?;
        let t = Instant::now();
        last = w.publish().map_err(|e| e.to_string())?.number;
        publish_ms.push(ms(t.elapsed()));
    }
    std::fs::write(dir.join("stress.done"), b"").map_err(|e| e.to_string())?;
    let (mut reads, mut bad, mut errors) = (0u64, 0u64, 0u64);
    for c in children {
        let out = c.wait_with_output().map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
        reads += v["reads"].as_u64().unwrap_or(0);
        bad += v["bad"].as_u64().unwrap_or(0);
        errors += v["errors"].as_u64().unwrap_or(0);
    }
    let _ = std::fs::remove_file(dir.join("stress.done"));
    Ok(json!({
        "readers": readers, "seconds": seconds, "generationsPublished": publish_ms.len(),
        "publishMs": summary(publish_ms), "reads": reads, "partialReads": bad, "errors": errors,
    }))
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return fail("usage: graph-bench build|open|query|disk|stress --dir DIR [options]");
    };
    let (mut dir, mut scenes, mut runs, mut readers, mut seconds) =
        (None::<PathBuf>, None::<u64>, 20usize, 4usize, 30u64);
    let mut out = PathBuf::from("specs/006-graph-store/results");
    let rest: Vec<String> = args.collect();
    for pair in rest.chunks(2) {
        let value = pair.get(1).cloned();
        let number = value.as_deref().and_then(|v| v.parse::<u64>().ok());
        match pair[0].as_str() {
            "--dir" => dir = value.map(PathBuf::from),
            "--out" => out = value.map(PathBuf::from).unwrap_or(out),
            "--scenes" => scenes = number,
            "--runs" => runs = number.map_or(runs, |n| n as usize),
            "--readers" => readers = number.map_or(readers, |n| n as usize),
            "--seconds" => seconds = number.unwrap_or(seconds),
            other => return fail(&format!("unknown argument: {other}")),
        }
    }
    let Some(dir) = dir else {
        return fail("--dir DIR is required");
    };
    if command == "stress-reader" {
        return stress_reader(&dir);
    }
    let result = match command.as_str() {
        "build" => match scenes {
            Some(n) if n > 0 => build(&dir, n),
            _ => return fail("build needs --scenes N"),
        },
        "open" => open(&dir, runs),
        "query" => query(&dir, runs),
        "disk" => disk(&dir),
        "stress" => stress(&dir, readers, seconds),
        other => return fail(&format!("unknown command: {other}")),
    };
    let result = match result {
        Ok(r) => r,
        Err(e) => return fail(&e),
    };
    let scenes = scenes.or_else(|| scenes_of(&dir)).unwrap_or(0);
    let doc = json!({
        "command": std::env::args().skip(1).collect::<Vec<_>>().join(" "),
        "scenes": scenes,
        "result": result,
    });
    let text = serde_json::to_string_pretty(&doc).expect("JSON serializes") + "\n";
    println!("{text}");
    if std::fs::create_dir_all(&out).is_ok() {
        let path = out.join(format!("graph-{command}-{scenes}.json"));
        if std::fs::write(&path, &text).is_ok() {
            println!("wrote {}", path.display());
        }
    }
    ExitCode::SUCCESS
}
