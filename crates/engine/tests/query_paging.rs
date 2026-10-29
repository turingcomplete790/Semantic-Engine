//! Large answers come in pages (spec 009 T032; research R6; SC-005). One store of 30,000
//! scenes is shared by the tests that only read it; tests that publish or remove generations
//! build their own. The stores hold the vocabulary and one type fact per scene, not a full
//! `synth` build: paging reads only those, and a 30,000-scene build takes minutes in debug.

mod common;

use common::build::{load_library, SERVER};
use common::query::{query_fixture_path, store_of};
use engine::build::vocabulary_facts;
use engine::graph::{Fact, GraphStore, Term};
use engine::query::names::VOCAB;
use engine::query::{self, ErrorKind, Page, PageRequest, PAGE_BYTES};
use std::collections::BTreeSet;
use std::sync::OnceLock;

const SCENES: usize = 30_000;
const EVERY_SCENE: &str = "SELECT ?s WHERE { ?s a sem:Scene }";

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

fn big(name: &str) -> GraphStore {
    let dir = std::env::temp_dir().join(format!("se-query-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = GraphStore::new(dir.join("graph"), SERVER);
    store.clear_for_rebuild().unwrap();
    let mut facts = vocabulary_facts();
    facts.extend((1..=SCENES).map(|i| Fact {
        subject: Term::Iri(format!("urn:semantic-stash:{SERVER}:scene:{i}")),
        predicate: Term::Iri(RDF_TYPE.into()),
        object: Term::Iri(format!("{VOCAB}Scene")),
    }));
    let mut w = store.writer().unwrap();
    w.add(&facts).unwrap();
    w.publish().unwrap();
    store
}

fn shared() -> &'static GraphStore {
    static STORE: OnceLock<GraphStore> = OnceLock::new();
    STORE.get_or_init(|| big("paging-shared"))
}

fn page(store: &GraphStore, text: &str, req: &PageRequest) -> Page {
    query::run(store, text, req).unwrap_or_else(|e| panic!("{e}"))
}

fn next(req: &PageRequest, p: &Page) -> Option<PageRequest> {
    p.continuation.as_ref().map(|c| PageRequest {
        continuation: Some(c.clone()),
        ..req.clone()
    })
}

/// Every page of `text`, in order.
fn all_pages(store: &GraphStore, text: &str, first: PageRequest) -> Vec<Page> {
    let mut out = Vec::new();
    let mut req = Some(first);
    while let Some(r) = req {
        let p = page(store, text, &r);
        req = next(&r, &p);
        out.push(p);
        assert!(out.len() < 1_000, "paging does not end");
    }
    out
}

fn iris(pages: &[Page]) -> Vec<String> {
    pages
        .iter()
        .flat_map(|p| p.bindings.iter())
        .map(|row| match &row[0] {
            engine::query::Value::Item { iri, .. } => iri.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn scene_iris(store: &GraphStore) -> BTreeSet<String> {
    let r = store.reader().unwrap();
    r.select(&format!("SELECT ?s WHERE {{ ?s a <{VOCAB}Scene> }}"))
        .unwrap()
        .rows
        .into_iter()
        .map(|row| match &row[0] {
            Some(Term::Iri(i)) => i.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn expect_kind(store: &GraphStore, text: &str, req: &PageRequest) -> ErrorKind {
    match query::run(store, text, req) {
        Ok(p) => panic!("expected an error, got {} bindings", p.bindings.len()),
        Err(e) => e.kind,
    }
}

#[test]
fn every_scene_page_by_page_within_a_mebibyte_each_and_once() {
    let store = shared();
    let pages = all_pages(store, EVERY_SCENE, PageRequest::default());
    assert!(pages.len() >= 3, "{} pages", pages.len());
    let mut offset = 0;
    for p in &pages {
        let bytes = serde_json::to_string(&p.bindings_json()).unwrap().len();
        assert!(bytes <= PAGE_BYTES, "a page of {bytes} bytes");
        assert_eq!(p.offset, offset);
        offset += p.bindings.len() as u64;
    }
    let got = iris(&pages);
    let unique: BTreeSet<String> = got.iter().cloned().collect();
    assert_eq!(unique.len(), got.len(), "no binding repeats");
    assert_eq!(unique, scene_iris(store));
    assert_eq!(unique.len(), SCENES);
}

#[test]
fn page_size_and_limit() {
    let store = shared();
    let req = PageRequest {
        page_size: Some(100),
        ..Default::default()
    };
    let first = page(store, EVERY_SCENE, &req);
    assert_eq!(first.bindings.len(), 100);
    let second = page(store, EVERY_SCENE, &next(&req, &first).unwrap());
    assert_eq!(second.bindings.len(), 100);
    assert_eq!(second.offset, 100);

    let pages = all_pages(
        store,
        EVERY_SCENE,
        PageRequest {
            page_size: Some(100),
            limit: Some(250),
            continuation: None,
        },
    );
    let sizes: Vec<usize> = pages.iter().map(|p| p.bindings.len()).collect();
    assert_eq!(sizes, [100, 100, 50]);
    assert!(pages.last().unwrap().continuation.is_none());

    let exact = all_pages(
        store,
        EVERY_SCENE,
        PageRequest {
            page_size: Some(100),
            limit: Some(200),
            continuation: None,
        },
    );
    assert_eq!(exact.len(), 2, "no empty page after the limit");
}

#[test]
fn invalid_continuations_are_refused() {
    let store = shared();
    let req = PageRequest {
        page_size: Some(10),
        ..Default::default()
    };
    let c = page(store, EVERY_SCENE, &req).continuation.unwrap();
    let with = |c: &str| PageRequest {
        continuation: Some(c.to_string()),
        ..req.clone()
    };

    // One altered hex digit, anywhere.
    for i in [0, c.len() / 2, c.len() - 1] {
        let mut bytes = c.clone().into_bytes();
        bytes[i] = if bytes[i] == b'0' { b'1' } else { b'0' };
        let altered = String::from_utf8(bytes).unwrap();
        assert_eq!(
            expect_kind(store, EVERY_SCENE, &with(&altered)),
            ErrorKind::PageInvalid,
            "byte {i}"
        );
    }
    // Another query's continuation.
    let other = "SELECT ?p WHERE { ?p a sem:Performer }";
    assert_eq!(expect_kind(store, other, &with(&c)), ErrorKind::PageInvalid);
    // Garbage.
    for g in ["", "zz", "7b7d", "not hex at all"] {
        assert_eq!(
            expect_kind(store, EVERY_SCENE, &with(g)),
            ErrorKind::PageInvalid,
            "{g:?}"
        );
    }
}

#[test]
fn a_publish_between_pages_changes_nothing_and_a_removed_generation_expires() {
    let store = big("paging-publish");
    let req = PageRequest {
        page_size: Some(5_000),
        ..Default::default()
    };
    let first = page(&store, EVERY_SCENE, &req);
    let generation = first.generation.number;

    // A new generation with one more scene.
    let extra = format!("urn:semantic-stash:{SERVER}:scene:999999");
    let mut w = store.writer().unwrap();
    w.add(&[Fact {
        subject: Term::Iri(extra.clone()),
        predicate: Term::Iri(RDF_TYPE.into()),
        object: Term::Iri(format!("{VOCAB}Scene")),
    }])
    .unwrap();
    let published = w.publish().unwrap();
    assert!(published.number > generation);

    let mut pages = vec![first];
    let mut r = next(&req, &pages[0]);
    while let Some(q) = r {
        let p = page(&store, EVERY_SCENE, &q);
        r = next(&q, &p);
        pages.push(p);
    }
    for p in &pages {
        assert_eq!(p.generation.number, generation);
    }
    let got: BTreeSet<String> = iris(&pages).into_iter().collect();
    assert_eq!(got.len(), SCENES);
    assert!(!got.contains(&extra), "the later generation is not read");

    // The first generation removed: its continuation expires.
    let c = page(&store, EVERY_SCENE, &req);
    assert_eq!(c.generation.number, published.number);
    let old = pages[0].continuation.clone().unwrap();
    std::fs::remove_dir_all(store.dir().join("snapshots").join(generation.to_string())).unwrap();
    assert_eq!(
        expect_kind(
            &store,
            EVERY_SCENE,
            &PageRequest {
                continuation: Some(old),
                ..req
            }
        ),
        ErrorKind::PageExpired { generation }
    );
}

#[test]
fn a_single_binding_over_a_page_is_too_large() {
    let mut lib = load_library(&query_fixture_path("scope.json"));
    lib.tags[0].name = "x".repeat(1_100_000 + 10);
    let store = store_of(lib, "paging-too-large");
    assert_eq!(
        expect_kind(
            &store,
            "SELECT ?n WHERE { tag:1 rdfs:label ?n }",
            &PageRequest::default()
        ),
        ErrorKind::TooLarge
    );
}
