//! Killed updates never publish a partial graph, and the next update still reaches a full
//! rebuild's graph (spec 008 T018; US3, FR-013, SC-004).

mod common;

use common::build::SERVER;
use common::update::{all_facts, rebuilt_facts, Library};
use engine::build::update::update;
use engine::build::{run, synth, BuildOptions};
use engine::graph::{GraphStore, Health, Term};
use serde_json::json;
use stash_adapter::reader::MediaKind;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CHILD: &str = "SE_UPDATE_KILL_CHILD";
const KILLS: usize = 12;
const PUBLISH_KILLS: usize = 3;

/// The library both processes use: the same synthetic library, with 100 scenes changed.
fn changed() -> Library {
    let mut lib = Library::new(synth::library(300, 100));
    for i in 1..=100 {
        lib.edit_media_field(
            MediaKind::Scene,
            &i.to_string(),
            "pluggableAssignments",
            json!({}),
        );
    }
    lib
}

fn base() -> Library {
    Library::new(synth::library(300, 100))
}

/// The child: one update of the changed library, logging to stderr.
#[test]
fn child_update() {
    let Ok(dir) = std::env::var(CHILD) else {
        return;
    };
    let store = GraphStore::new(Path::new(&dir), SERVER);
    update(
        &changed().reader(),
        &store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, m| eprintln!("{m}"),
    )
    .unwrap();
    eprintln!("CHILD DONE");
}

fn spawn(dir: &Path) -> (std::process::Child, mpsc::Receiver<String>) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["child_update", "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (tx, rx) = mpsc::channel();
    let err = child.stderr.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    (child, rx)
}

/// The generation, after checking it is whole.
fn whole(store: &GraphStore) -> u64 {
    let Health::Ready { generation } = store.check().unwrap() else {
        panic!("not ready: {:?}", store.check());
    };
    let r = store.reader().unwrap();
    let rows = r
        .select("SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }")
        .unwrap()
        .rows;
    let Some(Term::Literal { value, .. }) = &rows[0][0] else {
        panic!()
    };
    assert_eq!(
        value.parse::<u64>().unwrap(),
        r.generation().facts,
        "generation {generation} is whole"
    );
    generation
}

/// Rebuilds the base graph, so every kill starts from the same point.
fn reset(store: &GraphStore) {
    run(
        &base().reader(),
        store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
}

#[test]
fn killed_updates_publish_nothing_partial() {
    if std::env::var(CHILD).is_ok() {
        return;
    }
    let dir: PathBuf =
        std::env::temp_dir().join(format!("se-update-kill-{}/graph", std::process::id()));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let store = GraphStore::new(&dir, SERVER);
    reset(&store);

    // One uninterrupted child update, to time it.
    let t = Instant::now();
    let (mut child, rx) = spawn(&dir);
    assert!(child.wait().unwrap().success());
    let full = t.elapsed();
    assert!(rx.iter().any(|l| l == "CHILD DONE"));

    let mut before_publish = 0;
    for k in 0..KILLS {
        reset(&store);
        let base_generation = whole(&store);
        let (mut child, rx) = spawn(&dir);
        let at_publish = k < PUBLISH_KILLS;
        let deadline = if at_publish {
            full * 2
        } else {
            full.mul_f64(0.05 + 0.9 * (k - PUBLISH_KILLS) as f64 / (KILLS - PUBLISH_KILLS) as f64)
        };
        let start = Instant::now();
        let mut saw_publish = false;
        while start.elapsed() < deadline {
            if let Ok(l) = rx.recv_timeout(Duration::from_millis(1)) {
                if l == "update: publishing" {
                    saw_publish = true;
                    if at_publish {
                        break;
                    }
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        saw_publish |= rx.try_iter().any(|l| l == "update: publishing");
        let g = whole(&store);
        if saw_publish {
            assert!(g == base_generation || g == base_generation + 1, "kill {k}");
        } else {
            before_publish += 1;
            assert_eq!(
                g, base_generation,
                "kill {k} before publishing changed the graph"
            );
        }
        // The next update, uninterrupted, reaches the full rebuild's graph.
        let lib = changed();
        update(
            &lib.reader(),
            &store,
            SERVER,
            &BuildOptions::default(),
            &mut |_| {},
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(
            all_facts(&store),
            rebuilt_facts(&lib, &format!("kill{k}")),
            "kill {k}"
        );
    }
    eprintln!("full update {full:?}; {before_publish} kills before publishing");
    assert!(
        before_publish >= KILLS / 2,
        "only {before_publish} kills before publishing"
    );
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
