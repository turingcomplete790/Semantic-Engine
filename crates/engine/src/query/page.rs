//! Answers: typed values and pages (spec 009 research R4, R6; data-model "Page and
//! continuation"; contracts/operations-1.4.md "Value types").

use super::names::{VOCAB, XSD};
use crate::build::update::owner_kind;
use crate::graph::{Generation, Term};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};
use std::cmp::Ordering;
use std::path::Path;

/// A page's serialized `bindings` stay within this many bytes (decision record rule 9).
pub const PAGE_BYTES: usize = 1_048_576;
/// At most this many bindings in one page.
pub const PAGE_ROWS_MAX: u32 = 20_000;

const RDF_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";

/// One value of a binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Value {
    /// A Stash item: a tag, performer, scene or image.
    Item {
        iri: String,
        kind: String,
        id: String,
    },
    /// Any other node: an instance, pairing, timeline entry or rule.
    Node { iri: String },
    /// A vocabulary term.
    Term { iri: String, name: String },
    Literal {
        value: String,
        datatype: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
}

impl Value {
    pub fn from_term(t: &Term) -> Value {
        match t {
            Term::Iri(iri) => {
                if let Some(name) = iri.strip_prefix(VOCAB) {
                    if !name.is_empty() {
                        return Value::Term {
                            iri: iri.clone(),
                            name: name.to_string(),
                        };
                    }
                }
                match owner_kind(iri) {
                    Some(kind) => Value::Item {
                        iri: iri.clone(),
                        kind: kind.as_str().to_string(),
                        id: iri.rsplit(':').next().unwrap_or("").to_string(),
                    },
                    None => Value::Node { iri: iri.clone() },
                }
            }
            Term::Blank(b) => Value::Node {
                iri: format!("_:{b}"),
            },
            Term::Literal {
                value,
                datatype,
                language,
            } => Value::Literal {
                value: value.clone(),
                datatype: match (datatype, language) {
                    (Some(d), _) => d.clone(),
                    (None, Some(_)) => RDF_LANG_STRING.to_string(),
                    (None, None) => format!("{XSD}string"),
                },
                language: language.clone(),
            },
        }
    }
}

/// The answer order (research R4): binding by binding, by each projected variable in projection
/// order. Within one variable: unbound, then blank nodes, then IRIs, then literals. IRIs and
/// blank nodes compare by their text; numeric literals by value, before every other literal;
/// other literals by lexical form, datatype and language. Ties are broken by the full term, so
/// the order is total and the same on every evaluation of one generation.
pub fn compare_rows(a: &[Option<Term>], b: &[Option<Term>]) -> Ordering {
    a.iter()
        .zip(b)
        .map(|(x, y)| compare_terms(x.as_ref(), y.as_ref()))
        .find(|o| o.is_ne())
        .unwrap_or_else(|| a.len().cmp(&b.len()))
}

const NUMERIC: &[&str] = &[
    "integer",
    "decimal",
    "double",
    "float",
    "int",
    "long",
    "short",
    "byte",
    "nonNegativeInteger",
    "positiveInteger",
    "nonPositiveInteger",
    "negativeInteger",
    "unsignedInt",
    "unsignedLong",
    "unsignedShort",
    "unsignedByte",
];

/// The value of a numeric literal, when it is one and parses.
pub(crate) fn numeric(datatype: Option<&str>, value: &str) -> Option<f64> {
    let local = datatype?.strip_prefix(XSD)?;
    if !NUMERIC.contains(&local) {
        return None;
    }
    value.trim().parse::<f64>().ok().filter(|v| !v.is_nan())
}

pub(crate) fn compare_terms(a: Option<&Term>, b: Option<&Term>) -> Ordering {
    fn rank(t: Option<&Term>) -> u8 {
        match t {
            None => 0,
            Some(Term::Blank(_)) => 1,
            Some(Term::Iri(_)) => 2,
            Some(Term::Literal { .. }) => 3,
        }
    }
    match (a, b) {
        (Some(Term::Iri(x)), Some(Term::Iri(y))) | (Some(Term::Blank(x)), Some(Term::Blank(y))) => {
            x.cmp(y)
        }
        (
            Some(Term::Literal {
                value: v1,
                datatype: d1,
                language: l1,
            }),
            Some(Term::Literal {
                value: v2,
                datatype: d2,
                language: l2,
            }),
        ) => {
            let n1 = numeric(d1.as_deref(), v1);
            let n2 = numeric(d2.as_deref(), v2);
            let by_value = match (n1, n2) {
                (Some(x), Some(y)) => x.total_cmp(&y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            by_value
                .then_with(|| v1.cmp(v2))
                .then_with(|| d1.cmp(d2))
                .then_with(|| l1.cmp(l2))
        }
        _ => rank(a).cmp(&rank(b)),
    }
}

/// What a client asks for besides the query (contracts/operations-1.4.md "Arguments").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageRequest {
    /// 1 to 20,000.
    pub page_size: Option<u32>,
    /// At least 1.
    pub limit: Option<u64>,
    pub continuation: Option<String>,
}

/// One page of an answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub variables: Vec<String>,
    /// One row per binding, one value per variable, in `variables` order.
    pub bindings: Vec<Vec<Value>>,
    /// The position of this page's first binding in the whole answer.
    pub offset: u64,
    /// Present when more bindings follow.
    pub continuation: Option<String>,
    /// The generation that answered.
    pub generation: Generation,
    /// Something the caller should log: the generation's query index could not be read.
    pub note: Option<String>,
}

