//! The engine's own graph terms (spec 006 data-model; research R1). Every conversion to and from
//! Oxigraph's types is in this file, so no other part of the engine sees them (rule G9).

use super::error::GraphError;
use oxigraph::model as ox;
use serde::{Deserialize, Serialize};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
const RDF_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

/// One RDF term. The order is only for sorting sets of facts (spec 007 research R9).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Term {
    /// An absolute IRI.
    Iri(String),
    /// A blank node label, `[A-Za-z0-9_-]+`.
    Blank(String),
    /// A literal: a plain string, a typed value, or a language-tagged string; never both a
    /// datatype and a language.
    Literal {
        value: String,
        datatype: Option<String>,
        language: Option<String>,
    },
}

impl Term {
    pub fn iri(s: impl Into<String>) -> Term {
        Term::Iri(s.into())
    }
    pub fn string(s: impl Into<String>) -> Term {
        Term::Literal {
            value: s.into(),
            datatype: None,
            language: None,
        }
    }
    pub fn typed(s: impl Into<String>, datatype: impl Into<String>) -> Term {
        Term::Literal {
            value: s.into(),
            datatype: Some(datatype.into()),
            language: None,
        }
    }

    /// The term in N-Triples form. A plain string is written without a datatype.
    pub fn to_ntriples(&self) -> String {
        match self {
            Term::Iri(i) => format!("<{i}>"),
            Term::Blank(b) => format!("_:{b}"),
            Term::Literal {
                value,
                datatype,
                language,
            } => {
                let mut out = String::from("\"");
                for c in value.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        _ => out.push(c),
                    }
                }
                out.push('"');
                match (datatype, language) {
                    (_, Some(lang)) => format!("{out}@{lang}"),
                    (Some(dt), None) if dt != XSD_STRING => format!("{out}^^<{dt}>"),
                    _ => out,
                }
            }
        }
    }
}

/// One statement in the store's default graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Fact {
    /// `Iri` or `Blank` only.
    pub subject: Term,
    /// `Iri` only.
    pub predicate: Term,
    pub object: Term,
}

impl Fact {
    pub fn new(subject: Term, predicate: Term, object: Term) -> Fact {
        Fact {
            subject,
            predicate,
            object,
        }
    }

    /// One N-Triples line, without the line break.
    pub fn to_ntriples(&self) -> String {
        format!(
            "{} {} {} .",
            self.subject.to_ntriples(),
            self.predicate.to_ntriples(),
            self.object.to_ntriples()
        )
    }
}

/// Facts owned by one item: they live in the graph named by `owner`, an absolute IRI (spec 008
/// research R1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owned {
    pub owner: String,
    pub facts: Vec<Fact>,
}

/// What `Writer::replace_owned` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replaced {
    Unchanged,
    Changed { removed: u64, added: u64 },
}

/// A fact pattern for `remove_matching`; `None` matches anything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pattern {
    pub subject: Option<Term>,
    pub predicate: Option<Term>,
    pub object: Option<Term>,
}

/// The answer to a `SELECT` query.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Solutions {
    /// In the query's projection order, without `?`.
    pub variables: Vec<String>,
    /// One row per solution, one cell per variable; `None` where the variable is unbound.
    pub rows: Vec<Vec<Option<Term>>>,
}

fn invalid(message: impl Into<String>) -> GraphError {
    GraphError::InvalidFact {
        message: message.into(),
    }
}

fn named(iri: &str) -> Result<ox::NamedNode, GraphError> {
    ox::NamedNode::new(iri).map_err(|e| invalid(format!("{iri:?} is not an absolute IRI: {e}")))
}

fn blank(label: &str) -> Result<ox::BlankNode, GraphError> {
    let ok = !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !ok {
        return Err(invalid(format!(
            "{label:?} is not a blank node label ([A-Za-z0-9_-]+)"
        )));
    }
    ox::BlankNode::new(label).map_err(|e| invalid(format!("{label:?}: {e}")))
}

