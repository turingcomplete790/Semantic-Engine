//! Reading one published generation (spec 006 FR-010 to FR-013; research R7). A reader opens
//! the current generation read-only and answers from it for its whole life, even if a newer
//! one is published meanwhile.

use super::error::GraphError;
use super::layout::Generation;
use super::term::{from_ox_term, to_ox_quad, Fact, Solutions};
use oxigraph::sparql::{Query, QueryResults, Update};
use oxigraph::store::Store;

pub struct Reader {
    pub(crate) generation: Generation,
    pub(crate) store: Store,
}

/// Reads `line:column` from a SPARQL syntax error's text, which starts `error at {l}:{c}:`.
fn position(message: &str) -> (Option<u32>, Option<u32>) {
    let Some(rest) = message.split("error at ").nth(1) else {
        return (None, None);
    };
    let mut it = rest.split(|c: char| c == ':' || c.is_whitespace());
    let line = it.next().and_then(|s| s.parse().ok());
    let column = it.next().and_then(|s| s.parse().ok());
    match (line, column) {
        (Some(l), Some(c)) => (Some(l), Some(c)),
        _ => (None, None),
    }
}

fn invalid(message: impl Into<String>) -> GraphError {
    GraphError::InvalidQuery {
        message: message.into(),
        line: None,
        column: None,
    }
}

impl Reader {
    pub fn generation(&self) -> &Generation {
        &self.generation
    }

    /// Runs a SPARQL `SELECT` query. Every other query form, and every update, is refused.
    pub fn select(&self, query: &str) -> Result<Solutions, GraphError> {
        let parsed = match Query::parse(query, None) {
            Ok(q) => q,
            Err(e) => {
                if Update::parse(query, None).is_ok() {
                    return Err(invalid(
                        "only SELECT queries are allowed; this is an update",
                    ));
                }
                let message = e.to_string();
                let (line, column) = position(&message);
                return Err(GraphError::InvalidQuery {
                    message,
                    line,
                    column,
                });
            }
        };
        // The parsed query prints in canonical form: an optional `BASE <iri>`, then the form.
        let text = parsed.to_string();
        let form = text
            .split_whitespace()
            .find(|w| !w.eq_ignore_ascii_case("BASE") && !w.starts_with('<'))
            .unwrap_or("")
            .to_ascii_uppercase();
        if form != "SELECT" {
            return Err(invalid(format!(
                "only SELECT queries are allowed; this is {form}"
            )));
        }
        // Format 2 keeps each item's facts in its own graph (spec 008 R1); a query's default
        // graph is the union of them all, so answers do not depend on where a fact lives.
        let mut parsed = parsed;
        parsed.dataset_mut().set_default_graph_as_union();
        match self.store.query(parsed).map_err(GraphError::internal)? {
            QueryResults::Solutions(solutions) => {
                let variables: Vec<String> = solutions
                    .variables()
                    .iter()
                    .map(|v| v.as_str().to_string())
                    .collect();
                let mut rows = Vec::new();
                for s in solutions {
                    let s = s.map_err(GraphError::internal)?;
                    rows.push(
                        variables
                            .iter()
                            .map(|v| s.get(v.as_str()).map(from_ox_term))
                            .collect(),
                    );
                }
                Ok(Solutions { variables, rows })
            }
            _ => Err(invalid("only SELECT queries are allowed")),
        }
    }

    /// True when the generation holds `fact` in any graph (for tests and checks).
    pub fn contains(&self, fact: &Fact) -> Result<bool, GraphError> {
        let q = to_ox_quad(fact)?;
        match self
            .store
            .quads_for_pattern(
                Some(q.subject.as_ref()),
                Some(q.predicate.as_ref()),
                Some(q.object.as_ref()),
                None,
            )
            .next()
        {
            Some(found) => found.map(|_| true).map_err(GraphError::internal),
            None => Ok(false),
        }
    }
}