/// A binding as the envelope sends it: an object from variable to value.
pub fn binding_json(variables: &[String], row: &[Value]) -> Json {
    let mut m = Map::new();
    for (v, x) in variables.iter().zip(row) {
        m.insert(
            v.clone(),
            serde_json::to_value(x).expect("values serialize"),
        );
    }
    Json::Object(m)
}

impl Page {
    /// The bindings as the envelope sends them.
    pub fn bindings_json(&self) -> Json {
        Json::Array(
            self.bindings
                .iter()
                .map(|r| binding_json(&self.variables, r))
                .collect(),
        )
    }
}

/// The serialized size of one binding, with the comma that separates it from the next.
pub fn binding_bytes(variables: &[String], row: &[Value]) -> usize {
    /// Counts what is written, without keeping it.
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0 += b.len();
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    let mut map = serde_json::Serializer::new(&mut count);
    use serde::ser::{SerializeMap, Serializer};
    let ok = (|| {
        let mut m = map.serialize_map(Some(variables.len()))?;
        for (v, x) in variables.iter().zip(row) {
            m.serialize_entry(v, x)?;
        }
        m.end()
    })();
    match ok {
        Ok(()) => count.0 + 1,
        Err(_) => usize::MAX,
    }
}

/// Where a later page continues (research R6; data-model "Page and continuation"). Sent to the
/// client as the lower-case hex of its JSON; nothing is kept between calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Continuation {
    pub v: u32,
    pub generation: u64,
    pub offset: u64,
    pub page_size: Option<u32>,
    pub limit: Option<u64>,
    pub query_hash: u64,
    pub check: u64,
}

/// The continuation format.
const CONTINUATION_VERSION: u32 = 1;
/// The check key's file, in the store's parent folder (the plugin folder).
pub const KEY_FILE: &str = "query.key";
const KEY_BYTES: usize = 16;

/// FNV-1a, 64 bits.
pub fn fnv1a64(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in *part {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

impl Continuation {
    pub fn new(
        generation: u64,
        offset: u64,
        page_size: Option<u32>,
        limit: Option<u64>,
        query: &str,
        key: &[u8],
    ) -> Continuation {
        let mut c = Continuation {
            v: CONTINUATION_VERSION,
            generation,
            offset,
            page_size,
            limit,
            query_hash: fnv1a64(&[query.as_bytes()]),
            check: 0,
        };
        c.check = c.expected_check(key);
        c
    }

    /// The check over the key and every other field.
    fn expected_check(&self, key: &[u8]) -> u64 {
        let fields = serde_json::to_vec(&Continuation {
            check: 0,
            ..self.clone()
        })
        .expect("serializes");
        fnv1a64(&[key, &fields])
    }

    pub fn encode(&self) -> String {
        let json = serde_json::to_vec(self).expect("serializes");
        json.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Decodes `text` and checks it against `key` and `query`; `None` when any of it fails.
    pub fn decode(text: &str, query: &str, key: &[u8]) -> Option<Continuation> {
        if !text.len().is_multiple_of(2)
            || !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return None;
        }
        let bytes: Vec<u8> = (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16))
            .collect::<Result<_, _>>()
            .ok()?;
        let c: Continuation = serde_json::from_slice(&bytes).ok()?;
        (c.v == CONTINUATION_VERSION
            && c.query_hash == fnv1a64(&[query.as_bytes()])
            && c.check == c.expected_check(key))
        .then_some(c)
    }
}

/// The check key in `folder`. With `create`, a missing key is made: 16 random bytes, written
/// to a temporary file and linked into place, so two first calls agree on one key. A disposable
/// cache: losing it only makes outstanding continuations invalid.
pub fn check_key(folder: &Path, create: bool) -> std::io::Result<Option<Vec<u8>>> {
    let path = folder.join(KEY_FILE);
    let read = |p: &Path| match std::fs::read(p) {
        Ok(k) if k.len() == KEY_BYTES => Ok(Some(k)),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    };
    if let Some(k) = read(&path)? {
        return Ok(Some(k));
    }
    if !create {
        return Ok(None);
    }
    let tmp = folder.join(format!("{KEY_FILE}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, uuid::Uuid::new_v4().as_bytes())?;
    let linked = std::fs::hard_link(&tmp, &path);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    read(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(v: &str) -> Option<Term> {
        Some(Term::typed(v, format!("{XSD}integer")))
    }

    #[test]
    fn the_answer_order_is_total() {
        let mut rows = vec![
            vec![Some(Term::string("b"))],
            vec![int("10")],
            vec![Some(Term::iri("urn:x:b"))],
            vec![None],
            vec![int("9")],
            vec![Some(Term::Blank("n1".into()))],
            vec![Some(Term::string("a"))],
            vec![Some(Term::iri("urn:x:a"))],
            vec![Some(Term::typed("9.0", format!("{XSD}decimal")))],
        ];
        rows.sort_by(|a, b| compare_rows(a, b));
        assert_eq!(
            rows,
            vec![
                vec![None],
                vec![Some(Term::Blank("n1".into()))],
                vec![Some(Term::iri("urn:x:a"))],
                vec![Some(Term::iri("urn:x:b"))],
                vec![int("9")],
                vec![Some(Term::typed("9.0", format!("{XSD}decimal")))],
                vec![int("10")],
                vec![Some(Term::string("a"))],
                vec![Some(Term::string("b"))],
            ]
        );
        // By the first variable, then the next.
        let a = [Some(Term::iri("urn:x:a")), Some(Term::iri("urn:x:z"))];
        let b = [Some(Term::iri("urn:x:b")), Some(Term::iri("urn:x:a"))];
        assert_eq!(compare_rows(&a, &b), Ordering::Less);
    }
}
