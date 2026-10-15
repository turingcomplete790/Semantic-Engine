#![allow(dead_code)]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const FILES: [&str; 4] = [
    "core.ttl",
    "labels.ttl",
    "timeline.ttl",
    "stash-mapping.ttl",
];

pub fn repo_vocab_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vocabulary")
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Copies the repository vocabulary (ttl files, CHANGELOG.md, TERMS.md, released/) to a fresh
/// temporary directory and returns it, so a test can damage one file.
pub fn scratch_copy() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("sv-test-{}-{}", std::process::id(), n));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("released")).unwrap();
    let src = repo_vocab_dir();
    for f in FILES.iter().chain(["CHANGELOG.md", "TERMS.md"].iter()) {
        if src.join(f).exists() {
            fs::copy(src.join(f), dir.join(f)).unwrap();
        }
    }
    if let Ok(rd) = fs::read_dir(src.join("released")) {
        for e in rd.flatten() {
            fs::copy(e.path(), dir.join("released").join(e.file_name())).unwrap();
        }
    }
    dir
}

pub fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).unwrap()
}

pub fn write(dir: &Path, name: &str, text: &str) {
    fs::write(dir.join(name), text).unwrap();
}

pub fn rules(violations: &[semantic_vocabulary::Violation]) -> Vec<String> {
    let mut r: Vec<String> = violations.iter().map(|v| v.rule.clone()).collect();
    r.sort();
    r.dedup();
    r
}
