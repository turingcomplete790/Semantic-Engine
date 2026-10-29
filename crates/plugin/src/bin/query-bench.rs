//! Measures the `query` operation (spec 009 research R11; quickstart §2, §4). Development only:
//! `scripts/package.sh` packages only the plugin binary.
//!
//! usage: query-bench build --scenes N --dir DIR
//!        query-bench run   --dir DIR [--runs N] [--out FILE]
//!        query-bench fresh [--out FILE]   (STASH_URL and STASH_API_KEY in the environment)
//!
//! - `build` writes a full build of `engine::build::synth` into `DIR/graph`, with its query index
//!   (spec 009 research R12) as every build writes it, and with a small Role
//!   and Relation hierarchy added (`isRole 1` under `isRole 0`, `isRelation 1` under
//!   `isRelation 0`) and a relation on every tenth scene, so the recursive queries have narrower
//!   tags to expand.
//! - `run` runs the release plugin binary once per call, as Stash does, with `DIR` as the plugin
//!   folder and a server connection nothing listens on (`query` never calls Stash), over a fixed
//!   query set. It reports median, p95 and maximum per query and overall.
//! - `fresh` changes one scene's Roles on the test instance, calls `query` through Stash every
//!   0.5 s until the change shows, restores the scene, and reports the time (SC-007).

use engine::build::{run as build_run, synth, BuildOptions};
use engine::graph::GraphStore;
use serde_json::{json, Value};
use stash_adapter::reader::field::Field;
use stash_adapter::reader::TagFlag;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

const SERVER_ID: &str = "00000000-0000-4000-8000-000000000001";
const DEFAULT_OUT: &str = "specs/009-query-operations/results/query-250k.json";
const FRESH_OUT: &str = "specs/009-query-operations/results/fresh.json";

/// The fixed query set (tasks T041). Tag names are `synth`'s: `{flag key} {n}`.
const QUERIES: &[(&str, &str)] = &[
    (
        "plain",
        "SELECT ?scene WHERE { ?scene :hasPerformer ?p . ?p :hasRole :isRole%202 }",
    ),
    (
        "two performers, !=",
        "SELECT ?scene ?a ?b WHERE { ?scene :hasPerformer ?a . ?scene :hasPerformer ?b . \
         ?a :hasRole :isRole%202 . ?b :hasRole :isRole%203 . FILTER (?a != ?b) }",
    ),
    (
        "three performers",
        "SELECT ?scene ?a ?b ?c WHERE { ?scene :hasPerformer ?a , ?b , ?c . \
         FILTER (?a != ?b) FILTER (?b != ?c) FILTER (?a != ?c) }",
    ),
    (
        "trait and role",
        "SELECT ?scene ?p WHERE { ?scene :hasPerformer ?p . ?p :hasRole :isRole%202 . \
         ?p :hasTrait :isTrait%202 }",
    ),
    (
        "time comparison",
        "SELECT ?scene ?x WHERE { ?scene :hasPerformer performer:7 . \
         ?x sem:inMedia ?scene ; sem:atTime ?t FILTER (?t > 45) }",
    ),
    (
        "recursive role",
        "SELECT ?scene WHERE { ?scene :hasPerformer ?p . ?p :hasRole recursive:isRole%200 }",
    ),
    (
        "recursive relation",
        "SELECT ?scene WHERE { ?r sem:ofRelation recursive:isRelation%200 ; sem:inMedia ?scene }",
    ),
    (
        "every scene, first 1 MiB page",
        "SELECT ?s WHERE { ?s a sem:Scene }",
    ),
];

fn fail(message: &str) -> ExitCode {
    eprintln!("query-bench: {message}");
    ExitCode::from(2)
}

fn tag_named(lib: &stash_adapter::reader::LibraryRead, name: &str) -> String {
    lib.tags
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("synth has no tag {name}"))
        .id
        .clone()
}

