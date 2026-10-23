//! Opt-in live checks against the test server (spec 004 T015, T031, T040).
//!
//! Run with `cargo test -p plugin --test live_relations -- --ignored`. Each check needs:
//!   STASH_TEST_URL           the test server, http://localhost:9998 (writes go nowhere else)
//!   STASH_TEST_API_KEY       its API key (never printed)
//!   STASH_TEST_PLUGINS_DIR   its plugins folder; this branch's plugin is installed there first
//!
//! Every check creates its own tags with a unique `SE-004` prefix and deletes them afterwards,
//! even when an assertion fails.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

const TEST_URL: &str = "http://localhost:9998";

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

struct Live {
    url: String,
    key: String,
    /// One client for every request, so timings measure Stash, not connection set-up.
    http: reqwest::blocking::Client,
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

static INSTALL: Once = Once::new();

/// The live connection, with this branch's plugin installed and plugins reloaded once per run.
fn live() -> Option<Live> {
    let (Some(url), Some(key), Some(plugins)) = (
        env("STASH_TEST_URL"),
        env("STASH_TEST_API_KEY"),
        env("STASH_TEST_PLUGINS_DIR"),
    ) else {
        eprintln!(
            "notice: STASH_TEST_URL, STASH_TEST_API_KEY, STASH_TEST_PLUGINS_DIR not set; skipped"
        );
        return None;
    };
    let url = url.trim_end_matches('/').to_string();
    assert_eq!(url, TEST_URL, "live checks write only to the test server");
    stash_adapter::install_tls_provider();
    let l = Live {
        url,
        key,
        http: reqwest::blocking::Client::new(),
    };
    INSTALL.call_once(|| {
        let out = Command::new("sh")
            .arg("scripts/package.sh")
            .current_dir(repo())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let package = repo().join(
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .last()
                .unwrap()
                .trim(),
        );
        let out = Command::new("sh")
            .arg(repo().join("scripts/install.sh"))
            .arg("--from-file")
            .arg(&package)
            .args(["--plugins-dir", &plugins, "--yes"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "install: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        l.gql("mutation { reloadPlugins }", json!({}));
    });
    Some(l)
}

impl Live {
    fn gql(&self, query: &str, variables: Value) -> Value {
        let v: Value = self
            .http
            .post(format!("{}/graphql", self.url))
            .header("ApiKey", &self.key)
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .expect("Stash reachable")
            .json()
            .expect("JSON response");
        assert!(v.get("errors").is_none(), "GraphQL errors: {v}");
        v["data"].clone()
    }

    fn op(&self, args: Value) -> Value {
        let out = self.gql(
            "mutation($a: Map) { runPluginOperation(plugin_id: \"semantic-engine\", args: $a) }",
            json!({ "a": args }),
        );
        out["runPluginOperation"].clone()
    }

    fn create_tag(&self, name: &str, parents: &[&str], custom_fields: Value) -> String {
        let out = self.gql(
            "mutation($i: TagCreateInput!) { tagCreate(input: $i) { id } }",
            json!({ "i": { "name": name, "parent_ids": parents, "custom_fields": custom_fields } }),
        );
        out["tagCreate"]["id"].as_str().unwrap().to_string()
    }
}

/// Deletes the tags it holds when dropped, so a failed assertion still cleans up.
struct Tags<'a> {
    live: &'a Live,
    ids: Vec<String>,
}

impl Drop for Tags<'_> {
    fn drop(&mut self) {
        if !self.ids.is_empty() {
            self.live.gql(
                "mutation($ids: [ID!]!) { tagsDestroy(ids: $ids) }",
                json!({ "ids": self.ids }),
            );
        }
    }
}

fn unique(name: &str) -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("SE-004 {name} {n}")
}