pub(crate) fn to_ox_term(t: &Term) -> Result<ox::Term, GraphError> {
    Ok(match t {
        Term::Iri(i) => named(i)?.into(),
        Term::Blank(b) => blank(b)?.into(),
        Term::Literal {
            value,
            datatype,
            language,
        } => match (datatype, language) {
            (Some(_), Some(_)) => {
                return Err(invalid(
                    "a literal has either a datatype or a language, not both",
                ))
            }
            (Some(dt), None) => ox::Literal::new_typed_literal(value.as_str(), named(dt)?).into(),
            (None, Some(lang)) => ox::Literal::new_language_tagged_literal(value.as_str(), lang)
                .map_err(|e| invalid(format!("{lang:?} is not a language tag: {e}")))?
                .into(),
            (None, None) => ox::Literal::new_simple_literal(value.as_str()).into(),
        },
    })
}

fn to_ox_subject(t: &Term) -> Result<ox::Subject, GraphError> {
    match t {
        Term::Iri(i) => Ok(named(i)?.into()),
        Term::Blank(b) => Ok(blank(b)?.into()),
        Term::Literal { .. } => Err(invalid("a subject is an IRI or a blank node")),
    }
}

pub(crate) fn to_ox_predicate(t: &Term) -> Result<ox::NamedNode, GraphError> {
    match t {
        Term::Iri(i) => named(i),
        _ => Err(invalid("a predicate is an IRI")),
    }
}

pub(crate) fn to_ox_quad(f: &Fact) -> Result<ox::Quad, GraphError> {
    Ok(ox::Quad::new(
        to_ox_subject(&f.subject)?,
        to_ox_predicate(&f.predicate)?,
        to_ox_term(&f.object)?,
        ox::GraphName::DefaultGraph,
    ))
}

/// A fact in an owner's graph.
pub(crate) fn to_ox_owned_quad(f: &Fact, graph: &ox::NamedNode) -> Result<ox::Quad, GraphError> {
    Ok(ox::Quad::new(
        to_ox_subject(&f.subject)?,
        to_ox_predicate(&f.predicate)?,
        to_ox_term(&f.object)?,
        graph.clone(),
    ))
}

/// The pattern's parts in Oxigraph's form: subject, predicate, object.
pub(crate) type OxPattern = (Option<ox::Subject>, Option<ox::NamedNode>, Option<ox::Term>);

pub(crate) fn to_ox_pattern(p: &Pattern) -> Result<OxPattern, GraphError> {
    Ok((
        p.subject.as_ref().map(to_ox_subject).transpose()?,
        p.predicate.as_ref().map(to_ox_predicate).transpose()?,
        p.object.as_ref().map(to_ox_term).transpose()?,
    ))
}

pub(crate) fn from_ox_term(t: &ox::Term) -> Term {
    match t {
        ox::Term::NamedNode(n) => Term::Iri(n.as_str().to_string()),
        ox::Term::BlankNode(b) => Term::Blank(b.as_str().to_string()),
        ox::Term::Literal(l) => {
            let language = l.language().map(str::to_string);
            let dt = l.datatype().as_str();
            let datatype = (language.is_none() && dt != XSD_STRING && dt != RDF_LANG_STRING)
                .then(|| dt.to_string());
            Term::Literal {
                value: l.value().to_string(),
                datatype,
                language,
            }
        }
        #[allow(unreachable_patterns)]
        other => Term::string(other.to_string()),
    }
}