/// The bench library: `synth`, with a Role and a Relation hierarchy and relations added.
fn bench_library(scenes: usize) -> stash_adapter::reader::LibraryRead {
    let mut lib = synth::library(scenes, scenes * 13 / 10);
    let (role0, role1) = (tag_named(&lib, "isRole 0"), tag_named(&lib, "isRole 1"));
    let (rel0, rel1) = (
        tag_named(&lib, "isRelation 0"),
        tag_named(&lib, "isRelation 1"),
    );
    for t in &mut lib.tags {
        if t.id == role1 {
            t.parents = vec![role0.clone()];
        }
        if t.id == rel1 {
            t.parents = vec![rel0.clone()];
        }
    }
    debug_assert!(lib
        .tags
        .iter()
        .any(|t| t.flags.contains(&TagFlag::Relation)));
    for (i, s) in lib
        .scenes
        .iter_mut()
        .enumerate()
        .filter(|(i, _)| i % 10 == 0)
    {
        let relation = if i % 20 == 0 { &rel0 } else { &rel1 };
        let (a, b) = (s.performers[0].clone(), s.performers[1].clone());
        s.fields.insert(
            "relationAssignments".into(),
            Field::Value(json!([{ "relationTagId": relation, "a": a, "b": b }])),
        );
    }
    lib
}

fn build(dir: &Path, scenes: usize) -> Result<Value, String> {
    let lib = bench_library(scenes);
    let store_dir = dir.join("graph");
    let _ = std::fs::remove_dir_all(&store_dir);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let store = GraphStore::new(&store_dir, SERVER_ID);
    let started = Instant::now();
    let report = build_run(
        &stash_adapter::reader::MemoryReader::new(lib),
        &store,
        SERVER_ID,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, m| eprintln!("{m}"),
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({ "scenes": scenes, "seconds": started.elapsed().as_secs_f64(), "report": report }))
}

/// The plugin binary beside this one (both are built by `cargo build --release -p plugin`).
fn plugin_binary() -> Result<PathBuf, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin = me.with_file_name(format!("semantic-engine{}", std::env::consts::EXE_SUFFIX));
    if bin.is_file() {
        Ok(bin)
    } else {
        Err(format!(
            "{} not found; build the plugin first",
            bin.display()
        ))
    }
}

