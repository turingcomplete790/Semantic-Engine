//! Names: vocabulary terms, tags by name, and items by id (spec 009 research R2; data-model
//! "Resolved"). Every name is checked against the vocabulary and the one generation the query
//! runs on, so an unknown name is an error at its position, never a silent empty answer.

use super::error::{Pos, QueryError};
use super::lookup::Lookup;
use super::parse::{Ast, CompareOp, IriRef, Node, NodeKind};
use crate::build::update::owner_kind;
use crate::graph::Term;
use stash_adapter::reader::Kind;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const VOCAB: &str = "urn:semantic-stash:vocab:";
pub const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// Prefixes every query has; a query's `PREFIX` lines may not redefine them.
pub const BUILT_IN_PREFIXES: &[&str] = &[
    "sem",
    "",
    "tag",
    "performer",
    "scene",
    "image",
    "recursive",
    "recursive-tag",
    "rdf",
    "rdfs",
    "xsd",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstKind {
    Vocab,
    Item(Kind),
    Literal,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
    pub term: Term,
    pub what: ConstKind,
    pub recursive: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RNode {
    Var(String),
    Const(Const),
}

impl RNode {
    pub fn var(&self) -> Option<&str> {
        match self {
            RNode::Var(v) => Some(v),
            RNode::Const(_) => None,
        }
    }

    pub fn iri(&self) -> Option<&str> {
        match self {
            RNode::Const(Const {
                term: Term::Iri(i), ..
            }) => Some(i),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RTriple {
    pub s: RNode,
    pub p: RNode,
    pub o: RNode,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RFilter {
    pub left: RNode,
    pub op: CompareOp,
    pub right: RNode,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub variables: Vec<String>,
    pub triples: Vec<RTriple>,
    pub filters: Vec<RFilter>,
}

/// Each vocabulary term's status (`Active`, `Planned`, `Derived`, `Retired`, …), by IRI. A
/// Derived term that a Stash mapping entry produces (`sem:produces`, for example `bustSize`) is
/// stored by the builder, so it counts as `Active`; one no mapping produces (`inStage`) is not
/// stored.
fn statuses() -> &'static HashMap<String, String> {
    static TERMS: OnceLock<HashMap<String, String>> = OnceLock::new();
    TERMS.get_or_init(|| {
        let v = semantic_vocabulary::embedded().expect("the embedded vocabulary parses");
        let produces = format!("{VOCAB}produces");
        let produced: std::collections::HashSet<String> = v
            .all_triples()
            .filter(|(_, t)| t.predicate.as_str() == produces)
            .map(|(_, t)| {
                let o = t.object.to_string();
                o.trim_start_matches('<').trim_end_matches('>').to_string()
            })
            .collect();
        v.terms()
            .map(|t| {
                let status = t
                    .values("status")
                    .first()
                    .map(|s| s.strip_prefix(VOCAB).unwrap_or(s).to_string())
                    .unwrap_or_else(|| "Active".into());
                let status = if status == "Derived" && produced.contains(&t.iri) {
                    "Active".to_string()
                } else {
                    status
                };
                (t.iri.clone(), status)
            })
            .collect()
    })
}

/// Whether `iri` is a vocabulary term, and if so its status as queries see it.
pub fn term_status(iri: &str) -> Option<&'static str> {
    statuses().get(iri).map(String::as_str)
}

struct Names<'r> {
    reader: &'r dyn Lookup,
    server: &'r str,
    prefixes: HashMap<String, String>,
    tags: HashMap<String, Result<String, String>>,
    exists: HashMap<String, bool>,
}

impl Names<'_> {
    fn item_iri(&self, kind: &str, id: &str) -> String {
        format!("urn:semantic-stash:{}:{kind}:{id}", self.server)
    }

    fn item_prefix(&self) -> String {
        format!("urn:semantic-stash:{}:", self.server)
    }

    /// The one tag whose Stash name is `name`.
    fn tag_by_name(&mut self, name: &str) -> Result<String, String> {
        if let Some(r) = self.tags.get(name) {
            return r.clone();
        }
        let subjects = self
            .reader
            .subjects(&format!("{RDFS}label"), &Term::string(name))
            .map_err(|e| e.message)?;
        let mut found: Vec<String> = subjects
            .into_iter()
            .filter_map(|t| match t {
                Term::Iri(i) if owner_kind(&i) == Some(Kind::Tag) => Some(i),
                _ => None,
            })
            .collect();
        found.dedup();
        found.sort();
        let r = match found.len() {
            0 => Err("no tag has this name".to_string()),
            1 => Ok(found.remove(0)),
            n => Err(format!(
                "{n} tags have this name; name one by id, as tag:{}",
                found[0].rsplit(':').next().unwrap_or("")
            )),
        };
        self.tags.insert(name.to_string(), r.clone());
        r
    }

    /// Whether the generation holds any fact about `iri`, as subject or object.
    fn item_exists(&mut self, iri: &str) -> Result<bool, QueryError> {
        if let Some(b) = self.exists.get(iri) {
            return Ok(*b);
        }
        let b = self.reader.mentions(iri)?;
        self.exists.insert(iri.to_string(), b);
        Ok(b)
    }

    fn is_role_or_relation(&self, iri: &str) -> Result<bool, QueryError> {
        let classes = self.reader.objects(iri, &format!("{RDF}type"))?;
        Ok(classes.into_iter().any(|c| {
            matches!(c, Term::Iri(c) if c == format!("{VOCAB}Role") || c == format!("{VOCAB}Relation"))
        }))
    }

    fn vocab_const(&self, iri: String, shown: &str, pos: Pos) -> Result<Const, QueryError> {
        match term_status(&iri) {
            Some("Active") => Ok(Const {
                term: Term::Iri(iri),
                what: ConstKind::Vocab,
                recursive: false,
                pos,
            }),
            Some(status) => Err(QueryError::unknown(
                shown,
                format!("this vocabulary term is {status}, so the graph holds no facts for it"),
                pos,
            )),
            None => Err(QueryError::unknown(shown, "not a vocabulary term", pos)),
        }
    }

    /// An item by its IRI: it must exist in the generation.
    fn item_const(
        &mut self,
        iri: String,
        kind: Kind,
        shown: &str,
        pos: Pos,
    ) -> Result<Const, QueryError> {
        if !self.item_exists(&iri)? {
            return Err(QueryError::unknown(
                shown,
                format!("no {} with this id is in the graph", kind.as_str()),
                pos,
            ));
        }
        Ok(Const {
            term: Term::Iri(iri),
            what: ConstKind::Item(kind),
            recursive: false,
            pos,
        })
    }

    fn recursive(&self, mut c: Const, shown: &str) -> Result<Const, QueryError> {
        let iri = match &c.term {
            Term::Iri(i) => i.clone(),
            _ => unreachable!("items are IRIs"),
        };
        if !self.is_role_or_relation(&iri)? {
            return Err(QueryError::new(
                super::error::ErrorKind::NotRecursive {
                    name: shown.to_string(),
                },
                format!("{shown}: only a Role or a Relation can be recursive"),
                Some(c.pos),
            ));
        }
        c.recursive = true;
        Ok(c)
    }

    /// A full IRI, checked when it is in the vocabulary's or this server's item namespace.
    fn full(&mut self, iri: String, shown: &str, pos: Pos) -> Result<Const, QueryError> {
        if iri.starts_with(VOCAB) {
            return self.vocab_const(iri, shown, pos);
        }
        if iri.starts_with(&self.item_prefix()) {
            if let Some(kind) = owner_kind(&iri) {
                return self.item_const(iri, kind, shown, pos);
            }
        }
        Ok(Const {
            term: Term::Iri(iri),
            what: ConstKind::Other,
            recursive: false,
            pos,
        })
    }

    fn item_by_id(
        &mut self,
        kind: Kind,
        id: &str,
        shown: &str,
        pos: Pos,
    ) -> Result<Const, QueryError> {
        if !semantic_vocabulary::iri::valid_id(id) {
            return Err(QueryError::unknown(shown, "not a Stash id", pos));
        }
        let iri = self.item_iri(kind.as_str(), id);
        self.item_const(iri, kind, shown, pos)
    }

    fn tag_named(&mut self, name: &str, shown: &str, pos: Pos) -> Result<Const, QueryError> {
        match self.tag_by_name(name) {
            Ok(iri) => Ok(Const {
                term: Term::Iri(iri),
                what: ConstKind::Item(Kind::Tag),
                recursive: false,
                pos,
            }),
            Err(why) => Err(QueryError::unknown(shown, why, pos)),
        }
    }

    fn iri(&mut self, r: &IriRef, pos: Pos) -> Result<Const, QueryError> {
        let (prefix, local) = match r {
            IriRef::Full(i) => return self.full(i.clone(), &format!("<{i}>"), pos),
            IriRef::Prefixed { prefix, local } => (prefix.as_str(), local.as_str()),
        };
        let shown = format!("{prefix}:{local}");
        let name = super::lex::percent_decode(local);
        let kind_of = |p: &str| match p {
            "tag" => Some(Kind::Tag),
            "performer" => Some(Kind::Performer),
            "scene" => Some(Kind::Scene),
            "image" => Some(Kind::Image),
            _ => None,
        };
        match prefix {
            "sem" => self.vocab_const(format!("{VOCAB}{name}"), &shown, pos),
            "" => {
                let iri = format!("{VOCAB}{name}");
                if term_status(&iri).is_some() {
                    self.vocab_const(iri, &shown, pos)
                } else {
                    self.tag_named(&name, &shown, pos)
                }
            }
            "recursive" => {
                let c = self.tag_named(&name, &shown, pos)?;
                self.recursive(c, &shown)
            }
            "recursive-tag" => {
                let c = self.item_by_id(Kind::Tag, &name, &shown, pos)?;
                self.recursive(c, &shown)
            }
            "rdf" => Ok(other(format!("{RDF}{name}"), pos)),
            "rdfs" => Ok(other(format!("{RDFS}{name}"), pos)),
            "xsd" => Ok(other(format!("{XSD}{name}"), pos)),
            p => {
                if let Some(kind) = kind_of(p) {
                    return self.item_by_id(kind, &name, &shown, pos);
                }
                match self.prefixes.get(p).cloned() {
                    Some(base) => self.full(format!("{base}{name}"), &shown, pos),
                    None => Err(QueryError::unknown(
                        format!("{p}:"),
                        "an unknown prefix; declare it with PREFIX",
                        pos,
                    )),
                }
            }
        }
    }

    /// A datatype IRI: expanded, never checked.
    fn datatype(&self, r: &IriRef, pos: Pos) -> Result<String, QueryError> {
        match r {
            IriRef::Full(i) => Ok(i.clone()),
            IriRef::Prefixed { prefix, local } => {
                let local = super::lex::percent_decode(local);
                let base = match prefix.as_str() {
                    "xsd" => XSD.to_string(),
                    "rdf" => RDF.to_string(),
                    "rdfs" => RDFS.to_string(),
                    "sem" => VOCAB.to_string(),
                    p => match self.prefixes.get(p) {
                        Some(b) => b.clone(),
                        None => {
                            return Err(QueryError::unknown(
                                format!("{p}:"),
                                "an unknown prefix; declare it with PREFIX",
                                pos,
                            ))
                        }
                    },
                };
                Ok(format!("{base}{local}"))
            }
        }
    }

    fn node(&mut self, n: &Node) -> Result<RNode, QueryError> {
        Ok(match &n.kind {
            NodeKind::Var(v) => RNode::Var(v.clone()),
            NodeKind::Iri(r) => RNode::Const(self.iri(r, n.pos)?),
            NodeKind::Literal {
                lexical,
                datatype,
                language,
            } => {
                let datatype = match datatype {
                    Some(d) => Some(self.datatype(d, n.pos)?),
                    None => None,
                };
                RNode::Const(Const {
                    term: Term::Literal {
                        value: lexical.clone(),
                        datatype,
                        language: language.clone(),
                    },
                    what: ConstKind::Literal,
                    recursive: false,
                    pos: n.pos,
                })
            }
        })
    }
}

fn other(iri: String, pos: Pos) -> Const {
    Const {
        term: Term::Iri(iri),
        what: ConstKind::Other,
        recursive: false,
        pos,
    }
}

/// Resolves every name of `ast` against `reader`'s generation, for the store of `server`.
pub fn resolve(ast: &Ast, reader: &dyn Lookup, server: &str) -> Result<Resolved, QueryError> {
    let mut names = Names {
        reader,
        server,
        prefixes: ast
            .prefixes
            .iter()
            .map(|(n, i, _)| (n.clone(), i.clone()))
            .collect(),
        tags: HashMap::new(),
        exists: HashMap::new(),
    };
    let mut triples = Vec::new();
    for t in &ast.patterns {
        triples.push(RTriple {
            s: names.node(&t.s)?,
            p: names.node(&t.p)?,
            o: names.node(&t.o)?,
            pos: t.pos,
        });
    }
    let mut filters = Vec::new();
    for f in &ast.filters {
        filters.push(RFilter {
            left: names.node(&f.left)?,
            op: f.op,
            right: names.node(&f.right)?,
            pos: f.pos,
        });
    }
    Ok(Resolved {
        variables: ast.variables(),
        triples,
        filters,
    })
}
