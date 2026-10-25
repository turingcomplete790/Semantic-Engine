//! Readers in other processes never see a partial change while a writer publishes (spec 006
//! T014; SC-001; research R9). The spike's live-store option failed 72% of such reads.

mod common;

use common::graph::{cleared, fact, lit, SERVER};
use engine::graph::{Fact, GraphStore};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const READER: &str = "SE_GRAPH_READER_DIR";
const BLOCK: usize = 200;
const GENERATIONS: u64 = 50;

fn block(n: u64) -> Vec<Fact> {
    (0..BLOCK)
        .map(|i| fact(&format!("block/{i}"), "gen", lit(&n.to_string())))
        .collect()
}

/// Run as a child process: read in a loop until the writer is done; print reads and mismatches.
#[test]
fn child_reader_loop() {
    let Ok(dir) = std::env::var(READER) else {
        return;
    };
    let store = GraphStore::new(&dir, SERVER);
    let done = std::path::Path::new(&dir).join("writer.done");
    let (mut reads, mut bad, mut errors) = (0u64, 0u64, 0u64);
    let deadline = Instant::now() + Duration::from_secs(120);
    while !done.exists() && Instant::now() < deadline {
        match store.reader() {
            Ok(r) => {
                let n = r.generation().number.to_string();
                match r.select("SELECT ?v WHERE { ?b <urn:t:gen> ?v }") {
                    Ok(s) => {
                        reads += 1;
                        let all_match = s.rows.len() == BLOCK
                            && s.rows.iter().all(|row| match &row[0] {
                                Some(engine::graph::Term::Literal { value, .. }) => *value == n,
                                _ => false,
                            });
                        if !all_match {
                            bad += 1;
                        }
                    }
                    Err(e) => {
                        errors += 1;
                        eprintln!("query error: {e}");
                    }
                }
            }
            Err(e) => {
                errors += 1;
                eprintln!("open error: {e}");
            }
        }
    }
    println!("READS {reads} BAD {bad} ERRORS {errors}");
}

#[test]
fn readers_never_see_a_partial_generation() {
    let (dir, store) = cleared("concurrency");
    let mut w = store.writer().unwrap();
    w.add(&block(1)).unwrap();
    w.publish().unwrap();

    let children: Vec<_> = (0..4)
        .map(|_| {
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "child_reader_loop",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(READER, &dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();

    for n in 2..=GENERATIONS {
        let mut w = store.writer().unwrap();
        w.remove(&block(n - 1)).unwrap();
        w.add(&block(n)).unwrap();
        assert_eq!(w.publish().unwrap().number, n);
        std::thread::sleep(Duration::from_millis(40));
    }
    std::thread::sleep(Duration::from_millis(200));
    std::fs::write(dir.join("writer.done"), b"").unwrap();

    let (mut reads, mut bad, mut errors) = (0u64, 0u64, 0u64);
    for c in children {
        let out = c.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        // The harness prints the result after the test's name, on the same line.
        let line = text
            .lines()
            .find_map(|l| l.find("READS ").map(|i| &l[i..]))
            .unwrap_or_else(|| {
                panic!(
                    "no result: status {:?}\nstdout: {text}\nstderr: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                )
            });
        let n: Vec<u64> = line
            .split_whitespace()
            .filter_map(|w| w.parse().ok())
            .collect();
        reads += n[0];
        bad += n[1];
        errors += n[2];
    }
    assert!(reads >= 5_000, "only {reads} reads");
    assert_eq!((bad, errors), (0, 0), "{reads} reads");
}