/// One plugin call, as Stash makes it: the input on stdin, the output on stdout.
fn call(bin: &Path, dir: &Path, query: &str) -> Result<(Duration, Value), String> {
    let input = json!({
        "server_connection": {
            "Scheme": "http", "Host": "127.0.0.1", "Port": 9,
            "SessionCookie": {"Name": "session", "Value": ""},
            "Dir": dir, "PluginDir": dir
        },
        "args": { "op": "query", "query": query }
    })
    .to_string();
    let started = Instant::now();
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(input.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let took = started.elapsed();
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let env = v["output"].clone();
    if env["ok"] != true {
        return Err(format!(
            "{} after {:.1} s: {}",
            env["error"]["code"],
            took.as_secs_f64(),
            env["error"]["message"]
        ));
    }
    Ok((took, env))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Median, p95 and maximum, in milliseconds.
fn stats(mut times: Vec<f64>) -> Value {
    times.sort_by(f64::total_cmp);
    let at = |q: f64| times[((times.len() as f64 * q).ceil() as usize).clamp(1, times.len()) - 1];
    json!({ "runs": times.len(), "medianMs": at(0.5), "p95Ms": at(0.95), "maxMs": times[times.len() - 1] })
}

fn run(dir: &Path, runs: usize) -> Result<Value, String> {
    let bin = plugin_binary()?;
    let mut all = Vec::new();
    let mut rows = Vec::new();
    for (name, query) in QUERIES {
        // The first call warms the file cache, as a store in use is.
        let first = match call(&bin, dir, query) {
            Ok((_, env)) => env,
            Err(e) => {
                eprintln!("{name:32} failed: {e}");
                rows.push(json!({ "name": name, "query": query, "error": e }));
                continue;
            }
        };
        let mut times = Vec::new();
        for _ in 0..runs {
            times.push(ms(call(&bin, dir, query)?.0));
        }
        all.extend(times.iter().copied());
        let r = &first["result"];
        let bytes = r["bindings"].to_string().len();
        let s = stats(times);
        eprintln!(
            "{name:32} median {:7.1} ms  p95 {:7.1} ms  max {:7.1} ms  {} bindings, {bytes} bytes{}",
            s["medianMs"].as_f64().unwrap(),
            s["p95Ms"].as_f64().unwrap(),
            s["maxMs"].as_f64().unwrap(),
            r["page"]["count"],
            if r["page"]["continuation"].is_null() { "" } else { ", more pages" },
        );
        rows.push(json!({
            "name": name, "query": query, "time": s,
            "bindings": r["page"]["count"], "pageBytes": bytes,
            "morePages": !r["page"]["continuation"].is_null(),
            "engineSeconds": r["seconds"],
        }));
    }
    let overall = if all.is_empty() {
        Value::Null
    } else {
        stats(all)
    };
    Ok(json!({ "queries": rows, "overall": overall }))
}

fn now() -> String {
    stash_adapter::reader::time::rfc3339_utc(std::time::SystemTime::now())
}

fn write(out: &Path, v: &Value) -> Result<(), String> {
    if let Some(p) = out.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        out,
        serde_json::to_string_pretty(v).expect("serializes") + "\n",
    )
    .map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else {
        return fail("usage: query-bench build|run|fresh [options]; see the source header");
    };
    let (mut dir, mut scenes, mut runs, mut out) = (None, 0usize, 50usize, None);
    let mut i = 1;
    while i < args.len() {
        let value = args.get(i + 1).cloned();
        match args[i].as_str() {
            "--dir" => dir = value.map(PathBuf::from),
            "--scenes" => scenes = value.and_then(|v| v.parse().ok()).unwrap_or(0),
            "--runs" => runs = value.and_then(|v| v.parse().ok()).unwrap_or(0),
            "--out" => out = value.map(PathBuf::from),
            other => return fail(&format!("unknown argument {other}")),
        }
        i += 2;
    }
    let result = match cmd.as_str() {
        "build" => match (dir, scenes) {
            (Some(d), n) if n > 0 => build(&d, n).map(|v| (v, None)),
            _ => return fail("build needs --scenes N and --dir DIR"),
        },
        "run" => match dir {
            Some(d) if runs > 0 => {
                run(&d, runs).map(|v| (v, Some(out.unwrap_or(DEFAULT_OUT.into()))))
            }
            _ => return fail("run needs --dir DIR (and --runs N > 0)"),
        },
        "fresh" => fresh::fresh().map(|v| (v, Some(out.unwrap_or(FRESH_OUT.into())))),
        other => return fail(&format!("unknown command {other}")),
    };
    match result {
        Ok((mut v, out)) => {
            v["measuredAt"] = json!(now());
            v["command"] = json!(cmd);
            println!("{}", serde_json::to_string_pretty(&v).expect("serializes"));
            if let Some(out) = out {
                if let Err(e) = write(&out, &v) {
                    return fail(&format!("writing {}: {e}", out.display()));
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e),
    }
}

mod fresh {
    //! `query-bench fresh` (spec 009 T044; SC-007). It writes only the one scene it changes, and
    //! puts it back, on the server in `STASH_URL`, which must not be the owner's production
    //! server (localhost:9999).

    use serde_json::{json, Map, Value};
    use stash_adapter::relations::MediaKind;
    use stash_adapter::Client;
    use std::time::{Duration, Instant};

    const PLUGIN_ID: &str = "semantic-engine";
    const POLL: Duration = Duration::from_millis(500);
    const GIVE_UP: Duration = Duration::from_secs(120);
    const ROLES_KEY: &str = "roleTagAssignments";

    fn query(client: &Client, text: &str) -> Result<Vec<Value>, String> {
        let mut args = Map::new();
        args.insert("op".into(), json!("query"));
        args.insert("query".into(), json!(text));
        let out = client
            .run_plugin_operation(PLUGIN_ID, args)
            .map_err(|e| e.to_string())?;
        if out["ok"] != true {
            return Err(format!("query failed: {}", out["error"]));
        }
        Ok(out["result"]["bindings"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    fn id(binding: &Value, var: &str) -> Option<String> {
        binding[var]["id"].as_str().map(String::from)
    }

    /// The scene's Role assignments as stored: a JSON object, or JSON text of one.
    fn roles_of(stored: Option<&Value>) -> Map<String, Value> {
        match stored {
            Some(Value::String(t)) => serde_json::from_str(t).unwrap_or_default(),
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        }
    }

    fn write_roles(
        client: &Client,
        scene: &str,
        stored_as_text: bool,
        roles: &Map<String, Value>,
    ) -> Result<(), String> {
        let value = if stored_as_text {
            json!(Value::Object(roles.clone()).to_string())
        } else {
            Value::Object(roles.clone())
        };
        let mut partial = Map::new();
        partial.insert(ROLES_KEY.into(), value);
        client
            .update_item_custom_fields(MediaKind::Scene, scene, partial)
            .map_err(|e| e.to_string())
    }

    /// Polls until `scene`'s `performer` holds `role` (`want`) or no longer does; the seconds.
    fn wait_for(
        client: &Client,
        scene: &str,
        performer: &str,
        role: &str,
        want: bool,
    ) -> Result<f64, String> {
        let q = format!(
            "SELECT ?r WHERE {{ scene:{scene} :hasPerformer performer:{performer} . \
             performer:{performer} :hasRole ?r }}"
        );
        let started = Instant::now();
        loop {
            let held = query(client, &q)?
                .iter()
                .any(|b| id(b, "r").as_deref() == Some(role));
            if held == want {
                return Ok(started.elapsed().as_secs_f64());
            }
            if started.elapsed() > GIVE_UP {
                return Err(format!("the change did not show within {GIVE_UP:?}"));
            }
            std::thread::sleep(POLL);
        }
    }

    pub fn fresh() -> Result<Value, String> {
        let url = std::env::var("STASH_URL").map_err(|_| "set STASH_URL to the test instance")?;
        if url.contains(":9999") {
            return Err(
                "refusing localhost:9999: fresh writes, and only the test instance may be written"
                    .into(),
            );
        }
        let key = std::env::var("STASH_API_KEY").unwrap_or_default();
        let client = Client::with_api_key(&url, &key).map_err(|e| e.to_string())?;

        // A scene with a performer holding a Role, and a Role that performer does not hold there.
        let held = query(
            &client,
            "SELECT ?scene ?p ?r WHERE { ?scene a sem:Scene ; :hasPerformer ?p . ?p :hasRole ?r }",
        )?;
        let first = held.first().ok_or("no scene with a Role on this server")?;
        let (scene, performer) = (
            id(first, "scene").ok_or("no scene id")?,
            id(first, "p").ok_or("no performer id")?,
        );
        let holding: Vec<String> = held
            .iter()
            .filter(|b| {
                id(b, "scene") == Some(scene.clone()) && id(b, "p") == Some(performer.clone())
            })
            .filter_map(|b| id(b, "r"))
            .collect();
        let role = query(&client, "SELECT ?t WHERE { ?t a sem:Role }")?
            .iter()
            .filter_map(|b| id(b, "t"))
            .find(|t| !holding.contains(t))
            .ok_or("no Role tag left to add")?;

        let record = client
            .items_by_ids(MediaKind::Scene, std::slice::from_ref(&scene))
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .ok_or("the scene was not found")?;
        let stored = record.custom_fields.get(ROLES_KEY).cloned();
        let as_text = !matches!(stored, Some(Value::Object(_)));
        let original = roles_of(stored.as_ref());
        let mut changed = original.clone();
        let list = changed
            .entry(performer.clone())
            .or_insert_with(|| json!([]));
        if let Some(a) = list.as_array_mut() {
            a.push(json!(role));
        }

        eprintln!("scene {scene}: adding Role {role} to performer {performer}");
        let saved = Instant::now();
        write_roles(&client, &scene, as_text, &changed)?;
        let shown = wait_for(&client, &scene, &performer, &role, true);
        let save_to_answer = saved.elapsed().as_secs_f64();
        eprintln!("restoring scene {scene}");
        let restored = Instant::now();
        let restore = write_roles(&client, &scene, as_text, &original);
        let gone = restore
            .as_ref()
            .map_err(|e| e.clone())
            .and_then(|_| wait_for(&client, &scene, &performer, &role, false));
        let shown = shown?;
        restore?;
        Ok(json!({
            "server": url,
            "scene": scene,
            "performer": performer,
            "role": role,
            "secondsToShow": shown,
            "secondsFromSave": save_to_answer,
            "secondsToRestore": gone.map(|_| restored.elapsed().as_secs_f64()).ok(),
            "pollSeconds": POLL.as_secs_f64(),
            "target": "under 30 s (SC-007)",
        }))
    }
}