fn ids(list: &Value) -> Vec<String> {
    let mut v: Vec<String> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// Quickstart step 1: Example 1 defined in Stash is reported with its hierarchy (T015).
#[test]
#[ignore = "live: needs STASH_TEST_* and writes test tags to the test server"]
fn relations_definitions() {
    let Some(l) = live() else { return };
    let mut tags = Tags {
        live: &l,
        ids: Vec::new(),
    };
    let role = json!({ "isRole": 1 });
    let mut add = |name: &str, parents: &[&str], cf: Value| {
        let id = l.create_tag(&unique(name), parents, cf);
        tags.ids.push(id.clone());
        id
    };
    let customer = add("Customer", &[], role.clone());
    let patient = add("Patient", &[&customer], role.clone());
    let professional = add("Professional", &[], role.clone());
    let nurse = add("Nurse", &[&professional], role.clone());
    let doctor = add("Doctor", &[&professional], role);
    let ends = |a: &str, b: &str| json!({ "a": a, "b": b }).to_string();
    let full = add(
        "Full Service",
        &[],
        json!({ "isRelation": 1, "relationEnds": ends(&customer, &professional) }),
    );
    let extra = add(
        "Extra Care",
        &[&full],
        json!({ "isRelation": 1, "relationEnds": ends(&patient, &nurse) }),
    );

    let env = l.op(json!({ "op": "relations", "tag": full, "recursive": true }));
    assert_eq!(env["ok"], true, "{env}");
    let r = &env["result"];
    assert_eq!(
        ids(&r["relations"]),
        sorted(vec![full.clone(), extra.clone()])
    );
    let ec = r["relations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == extra.as_str())
        .unwrap();
    assert_eq!(ec["broader"], json!([full]));
    assert_eq!(ec["valid"], true);
    assert_eq!(ec["ends"], json!({ "a": patient, "b": nurse }));

    let env = l.op(json!({ "op": "relations", "tag": professional }));
    assert_eq!(
        ids(&env["result"]["roles"]),
        sorted(vec![professional.clone(), nurse, doctor])
    );
    let env = l.op(json!({ "op": "relations", "tag": patient }));
    assert_eq!(
        ids(&env["result"]["roles"]),
        sorted(vec![customer, patient])
    );
}

// ---- Auto-apply on save (T031). Run with `-- --ignored --test-threads=1 --nocapture`.

use std::time::{Duration, Instant};

/// Example 2 built on the test server from scratch: tags, two performers, and the scenes a check
/// creates. Everything is deleted on drop.
struct Example2<'a> {
    live: &'a Live,
    tags: Vec<String>,
    performers: Vec<String>,
    scenes: Vec<String>,
    kissing: String,
    smaller: String,
    larger: String,
    relation: String,
    p1: String,
    p2: String,
}

impl Drop for Example2<'_> {
    fn drop(&mut self) {
        let l = self.live;
        for chunk in self.scenes.chunks(200) {
            l.gql(
                "mutation($i: ScenesDestroyInput!) { scenesDestroy(input: $i) }",
                json!({ "i": { "ids": chunk } }),
            );
        }
        if !self.performers.is_empty() {
            l.gql(
                "mutation($ids: [ID!]!) { performersDestroy(ids: $ids) }",
                json!({ "ids": self.performers }),
            );
        }
        if !self.tags.is_empty() {
            l.gql(
                "mutation($ids: [ID!]!) { tagsDestroy(ids: $ids) }",
                json!({ "ids": self.tags }),
            );
        }
    }
}

