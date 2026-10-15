//! Semantic vocabulary for the Stash semantic engine.
//!
//! Loads the Turtle vocabulary files, exposes the terms, builds item IRIs, and checks the
//! vocabulary against the rules in `specs/001-semantic-vocabulary/contracts/vocabulary-check.md`.

pub mod check;
pub mod iri;
mod terms_md;

use oxrdf::{Subject, Term, Triple};
use oxttl::TurtleParser;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use check::Violation;

pub const NS: &str = "urn:semantic-stash:vocab:";
pub const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
pub const FILE_NAMES: [&str; 4] = [
    "core.ttl",
    "labels.ttl",
    "timeline.ttl",
    "stash-mapping.ttl",
];
pub const CONTRACT_VERSION: &str = "1.0.0";

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{file}: {message}")]
    Parse { file: String, message: String },
    #[error("{file}: blank nodes are not allowed in the vocabulary")]
    BlankNode { file: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermKind {
    Class,
    Property,
}

impl TermKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TermKind::Class => "class",
            TermKind::Property => "property",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VocabFile {
    pub name: String,
    pub text: String,
    pub triples: Vec<Triple>,
}

/// One class or property with its annotations.
#[derive(Debug, Clone)]
pub struct TermInfo {
    pub iri: String,
    pub kind: TermKind,
    pub file: String,
    /// predicate IRI -> values (IRIs and literal lexical forms, in file order)
    pub props: BTreeMap<String, Vec<String>>,
    /// language tags of rdfs:label values
    pub label_langs: Vec<Option<String>>,
}

impl TermInfo {
    /// Values of a vocabulary (`sem:`) annotation by local name, or an rdfs one with the
    /// `rdfs:` prefix, for example `values("scope")` or `values("rdfs:domain")`.
    pub fn values(&self, name: &str) -> Vec<String> {
        let key = expand(name);
        self.props.get(&key).cloned().unwrap_or_default()
    }
    pub fn local(&self) -> &str {
        self.iri.strip_prefix(NS).unwrap_or(&self.iri)
    }
}

pub(crate) fn expand(name: &str) -> String {
    if let Some(r) = name.strip_prefix("rdfs:") {
        format!("{RDFS}{r}")
    } else if let Some(r) = name.strip_prefix("rdf:") {
        format!("{RDF}{r}")
    } else {
        format!("{NS}{name}")
    }
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub version: String,
    /// local name -> (kind, domain, range)
    pub terms: BTreeMap<String, (String, String, String)>,
}

#[derive(Debug, Clone)]
pub struct Vocabulary {
    pub files: Vec<VocabFile>,
    pub changelog: Option<String>,
    pub terms_md: Option<String>,
    pub snapshots: Vec<Snapshot>,
}

fn parse_file(name: &str, text: &str) -> Result<Vec<Triple>, Error> {
    let mut out = Vec::new();
    for t in TurtleParser::new().for_reader(text.as_bytes()) {
        match t {
            Ok(t) => {
                if matches!(t.subject, Subject::BlankNode(_))
                    || matches!(t.object, Term::BlankNode(_))
                {
                    return Err(Error::BlankNode {
                        file: name.to_string(),
                    });
                }
                out.push(t);
            }
            Err(e) => {
                return Err(Error::Parse {
                    file: name.to_string(),
                    message: e.to_string(),
                })
            }
        }
    }
    Ok(out)
}

pub fn load(dir: &Path) -> Result<Vocabulary, Error> {
    let mut files = Vec::new();
    for name in FILE_NAMES {
        let path = dir.join(name);
        let text = fs::read_to_string(&path).map_err(|source| Error::Io { path, source })?;
        let triples = parse_file(name, &text)?;
        files.push(VocabFile {
            name: name.to_string(),
            text,
            triples,
        });
    }
    let changelog = fs::read_to_string(dir.join("CHANGELOG.md")).ok();
    let terms_md = fs::read_to_string(dir.join("TERMS.md")).ok();
    let mut snapshots = Vec::new();
    if let Ok(rd) = fs::read_dir(dir.join("released")) {
        for e in rd.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) == Some("json") {
                let text = fs::read_to_string(e.path()).map_err(|source| Error::Io {
                    path: e.path(),
                    source,
                })?;
                if let Some(s) = Snapshot::from_json(&text) {
                    snapshots.push(s);
                }
            }
        }
    }
    Ok(Vocabulary {
        files,
        changelog,
        terms_md,
        snapshots,
    })
}

macro_rules! emb {
    ($n:literal) => {
        ($n, include_str!(concat!("../../../vocabulary/", $n)))
    };
}

