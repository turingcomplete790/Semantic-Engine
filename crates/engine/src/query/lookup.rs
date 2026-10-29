//! The few lookups name resolution and the hierarchy need (research R2, R5), answered by the
//! query index when the generation has one (research R12), or by the store.

use super::error::QueryError;
use crate::graph::{Reader, Term};
use crate::qindex::{spo, Order, QIndex};

pub trait Lookup {
    /// Every subject `s` with a fact `s predicate object`.
    fn subjects(&self, predicate: &str, object: &Term) -> Result<Vec<Term>, QueryError>;
    /// Every object `o` with a fact `subject predicate o`.
    fn objects(&self, subject: &str, predicate: &str) -> Result<Vec<Term>, QueryError>;
    /// Whether any fact has `iri` as its subject or its object.
    fn mentions(&self, iri: &str) -> Result<bool, QueryError>;
    /// Every `(s, o)` with a fact `s predicate o`.
    fn pairs(&self, predicate: &str) -> Result<Vec<(Term, Term)>, QueryError>;
}

fn failed(e: impl std::fmt::Display) -> QueryError {
    QueryError::internal(format!("a name lookup failed: {e}"))
}

impl Lookup for Reader {
    fn subjects(&self, predicate: &str, object: &Term) -> Result<Vec<Term>, QueryError> {
        let q = format!(
            "SELECT DISTINCT ?s WHERE {{ ?s <{predicate}> {} }}",
            object.to_ntriples()
        );
        Ok(self
            .select(&q)
            .map_err(failed)?
            .rows
            .into_iter()
            .filter_map(|r| r.into_iter().next().flatten())
            .collect())
    }

    fn objects(&self, subject: &str, predicate: &str) -> Result<Vec<Term>, QueryError> {
        let q = format!("SELECT DISTINCT ?o WHERE {{ <{subject}> <{predicate}> ?o }}");
        Ok(self
            .select(&q)
            .map_err(failed)?
            .rows
            .into_iter()
            .filter_map(|r| r.into_iter().next().flatten())
            .collect())
    }

    fn mentions(&self, iri: &str) -> Result<bool, QueryError> {
        let q =
            format!("SELECT ?x WHERE {{ {{ <{iri}> ?p ?x }} UNION {{ ?x ?p <{iri}> }} }} LIMIT 1");
        Ok(!self.select(&q).map_err(failed)?.rows.is_empty())
    }

    fn pairs(&self, predicate: &str) -> Result<Vec<(Term, Term)>, QueryError> {
        let q = format!("SELECT DISTINCT ?s ?o WHERE {{ ?s <{predicate}> ?o }}");
        Ok(self
            .select(&q)
            .map_err(failed)?
            .rows
            .into_iter()
            .filter_map(|r| match (r.first(), r.get(1)) {
                (Some(Some(s)), Some(Some(o))) => Some((s.clone(), o.clone())),
                _ => None,
            })
            .collect())
    }
}

impl Lookup for QIndex {
    fn subjects(&self, predicate: &str, object: &Term) -> Result<Vec<Term>, QueryError> {
        let (Some(p), Some(o)) = (self.id(&Term::iri(predicate)), self.id(object)) else {
            return Ok(Vec::new());
        };
        let mut ids: Vec<u32> = self
            .range(Order::Pos, &[p, o])
            .map(|i| self.entry(Order::Pos, i)[2])
            .collect();
        ids.dedup();
        Ok(ids.into_iter().map(|id| self.term(id)).collect())
    }

    fn objects(&self, subject: &str, predicate: &str) -> Result<Vec<Term>, QueryError> {
        let (Some(s), Some(p)) = (self.id(&Term::iri(subject)), self.id(&Term::iri(predicate)))
        else {
            return Ok(Vec::new());
        };
        let mut ids: Vec<u32> = self
            .range(Order::Spo, &[s, p])
            .map(|i| self.entry(Order::Spo, i)[2])
            .collect();
        ids.dedup();
        Ok(ids.into_iter().map(|id| self.term(id)).collect())
    }

    fn mentions(&self, iri: &str) -> Result<bool, QueryError> {
        Ok(self
            .id(&Term::iri(iri))
            .is_some_and(|id| self.block(Order::Spo, id) > 0 || self.block(Order::Osp, id) > 0))
    }

    fn pairs(&self, predicate: &str) -> Result<Vec<(Term, Term)>, QueryError> {
        let Some(p) = self.id(&Term::iri(predicate)) else {
            return Ok(Vec::new());
        };
        let mut out: Vec<(u32, u32)> = self
            .range(Order::Pos, &[p])
            .map(|i| {
                let [s, _, o] = spo(Order::Pos, self.entry(Order::Pos, i));
                (s, o)
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        Ok(out
            .into_iter()
            .map(|(s, o)| (self.term(s), self.term(o)))
            .collect())
    }
}
