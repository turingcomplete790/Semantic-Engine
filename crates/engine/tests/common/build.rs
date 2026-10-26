//! Shared helpers for the graph build tests (spec 007 T011).

use engine::build::Mapped;
use engine::graph::Fact;
use semantic_vocabulary::iri::ItemIris;
use stash_adapter::reader::LibraryRead;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The example server id the vocabulary documents use.
pub const SERVER: &str = "00000000-0000-4000-8000-000000000001";
const VOCAB: &str = "urn:semantic-stash:vocab:";

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/build")
        .join(name)
}

/// The captured test-instance library, read in place from the adapter's fixtures.
pub fn test_instance_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stash-adapter/tests/fixtures/test-instance/library-read.json")
}

pub fn load_library(path: &Path) -> LibraryRead {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub fn fixture_library() -> LibraryRead {
    load_library(&fixture_path("library.json"))
}

pub fn iris() -> ItemIris {
    ItemIris::new(SERVER).unwrap()
}

/// Sorted N-Triples lines, without the vocabulary's own facts.
pub fn lines(facts: &BTreeSet<Fact>) -> Vec<String> {
    let mut out: Vec<String> = facts
        .iter()
        .filter(|f| !matches!(&f.subject, engine::graph::Term::Iri(s) if s.starts_with(VOCAB)))
        .map(Fact::to_ntriples)
        .collect();
    out.sort();
    out
}

/// Expands one short-form token of `expected.nt` (fixture README § Short forms).
fn expand_token(t: &str) -> String {
    let item = format!("urn:semantic-stash:{SERVER}:");
    if t == "a" {
        "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>".into()
    } else if let Some(rest) = t.strip_prefix("rdfs:") {
        format!("<http://www.w3.org/2000/01/rdf-schema#{rest}>")
    } else if let Some(rest) = t.strip_prefix("sem:") {
        format!("<{VOCAB}{rest}>")
    } else if let Some(rest) = t.strip_prefix("i:") {
        format!("<{item}{rest}>")
    } else {
        panic!("unknown short form {t:?}")
    }
}

fn expand_object(o: &str) -> String {
    if o.starts_with('"') {
        match o.rfind("\"^^xsd:") {
            Some(at) => format!(
                "{}^^<http://www.w3.org/2001/XMLSchema#{}>",
                &o[..=at],
                &o[at + 7..]
            ),
            None => o.to_string(),
        }
    } else {
        expand_token(o)
    }
}

/// The expected lines of a short-form file, expanded and sorted.
pub fn expected_lines(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let body = line
            .strip_suffix(" .")
            .unwrap_or_else(|| panic!("line {}: no final \" .\"", n + 1));
        let mut parts = body.splitn(3, ' ');
        let (s, p, o) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts
                .next()
                .unwrap_or_else(|| panic!("line {}: no object", n + 1)),
        );
        out.push(format!(
            "{} {} {} .",
            expand_token(s),
            expand_token(p),
            expand_object(o)
        ));
    }
    out.sort();
    out
}

/// Expected lines missing from `actual`, and lines of `actual` not expected.
pub fn diff(expected: &[String], actual: &[String]) -> (Vec<String>, Vec<String>) {
    let e: BTreeSet<&String> = expected.iter().collect();
    let a: BTreeSet<&String> = actual.iter().collect();
    (
        e.difference(&a).map(|s| s.to_string()).collect(),
        a.difference(&e).map(|s| s.to_string()).collect(),
    )
}

/// Compares the mapped graph with an expected file. With `BUILD_FIXTURE_BLESS=1`, writes the
/// mapped graph next to it as `<name>.new` (full N-Triples) for review instead.
pub fn assert_graph(expected: &str, mapped: &Mapped) {
    let actual = lines(&mapped.facts);
    let path = fixture_path(expected);
    if std::env::var("BUILD_FIXTURE_BLESS").as_deref() == Ok("1") {
        let mut text = actual.join("\n");
        text.push('\n');
        std::fs::write(path.with_extension("nt.new"), text).unwrap();
        return;
    }
    let (missing, extra) = diff(&expected_lines(&path), &actual);
    assert!(
        missing.is_empty() && extra.is_empty(),
        "the graph differs from {expected}\n\nmissing ({}):\n{}\n\nextra ({}):\n{}\n",
        missing.len(),
        missing.join("\n"),
        extra.len(),
        extra.join("\n")
    );
}