/// The vocabulary files embedded at build time.
pub fn embedded() -> Result<Vocabulary, Error> {
    let items = [
        emb!("core.ttl"),
        emb!("labels.ttl"),
        emb!("timeline.ttl"),
        emb!("stash-mapping.ttl"),
    ];
    let mut files = Vec::new();
    for (name, text) in items {
        files.push(VocabFile {
            name: name.to_string(),
            text: text.to_string(),
            triples: parse_file(name, text)?,
        });
    }
    Ok(Vocabulary {
        files,
        changelog: Some(include_str!("../../../vocabulary/CHANGELOG.md").to_string()),
        terms_md: Some(include_str!("../../../vocabulary/TERMS.md").to_string()),
        snapshots: Vec::new(),
    })
}

impl Snapshot {
    pub fn from_json(text: &str) -> Option<Snapshot> {
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        let version = v.get("version")?.as_str()?.to_string();
        let mut terms = BTreeMap::new();
        for (k, t) in v.get("terms")?.as_object()? {
            terms.insert(
                k.clone(),
                (
                    t.get("kind")?.as_str()?.to_string(),
                    t.get("domain")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    t.get("range")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                ),
            );
        }
        Some(Snapshot { version, terms })
    }

    pub fn to_json(&self) -> String {
        let mut terms = serde_json::Map::new();
        for (k, (kind, domain, range)) in &self.terms {
            terms.insert(
                k.clone(),
                serde_json::json!({"kind": kind, "domain": domain, "range": range}),
            );
        }
        let v = serde_json::json!({"version": self.version, "terms": terms});
        serde_json::to_string_pretty(&v).unwrap() + "\n"
    }
}

fn obj_value(t: &Term) -> String {
    match t {
        Term::NamedNode(n) => n.as_str().to_string(),
        Term::Literal(l) => l.value().to_string(),
        Term::BlankNode(b) => b.to_string(),
    }
}

impl Vocabulary {
    /// All (file, triple) pairs.
    pub fn all_triples(&self) -> impl Iterator<Item = (&str, &Triple)> {
        self.files
            .iter()
            .flat_map(|f| f.triples.iter().map(move |t| (f.name.as_str(), t)))
    }

    /// Subjects (IRI strings) with the given rdf:type value, with the file of each statement.
    pub(crate) fn typed(&self, type_iri: &str) -> Vec<(String, String)> {
        let rdf_type = format!("{RDF}type");
        let mut v = Vec::new();
        for (file, t) in self.all_triples() {
            if t.predicate.as_str() == rdf_type {
                if let (Subject::NamedNode(s), Term::NamedNode(o)) = (&t.subject, &t.object) {
                    if o.as_str() == type_iri {
                        v.push((s.as_str().to_string(), file.to_string()));
                    }
                }
            }
        }
        v
    }

    fn collect(&self, subject: &str) -> (BTreeMap<String, Vec<String>>, Vec<Option<String>>) {
        let mut props: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut langs = Vec::new();
        let label = format!("{RDFS}label");
        for (_, t) in self.all_triples() {
            if let Subject::NamedNode(s) = &t.subject {
                if s.as_str() == subject {
                    props
                        .entry(t.predicate.as_str().to_string())
                        .or_default()
                        .push(obj_value(&t.object));
                    if t.predicate.as_str() == label {
                        if let Term::Literal(l) = &t.object {
                            langs.push(l.language().map(|x| x.to_string()));
                        }
                    }
                }
            }
        }
        (props, langs)
    }

    /// All predicate values of one subject, for non-term entries (mappings, retired fields).
    pub(crate) fn term_like(&self, iri: &str) -> BTreeMap<String, Vec<String>> {
        self.collect(iri).0
    }