/// A literal as the store keeps it: Oxigraph parses the XSD datatypes it knows and writes them
/// back in canonical form, integer-derived types as `xsd:integer` (its `numeric_encoder`). The
/// query index holds terms in the same form (spec 009 research R12). Other terms are unchanged.
pub(crate) fn canonical(t: &Term) -> Term {
    use oxsdatatypes::*;
    use std::str::FromStr;
    let Term::Literal {
        value,
        datatype: Some(d),
        language: None,
    } = t
    else {
        return t.clone();
    };
    fn lit<T: FromStr>(v: &str) -> Option<T> {
        T::from_str(v).ok()
    }
    let xsd = d.strip_prefix("http://www.w3.org/2001/XMLSchema#");
    let parsed: Option<ox::Literal> = match xsd {
        Some("boolean") => lit::<Boolean>(value).map(Into::into),
        Some("float") => lit::<Float>(value).map(Into::into),
        Some("double") => lit::<Double>(value).map(Into::into),
        Some(
            "integer" | "byte" | "short" | "int" | "long" | "unsignedByte" | "unsignedShort"
            | "unsignedInt" | "unsignedLong" | "positiveInteger" | "negativeInteger"
            | "nonPositiveInteger" | "nonNegativeInteger",
        ) => lit::<Integer>(value).map(Into::into),
        Some("decimal") => lit::<Decimal>(value).map(Into::into),
        Some("dateTime" | "dateTimeStamp") => lit::<DateTime>(value).map(Into::into),
        Some("time") => lit::<Time>(value).map(Into::into),
        Some("date") => lit::<Date>(value).map(Into::into),
        Some("gYearMonth") => lit::<GYearMonth>(value).map(Into::into),
        Some("gYear") => lit::<GYear>(value).map(Into::into),
        Some("gMonthDay") => lit::<GMonthDay>(value).map(Into::into),
        Some("gDay") => lit::<GDay>(value).map(Into::into),
        Some("gMonth") => lit::<GMonth>(value).map(Into::into),
        Some("duration") => lit::<Duration>(value).map(Into::into),
        Some("yearMonthDuration") => lit::<YearMonthDuration>(value).map(Into::into),
        Some("dayTimeDuration") => lit::<DayTimeDuration>(value).map(Into::into),
        _ => None,
    };
    match parsed {
        Some(l) => from_ox_term(&ox::Term::Literal(l)),
        None => t.clone(),
    }
}

pub(crate) fn from_ox_quad(q: &ox::Quad) -> Fact {
    let subject = match &q.subject {
        ox::Subject::NamedNode(n) => Term::Iri(n.as_str().to_string()),
        ox::Subject::BlankNode(b) => Term::Blank(b.as_str().to_string()),
        #[allow(unreachable_patterns)]
        other => Term::Iri(other.to_string()),
    };
    Fact {
        subject,
        predicate: Term::Iri(q.predicate.as_str().to_string()),
        object: from_ox_term(&q.object),
    }
}

/// A parsed RDF triple as a fact. The vocabulary crate parses with the same RDF model Oxigraph
/// uses, so its triples convert here, inside the graph module (rule G9; spec 007 research R7).
pub(crate) fn from_ox_triple(t: &ox::Triple) -> Fact {
    from_ox_quad(&ox::Quad::new(
        t.subject.clone(),
        t.predicate.clone(),
        t.object.clone(),
        ox::GraphName::DefaultGraph,
    ))
}

/// Checks a fact without storing it (used by tests and by callers that validate early).
pub fn validate(f: &Fact) -> Result<(), GraphError> {
    to_ox_quad(f).map(|_| ())
}

/// Converts a fact to the store's form and back; used by tests of the conversion.
pub fn round_trip(f: &Fact) -> Result<Fact, GraphError> {
    Ok(from_ox_quad(&to_ox_quad(f)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntriples_escapes_literals_and_writes_datatypes() {
        let f = Fact::new(
            Term::iri("urn:x:s"),
            Term::iri("urn:x:p"),
            Term::string("a \"b\" \\ c\nd"),
        );
        assert_eq!(
            f.to_ntriples(),
            r#"<urn:x:s> <urn:x:p> "a \"b\" \\ c\nd" ."#
        );
        let t = Term::typed("1.5", "http://www.w3.org/2001/XMLSchema#decimal");
        assert_eq!(
            t.to_ntriples(),
            "\"1.5\"^^<http://www.w3.org/2001/XMLSchema#decimal>"
        );
        assert_eq!(Term::typed("x", XSD_STRING).to_ntriples(), "\"x\"");
        assert_eq!(Term::Blank("b1".into()).to_ntriples(), "_:b1");
    }
}
