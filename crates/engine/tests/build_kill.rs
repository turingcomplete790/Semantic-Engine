//! Killed builds never publish a partial graph (spec 007 T023; research R2, SC-005). Stash stops
//! a task by killing its process, so the test kills child builds at varied points, some while
//! they publish, and checks that the store always holds a whole, published generation.

mod common;

use common::build::SERVER;
use engine::build::{run, synth, BuildOptions};
use engine::graph::{GraphStore, Health};
use stash_adapter::reader::MemoryReader;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CHILD: &str = "SE_BUILD_KILL_CHILD";
const SCENES: usize = 900;
const IMAGES: usize = 300;
const KILLS: usize = 20;
const PUBLISH_KILLS: usize = 3;

fn build_into(dir: &Path, scenes: usize, images: usize, log: &mut dyn FnMut(&str)) -> u64 {
    let store = GraphStore::new(dir, SERVER);
    let reader = MemoryReader::new(synth::library(scenes, images));
    run(
        &reader,
        &store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, m| log(m),
    )
    .unwrap()
    .generation
}

/// The child: one build into the folder named by `SE_BUILD_KILL_CHILD`, logging to stderr.
#[test]
fn child_build() {
    let Ok(dir) = std::env::var(CHILD) else {
        return;
    };
    build_into(Path::new(&dir), SCENES, IMAGES, &mut |m| eprintln!("{m}"));
    eprintln!("CHILD DONE");
}

fn spawn(dir: &Path) -> (std::process::Child, mpsc::Receiver<String>) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["child_build", "--exact", "--nocapture", "--test-threads=1"])
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

/// The published generation, after checking it is whole: ready, and answering a count query
/// with as many facts as its generation file says.
fn published(dir: &Path) -> u64 {
    let store = GraphStore::new(dir, SERVER);
    let Health::Ready { generation } = store.check().unwrap() else {
        panic!("the store is not ready: {:?}", store.check());
    };
    let r = store.reader().unwrap();
    let rows = r
        .select("SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }")
        .unwrap()
        .rows;
    let n = match &rows[0][0] {
        Some(engine::graph::Term::Literal { value, .. }) => value.parse::<u64>().unwrap(),
        other => panic!("{other:?}"),
    };
    assert_eq!(n, r.generation().facts, "generation {generation} is whole");
    generation
}

#[test]
fn killed_builds_leave_a_whole_published_generation() {
    if std::env::var(CHILD).is_ok() {
        return;
    }
    let dir: PathBuf =
        std::env::temp_dir().join(format!("se-build-kill-{}/graph", std::process::id()));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let mut current = build_into(&dir, 20, 5, &mut |_| {});

    // One uninterrupted child build, to time it up to publishing, and in full.
    let t = Instant::now();
    let (mut child, rx) = spawn(&dir);
    let mut to_publish = None;
    let mut done = false;
    for l in rx.iter() {
        if l == "build: publishing" {
            to_publish = Some(t.elapsed());
        }
        done |= l == "CHILD DONE";
    }
    assert!(child.wait().unwrap().success());
    let full = t.elapsed();
    assert!(done);
    let to_publish = to_publish.expect("the build logs its publish");
    current = {
        let g = published(&dir);
        assert_eq!(g, current + 1);
        g
    };

    let mut before_publish = 0;
    let mut advanced = 0;
    for k in 0..KILLS {
        let (mut child, rx) = spawn(&dir);
        let at_publish = k < PUBLISH_KILLS;
        let deadline = if at_publish {
            full * 2
        } else {
            // Spread from 5% to 120% of the time to publishing: publishing writes the query
            // index too, so it is a large share of the build.
            to_publish
                .mul_f64(0.05 + 1.15 * (k - PUBLISH_KILLS) as f64 / (KILLS - PUBLISH_KILLS) as f64)
        };
        let start = Instant::now();
        let mut saw_publish = false;
        while start.elapsed() < deadline {
            match rx.recv_timeout(Duration::from_millis(1)) {
                Ok(l) if l == "build: publishing" => {
                    saw_publish = true;
                    if at_publish {
                        break;
                    }
                }
                _ => {}
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        saw_publish |= rx.try_iter().any(|l| l == "build: publishing");
        let g = published(&dir);
        if saw_publish {
            assert!(
                g == current || g == current + 1,
                "kill {k}: {g} after {current}"
            );
            advanced += usize::from(g == current + 1);
        } else {
            before_publish += 1;
            assert_eq!(g, current, "kill {k} before publishing changed the graph");
        }
        current = g;
    }
    eprintln!(
        "full build {full:?}, publishing at {to_publish:?}; {before_publish} kills before publishing, {} while or after \
         publishing ({advanced} of those completed the publish)",
        KILLS - before_publish
    );
    assert!(
        before_publish >= KILLS / 2,
        "only {before_publish} kills landed before publishing"
    );

    // The next uninterrupted build succeeds.
    let g = build_into(&dir, 20, 5, &mut |_| {});
    assert_eq!(g, current + 1);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
