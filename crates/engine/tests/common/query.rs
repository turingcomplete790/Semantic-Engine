//! Helpers for the query tests (spec 009 T004): stores built from fixtures, answers as sorted
//! N-Triples maps, and error checks.

use super::build::{fixture_library, SERVER};
use engine::build::{run, BuildOptions};
use engine::graph::{GraphStore, Term};
use engine::query::{self, ErrorKind, Page, PageRequest, QueryError, Value};
use stash_adapter::reader::{LibraryRead, MemoryReader};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn query_fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/query")
        .join(name)
}

/// A fresh store holding a full build of `library`.
pub fn store_of(library: LibraryRead, name: &str) -> GraphStore {
    let dir = std::env::temp_dir().join(format!("se-query-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = GraphStore::new(dir.join("graph"), SERVER);
    run(
        &MemoryReader::new(library),
        &store,
        SERVER,
        &BuildOptions::default(),
        &mut |_| {},
        &mut |_, _| {},
    )
    .unwrap();
    store
}

/// A fresh store holding the compatibility fixture (spec 007).
pub fn fixture_store(name: &str) -> GraphStore {
    store_of(fixture_library(), name)
}

/// A value in N-Triples form, as the expected answers write it.
pub fn ntriples(v: &Value) -> String {
    match v {
        Value::Item { iri, .. } | Value::Node { iri } | Value::Term { iri, .. } => {
            format!("<{iri}>")
        }
        Value::Literal {
            value,
            datatype,
            language,
        } => {
            let t = match language {
                Some(l) => Term::Literal {
                    value: value.clone(),
                    datatype: None,
                    language: Some(l.clone()),
                },
                None if datatype == "http://www.w3.org/2001/XMLSchema#string" => {
                    Term::string(value)
                }
                None => Term::typed(value, datatype),
            };
            t.to_ntriples()
        }
    }
}

pub fn run_query(store: &GraphStore, text: &str) -> Result<Page, QueryError> {
    query::run(store, text, &PageRequest::default())
}

/// The answer's variables, and its bindings as sorted maps from variable to N-Triples value.
pub fn answer(store: &GraphStore, text: &str) -> (Vec<String>, Vec<BTreeMap<String, String>>) {
    let page = run_query(store, text).unwrap_or_else(|e| panic!("{e}\nquery:\n{text}"));
    let mut rows: Vec<BTreeMap<String, String>> = page
        .bindings
        .iter()
        .map(|r| {
            page.variables
                .iter()
                .cloned()
                .zip(r.iter().map(ntriples))
                .collect()
        })
        .collect();
    rows.sort();
    (page.variables, rows)
}

/// Asserts that `text` fails with `code` at `line:column` (when given), and returns the error.
pub fn expect_error(
    store: &GraphStore,
    text: &str,
    code: &str,
    at: Option<(u32, u32)>,
) -> QueryError {
    let e = match run_query(store, text) {
        Ok(p) => panic!(
            "expected {code}, got {} bindings\nquery:\n{text}",
            p.bindings.len()
        ),
        Err(e) => e,
    };
    assert_eq!(e.code().as_str(), code, "{e}\nquery:\n{text}");
    if let Some((line, column)) = at {
        let pos = e.pos.unwrap_or_else(|| panic!("no position: {e}"));
        assert_eq!(
            (pos.line, pos.column),
            (line, column),
            "{e}\nquery:\n{text}"
        );
    }
    e
}

pub fn is_unsupported(e: &QueryError, construct: &str) -> bool {
    matches!(&e.kind, ErrorKind::Unsupported { construct: c } if c == construct)
}

/// `<urn:semantic-stash:{SERVER}:{rest}>`.
pub fn item(rest: &str) -> String {
    format!("<urn:semantic-stash:{SERVER}:{rest}>")
}

/// The known queries of the compatibility fixture (`fixtures/query/known.json`).
pub fn known_queries() -> Vec<String> {
    let known: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(query_fixture_path("known.json")).unwrap())
            .unwrap();
    known
        .iter()
        .map(|k| k["query"].as_str().unwrap().to_string())
        .collect()
}

/// Each query's bindings as sorted N-Triples rows, or its error code.
pub fn answer_sets(store: &GraphStore, queries: &[String]) -> Vec<Result<Vec<String>, String>> {
    queries
        .iter()
        .map(|q| {
            run_query(store, q)
                .map(|p| {
                    let mut rows: Vec<String> = p
                        .bindings
                        .iter()
                        .map(|r| r.iter().map(ntriples).collect::<Vec<_>>().join(" "))
                        .collect();
                    rows.sort();
                    rows
                })
                .map_err(|e| e.code().as_str().to_string())
        })
        .collect()
}

/// Asserts that the current generation has a query index written at publish, and that it
/// answers `queries` exactly as the store does without it (spec 009 research R12).
pub fn index_agrees(store: &GraphStore, queries: &[String], what: &str) {
    let n = store.reader().unwrap().generation().number;
    let path = engine::qindex::path(store.dir(), n);
    assert!(path.is_file(), "{what}: generation {n} has no query index");
    let with = answer_sets(store, queries);
    let aside = path.with_extension("aside");
    std::fs::rename(&path, &aside).unwrap();
    let without = answer_sets(store, queries);
    std::fs::rename(&aside, &path).unwrap();
    for ((q, a), b) in queries.iter().zip(&with).zip(&without) {
        assert_eq!(a, b, "{what}: the index and the store differ on\n{q}");
    }
}