    /// Every class and property defined in the vocabulary, in IRI order.
    pub fn terms(&self) -> impl Iterator<Item = TermInfo> + '_ {
        let mut list: BTreeMap<String, (TermKind, String)> = BTreeMap::new();
        for (iri, file) in self.typed(&format!("{RDFS}Class")) {
            list.entry(iri).or_insert((TermKind::Class, file));
        }
        for (iri, file) in self.typed(&format!("{RDF}Property")) {
            list.entry(iri).or_insert((TermKind::Property, file));
        }
        list.into_iter().map(move |(iri, (kind, file))| {
            let (props, label_langs) = self.collect(&iri);
            TermInfo {
                iri,
                kind,
                file,
                props,
                label_langs,
            }
        })
    }

    pub fn term(&self, iri: &str) -> Option<TermInfo> {
        self.terms().find(|t| t.iri == iri)
    }

    /// True if the IRI is the vocabulary identifier or the subject of an `rdf:type` statement.
    pub fn is_defined(&self, iri: &str) -> bool {
        if iri == NS {
            return true;
        }
        let rdf_type = format!("{RDF}type");
        self.all_triples().any(|(_, t)| {
            t.predicate.as_str() == rdf_type
                && matches!(&t.subject, Subject::NamedNode(s) if s.as_str() == iri)
        })
    }

    pub fn properties_for_domain(&self, class_iri: &str) -> Vec<TermInfo> {
        self.terms()
            .filter(|t| t.kind == TermKind::Property && t.values("rdfs:domain") == [class_iri])
            .collect()
    }

    /// Every `sem:version` literal declared, in file order.
    pub fn declared_versions(&self) -> Vec<String> {
        let pred = format!("{NS}version");
        self.all_triples()
            .filter(|(_, t)| t.predicate.as_str() == pred)
            .map(|(_, t)| obj_value(&t.object))
            .collect()
    }

    pub fn version(&self) -> Option<String> {
        self.declared_versions().into_iter().next()
    }

    fn entries(&self, class: &str) -> Vec<BTreeMap<String, Vec<String>>> {
        self.typed(&format!("{NS}{class}"))
            .into_iter()
            .map(|(iri, _)| {
                let mut p = self.collect(&iri).0;
                p.insert("iri".into(), vec![iri]);
                p
            })
            .collect()
    }

    /// Stash fields covered by mapping entries.
    pub fn mapping_fields(&self) -> Vec<String> {
        let key = format!("{NS}fromField");
        self.entries("Mapping")
            .into_iter()
            .filter_map(|e| e.get(&key).and_then(|v| v.first().cloned()))
            .collect()
    }

    /// Stash fields recorded as retired.
    pub fn retired_fields(&self) -> Vec<String> {
        let key = format!("{NS}fromField");
        self.entries("RetiredField")
            .into_iter()
            .filter_map(|e| e.get(&key).and_then(|v| v.first().cloned()))
            .collect()
    }

    fn first(e: &BTreeMap<String, Vec<String>>, name: &str) -> String {
        e.get(&format!("{NS}{name}"))
            .and_then(|v| v.first().cloned())
            .unwrap_or_default()
    }

    fn short_term(s: String) -> String {
        s.strip_prefix(NS).map(|x| format!("`{x}`")).unwrap_or(s)
    }

    /// (field, entity, allowed values, produced term, status) per mapping entry.
    pub fn mapping_rows(&self) -> Vec<(String, String, String, String, String)> {
        self.entries("Mapping")
            .iter()
            .map(|e| {
                (
                    Self::first(e, "fromField"),
                    Self::first(e, "fromEntity"),
                    Self::first(e, "allowedValues"),
                    Self::short_term(Self::first(e, "produces")),
                    Self::first(e, "status")
                        .strip_prefix(NS)
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .collect()
    }

    /// (field, entity, note, replacement) per retired entry.
    pub fn retired_rows(&self) -> Vec<(String, String, String, String)> {
        self.entries("RetiredField")
            .iter()
            .map(|e| {
                (
                    Self::first(e, "fromField"),
                    Self::first(e, "fromEntity"),
                    Self::first(e, "retiredNote"),
                    Self::short_term(Self::first(e, "replacedBy")),
                )
            })
            .collect()
    }

    /// Mapped and retired fields together.
    pub fn mapped_fields(&self) -> Vec<String> {
        let mut v = self.mapping_fields();
        v.extend(self.retired_fields());
        v
    }

    pub fn generate_terms_md(&self) -> String {
        terms_md::generate(self)
    }

    /// Current vocabulary as a release snapshot.
    pub fn snapshot(&self) -> Snapshot {
        let mut terms = BTreeMap::new();
        for t in self.terms() {
            let local = t.local().to_string();
            let d = t.values("rdfs:domain").first().cloned().unwrap_or_default();
            let r = t.values("rdfs:range").first().cloned().unwrap_or_default();
            let short = |s: String| s.strip_prefix(NS).map(|x| format!("sem:{x}")).unwrap_or(s);
            terms.insert(local, (t.kind.as_str().to_string(), short(d), short(r)));
        }
        Snapshot {
            version: self.version().unwrap_or_default(),
            terms,
        }
    }

    pub fn check(&self) -> Vec<Violation> {
        check::run(self)
    }

    /// A notice when the compatibility rule had nothing to compare against.
    pub fn release_notice(&self) -> Option<String> {
        if self.snapshots.is_empty() {
            Some("no release snapshot found; V014 passes. Write one at release with --write-snapshot"
                .to_string())
        } else {
            None
        }
    }
}
