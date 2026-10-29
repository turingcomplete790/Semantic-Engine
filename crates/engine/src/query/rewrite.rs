//! The rewrite: the documents' short form tied to its scene, and recursive criteria (spec 009
//! research R3, R5; contracts/query-language.md "Meaning").

use super::error::{ErrorKind, QueryError};
use super::lookup::Lookup;
use super::names::{ConstKind, RNode, Resolved, VOCAB};
use super::parse::CompareOp;
use super::HIDDEN;
use crate::graph::Term;
use crate::relations::hierarchy::Hierarchy;
use stash_adapter::reader::Kind;

/// A term of the printed query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PTerm {
    Var(String),
    Term(Term),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub variables: Vec<String>,
    pub patterns: Vec<[PTerm; 3]>,
    /// `VALUES ?var { … }` blocks, from recursive criteria.
    pub values: Vec<(String, Vec<Term>)>,
    pub filters: Vec<(PTerm, CompareOp, PTerm)>,
}

/// The performer facts that hold within one media entity.
const PERFORMER_SCOPED: [&str; 3] = ["hasRole", "hasAction", "hasPluggable"];
const MEDIA_SCOPED: &str = "hasMiseEnScene";

fn sem(local: &str) -> String {
    format!("{VOCAB}{local}")
}

/// A node's identity for comparing terms of the query: its variable, or its term.
fn key(n: &RNode) -> String {
    match n {
        RNode::Var(v) => format!("?{v}"),
        RNode::Const(c) => c.term.to_ntriples(),
    }
}

struct Hidden(usize);

impl Hidden {
    fn next(&mut self) -> String {
        self.0 += 1;
        format!("{HIDDEN}{}", self.0)
    }
}

/// Rewrites `r` into a plan. `expand` gives the tag IRIs a recursive tag IRI matches.
pub fn rewrite(
    r: &Resolved,
    expand: &mut dyn FnMut(&str) -> Result<Vec<String>, QueryError>,
) -> Result<Plan, QueryError> {
    let mut hidden = Hidden(0);
    let mut values: Vec<(String, Vec<Term>)> = Vec::new();
    // One hidden variable per distinct recursive tag.
    let mut recursive: Vec<(String, String)> = Vec::new();
    let mut term = |n: &RNode,
                    hidden: &mut Hidden,
                    values: &mut Vec<(String, Vec<Term>)>|
     -> Result<PTerm, QueryError> {
        Ok(match n {
            RNode::Var(v) => PTerm::Var(v.clone()),
            RNode::Const(c) if c.recursive => {
                let Term::Iri(iri) = &c.term else {
                    unreachable!("recursive constants are tags")
                };
                if let Some((_, v)) = recursive.iter().find(|(i, _)| i == iri) {
                    PTerm::Var(v.clone())
                } else {
                    let v = hidden.next();
                    let tags = expand(iri)?.into_iter().map(Term::Iri).collect();
                    values.push((v.clone(), tags));
                    recursive.push((iri.clone(), v.clone()));
                    PTerm::Var(v)
                }
            }
            RNode::Const(c) => PTerm::Term(c.term.clone()),
        })
    };

    let has_performer = sem("hasPerformer");
    let mut patterns = Vec::new();
    for (i, t) in r.triples.iter().enumerate() {
        let verb = t.p.iri().and_then(|p| p.strip_prefix(VOCAB));
        let performer_scoped = verb.is_some_and(|v| PERFORMER_SCOPED.contains(&v));
        let media_scoped = verb == Some(MEDIA_SCOPED);
        let short = (performer_scoped || media_scoped)
            && match &t.o {
                RNode::Const(c) => c.what == ConstKind::Item(Kind::Tag),
                RNode::Var(v) => !r
                    .triples
                    .iter()
                    .enumerate()
                    .any(|(j, u)| j != i && u.s.var() == Some(v)),
            };
        let s = term(&t.s, &mut hidden, &mut values)?;
        let p = term(&t.p, &mut hidden, &mut values)?;
        let o = term(&t.o, &mut hidden, &mut values)?;
        if !short {
            patterns.push([s, p, o]);
            continue;
        }
        let instance = PTerm::Var(hidden.next());
        patterns.push([s, p, instance.clone()]);
        patterns.push([instance.clone(), PTerm::Term(Term::Iri(sem("ofTag"))), o]);
        if performer_scoped {
            let performer = key(&t.s);
            let mut media: Vec<&RNode> = Vec::new();
            for u in &r.triples {
                if u.p.iri() == Some(has_performer.as_str())
                    && key(&u.o) == performer
                    && !media.iter().any(|m| key(m) == key(&u.s))
                {
                    media.push(&u.s);
                }
            }
            let m = match media.as_slice() {
                [] => PTerm::Var(hidden.next()),
                [one] => term(one, &mut hidden, &mut values)?,
                _ => {
                    return Err(QueryError::new(
                        ErrorKind::ScopeAmbiguous {
                            performer: performer.clone(),
                        },
                        format!(
                            "{performer} is linked to {} scenes or images by hasPerformer, so \
                             this {} fact could hold in any of them; link the performer to one, \
                             or write the fact with its instance and inMedia",
                            media.len(),
                            verb.unwrap_or("")
                        ),
                        Some(t.pos),
                    ))
                }
            };
            patterns.push([instance, PTerm::Term(Term::Iri(sem("inMedia"))), m]);
        }
    }

    let mut filters = Vec::new();
    for f in &r.filters {
        let left = term(&f.left, &mut hidden, &mut values)?;
        let right = term(&f.right, &mut hidden, &mut values)?;
        filters.push((left, f.op, right));
    }
    Ok(Plan {
        variables: r.variables.clone(),
        patterns,
        values,
        filters,
    })
}

/// The `expand` of [`rewrite`] for one generation (research R5): the tag and every narrower Role
/// or Relation, by spec 004's hierarchy with loops removed. The generation's `sem:broaderRole`
/// and `sem:broaderRelation` links are read once, at the first recursive constant.
pub fn hierarchy_expand(
    reader: &dyn Lookup,
) -> impl FnMut(&str) -> Result<Vec<String>, QueryError> + '_ {
    let mut hierarchy: Option<Hierarchy> = None;
    move |tag| {
        if hierarchy.is_none() {
            let mut links = Vec::new();
            for p in ["broaderRole", "broaderRelation"] {
                for (c, b) in reader.pairs(&format!("{VOCAB}{p}"))? {
                    if let (Term::Iri(c), Term::Iri(b)) = (c, b) {
                        links.push((c, b));
                    }
                }
            }
            hierarchy = Some(Hierarchy::from_links(links));
        }
        let h = hierarchy.as_ref().expect("read above");
        Ok(h.expand_criterion(tag, true).into_iter().collect())
    }
}