impl<'a> Example2<'a> {
    fn new(live: &'a Live) -> Example2<'a> {
        let mut tags = Vec::new();
        let mut tag = |name: &str, cf: Value| {
            let id = live.create_tag(&unique(name), &[], cf);
            tags.push(id.clone());
            id
        };
        let smaller = tag("Smaller", json!({ "isRole": 1 }));
        let larger = tag("Larger", json!({ "isRole": 1 }));
        let short = tag("Short", json!({ "isLabel": 1 }));
        let tall = tag("Tall", json!({ "isLabel": 1 }));
        let kissing = tag("Kissing", json!({ "isAction": 1 }));
        let rules = json!([{ "id": "r1", "a": { "label": short }, "b": { "label": tall } }]);
        let relation = tag(
            "Size Difference",
            json!({
                "isRelation": 1,
                "relationEnds": json!({ "a": smaller, "b": larger }).to_string(),
                "relationRules": rules.to_string(),
            }),
        );
        let performer = |name: &str, label: &str| {
            live.gql(
                "mutation($i: PerformerCreateInput!) { performerCreate(input: $i) { id } }",
                json!({ "i": { "name": unique(name), "tag_ids": [label] } }),
            )["performerCreate"]["id"]
                .as_str()
                .unwrap()
                .to_string()
        };
        let p1 = performer("P1 Short", &short);
        let p2 = performer("P2 Tall", &tall);
        Example2 {
            live,
            tags,
            performers: vec![p1.clone(), p2.clone()],
            scenes: Vec::new(),
            kissing,
            smaller,
            larger,
            relation,
            p1,
            p2,
        }
    }

    /// Custom fields that pair P1 and P2 on Kissing (or not, with `paired: false`).
    fn pairing_fields(&self, paired: bool) -> Value {
        let k = &self.kissing;
        let actions = json!({
            &self.p1: [{ "actionTagId": k, "variations": [] }],
            &self.p2: [{ "actionTagId": k, "variations": [] }],
        });
        let pairings = if paired {
            json!([{ "a": { "performer": self.p1, "actionTagId": k },
                     "b": { "performer": self.p2, "actionTagId": k },
                     "direction": "bidirectional", "pairingId": "se-004-live" }])
        } else {
            json!([])
        };
        json!({ "actionAssignments": actions.to_string(), "actionPairings": pairings.to_string() })
    }

    fn create_scene(&mut self, paired: bool) -> String {
        let id = self.live.gql(
            "mutation($i: SceneCreateInput!) { sceneCreate(input: $i) { id } }",
            json!({ "i": {
                "title": unique("scene"),
                "performer_ids": [self.p1, self.p2],
                "custom_fields": self.pairing_fields(paired),
            } }),
        )["sceneCreate"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        self.scenes.push(id.clone());
        id
    }

    /// The scene's custom field `key`, parsed from its JSON text (null when absent).
    fn field(&self, scene: &str, key: &str) -> Value {
        let d = self.live.gql(
            "query($id: ID) { findScene(id: $id) { custom_fields } }",
            json!({ "id": scene }),
        );
        match &d["findScene"]["custom_fields"][key] {
            Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
            other => other.clone(),
        }
    }

    fn expected_relation(&self) -> Value {
        json!([{ "relationTagId": self.relation, "a": self.p1, "b": self.p2, "auto": true, "ruleId": "r1" }])
    }
}

/// Polls `check` every 250 ms until it holds or `limit` passes; returns the time it took.
fn wait_until(limit: Duration, mut check: impl FnMut() -> bool) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if check() {
            return Some(start.elapsed());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    None
}

fn plugin_folder() -> PathBuf {
    PathBuf::from(env("STASH_TEST_PLUGINS_DIR").unwrap()).join("semantic-engine")
}

/// Waits until no "Apply relations" job is pending or running.
fn wait_idle() {
    let marker = plugin_folder().join("apply.pending");
    assert!(
        wait_until(Duration::from_secs(120), || !marker.exists()).is_some(),
        "the Apply relations job did not finish"
    );
    std::thread::sleep(Duration::from_millis(500));
}

/// Quickstart step 2: Example 2 is applied after a save, with no client call (SC-002), and
/// removed when the pairing goes.
#[test]
#[ignore = "live: needs STASH_TEST_* and writes test data to the test server"]
fn auto_apply_example2() {
    let Some(l) = live() else { return };
    let mut ex = Example2::new(&l);
    let scene = ex.create_scene(true);
    let took = wait_until(Duration::from_secs(30), || {
        ex.field(&scene, "relationAssignments") == ex.expected_relation()
    });
    eprintln!("applied after {took:?}");
    assert!(
        took.is_some(),
        "not applied within 30 s: {}",
        ex.field(&scene, "relationAssignments")
    );
    assert_eq!(
        ex.field(&scene, "roleTagAssignments"),
        json!({ &ex.p1: [ex.smaller], &ex.p2: [ex.larger] })
    );
    assert_eq!(
        ex.field(&scene, "autoRoleAssignments"),
        json!({ &ex.p1: [ex.smaller], &ex.p2: [ex.larger] })
    );

    l.gql(
        "mutation($i: SceneUpdateInput!) { sceneUpdate(input: $i) { id } }",
        json!({ "i": { "id": scene, "custom_fields": { "partial": ex.pairing_fields(false) } } }),
    );
    let took = wait_until(Duration::from_secs(30), || {
        ex.field(&scene, "relationAssignments") == json!([])
    });
    eprintln!("removed after {took:?}");
    assert!(took.is_some(), "not removed within 30 s");
    assert_eq!(ex.field(&scene, "roleTagAssignments"), json!({}));
    wait_idle();
}

/// SC-006: unchanged saves after the relation is applied cause no further writes.
#[test]
#[ignore = "live: needs STASH_TEST_* and writes test data to the test server"]
fn idempotent_saves() {
    let Some(l) = live() else { return };
    let mut ex = Example2::new(&l);
    let scene = ex.create_scene(true);
    assert!(wait_until(Duration::from_secs(30), || {
        ex.field(&scene, "relationAssignments") == ex.expected_relation()
    })
    .is_some());
    wait_idle();
    let mut last = String::new();
    for _ in 0..10 {
        last = l.gql(
            "mutation($i: SceneUpdateInput!) { sceneUpdate(input: $i) { updated_at } }",
            json!({ "i": { "id": scene, "custom_fields": { "partial": ex.pairing_fields(true) } } }),
        )["sceneUpdate"]["updated_at"]
            .as_str()
            .unwrap()
            .to_string();
    }
    wait_idle();
    let now = l.gql(
        "query($id: ID) { findScene(id: $id) { updated_at } }",
        json!({ "id": scene }),
    )["findScene"]["updated_at"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(now, last, "the engine wrote to an up-to-date scene");
    assert_eq!(
        ex.field(&scene, "relationAssignments"),
        ex.expected_relation()
    );
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Re-enables the plugin on drop.
struct Enabled<'a>(&'a Live);
impl Drop for Enabled<'_> {
    fn drop(&mut self) {
        set_enabled(self.0, true);
    }
}

fn set_enabled(l: &Live, on: bool) {
    l.gql(
        "mutation($m: BoolMap!) { setPluginsEnabled(enabledMap: $m) }",
        json!({ "m": { "semantic-engine": on } }),
    );
}

/// SC-003: the plugin adds at most 7 ms per saved item, for a single save and for the first
/// save of a burst (which also starts the job).
#[test]
#[ignore = "live: needs STASH_TEST_* and writes test data to the test server"]
fn save_cost() {
    let Some(l) = live() else { return };
    let mut ex = Example2::new(&l);
    let scene = ex.create_scene(false);
    wait_idle();
    let save = |n: u32| {
        let t = Instant::now();
        l.gql(
            "mutation($i: SceneUpdateInput!) { sceneUpdate(input: $i) { id } }",
            json!({ "i": { "id": scene, "custom_fields": { "partial": { "seTouch": n } } } }),
        );
        t.elapsed().as_secs_f64() * 1000.0
    };
    // Plugin disabled: the baseline.
    let guard = Enabled(&l);
    set_enabled(&l, false);
    let off: Vec<f64> = (0..60).map(save).collect();
    set_enabled(&l, true);
    // Enabled, a job already pending: the hook only appends.
    std::fs::write(plugin_folder().join("apply.pending"), "").unwrap();
    let appending: Vec<f64> = (0..60).map(save).collect();
    std::fs::remove_file(plugin_folder().join("apply.pending")).ok();
    // Enabled, first save of a burst: the hook also starts the job.
    let mut first = Vec::new();
    for n in 0..20 {
        wait_idle();
        first.push(save(100 + n));
    }
    wait_idle();
    drop(guard);
    let (off, appending, first) = (median(off), median(appending), median(first));
    eprintln!(
        "save median: plugin off {off:.1} ms, hook appends {appending:.1} ms (+{:.1}), first of a burst {first:.1} ms (+{:.1})",
        appending - off,
        first - off
    );
    assert!(
        appending - off <= 7.0,
        "hook adds {:.1} ms",
        appending - off
    );
    assert!(
        first - off <= 7.0,
        "first save of a burst adds {:.1} ms",
        first - off
    );
}

/// SC-005: a 1,000-scene bulk edit is fully applied within 5 minutes.
#[test]
#[ignore = "live: needs STASH_TEST_* and writes test data to the test server"]
fn bulk_1000() {
    let Some(l) = live() else { return };
    let mut ex = Example2::new(&l);
    for _ in 0..1000 {
        ex.create_scene(false);
    }
    wait_idle();
    let start = Instant::now();
    l.gql(
        "mutation($i: BulkSceneUpdateInput!) { bulkSceneUpdate(input: $i) { id } }",
        json!({ "i": { "ids": ex.scenes, "custom_fields": { "partial": ex.pairing_fields(true) } } }),
    );
    let saved = start.elapsed();
    let ids = ex.scenes.clone();
    let done = wait_until(Duration::from_secs(300), || {
        let d = l.gql(
            "query($ids: [ID!]) { findScenes(ids: $ids, filter: { per_page: -1 }) { scenes { custom_fields } } }",
            json!({ "ids": ids }),
        );
        d["findScenes"]["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| {
                s["custom_fields"]["relationAssignments"]
                    .as_str()
                    .is_some_and(|t| t.contains("\"auto\":true"))
            })
    });
    eprintln!(
        "bulk save took {saved:?}; all 1000 applied after {:?}",
        done.map(|d| d + saved)
    );
    assert!(done.is_some(), "not all applied within 5 minutes");
    wait_idle();
}

/// SC-008 (live part): recommendations answer within 200 ms at p95 and never change native tags.
#[test]
#[ignore = "live: needs STASH_TEST_* and writes test data to the test server"]
fn recommendations_latency() {
    let Some(l) = live() else { return };
    let mut ex = Example2::new(&l);
    let scene = ex.create_scene(false);
    let native = [ex.kissing.clone(), ex.relation.clone()];
    l.gql(
        "mutation($i: SceneUpdateInput!) { sceneUpdate(input: $i) { id } }",
        json!({ "i": { "id": scene, "tag_ids": native } }),
    );
    wait_idle();
    let tags = |l: &Live| {
        let d = l.gql(
            "query($id: ID) { findScene(id: $id) { tags { id } } }",
            json!({ "id": scene }),
        );
        ids(&d["findScene"]["tags"])
    };
    let before = tags(&l);
    let mut times = Vec::new();
    let mut last = Value::Null;
    for _ in 0..100 {
        let t = Instant::now();
        last = l.op(json!({ "op": "recommendations", "media": "scene", "id": scene }));
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    assert_eq!(last["ok"], true, "{last}");
    assert_eq!(
        last["result"]["actions"],
        json!([{ "tag": ex.kissing, "from": ex.kissing }])
    );
    assert_eq!(
        last["result"]["relations"],
        json!([{ "tag": ex.relation, "from": ex.relation }])
    );
    times.sort_by(f64::total_cmp);
    let p95 = times[94];
    eprintln!(
        "recommendations: median {:.1} ms, p95 {p95:.1} ms",
        times[50]
    );
    assert!(p95 < 200.0, "p95 {p95:.1} ms");
    assert_eq!(tags(&l), before, "native tags must not change");
}

/// Quickstart step 4 (FR-011): editing a rule re-checks nothing by itself; "Re-apply relations"
/// brings the scene in line.
#[test]
#[ignore = "live: needs STASH_TEST_* and writes test data to the test server"]
fn reapply_after_a_rule_change() {
    let Some(l) = live() else { return };
    let mut ex = Example2::new(&l);
    let scene = ex.create_scene(true);
    assert!(wait_until(Duration::from_secs(30), || {
        ex.field(&scene, "relationAssignments") == ex.expected_relation()
    })
    .is_some());
    wait_idle();
    // Remove the rule: the stored automatic relation is now stale, but nothing re-checks it.
    l.gql(
        "mutation($i: TagUpdateInput!) { tagUpdate(input: $i) { id } }",
        json!({ "i": { "id": ex.relation, "custom_fields": { "partial": { "relationRules": "[]" } } } }),
    );
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(
        ex.field(&scene, "relationAssignments"),
        ex.expected_relation()
    );

    l.gql(
        "mutation($m: Map) { runPluginTask(plugin_id: \"semantic-engine\", task_name: \"Re-apply relations\", args_map: $m) }",
        json!({ "m": { "scope": "scenes", "ids": [scene] } }),
    );
    let took = wait_until(Duration::from_secs(30), || {
        ex.field(&scene, "relationAssignments") == json!([])
    });
    eprintln!("re-apply removed the stale relation after {took:?}");
    assert!(
        took.is_some(),
        "Re-apply relations did not remove the stale relation"
    );
    assert_eq!(ex.field(&scene, "autoRoleAssignments"), json!({}));
    wait_idle();
}
