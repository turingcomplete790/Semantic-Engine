//! Queries over the published graph (spec 009; contracts/query-language.md). A query is
//! parsed, its names resolved against one generation, rewritten so that the documents' short
//! form joins within one scene, printed as SPARQL, and evaluated read-only:
//!
//! ```text
//! text ──parse──► Ast ──resolve──► Resolved ──rewrite──► Plan ──print──► SPARQL ──► Page
//! ```
//!
//! Nothing is evaluated until every step has succeeded, and every error with a place in the
//! text carries its position.

pub mod error;
pub mod lex;
pub mod lookup;
pub mod names;
pub mod page;
pub mod parse;
pub mod print;
pub mod rewrite;

pub use error::{ErrorKind, Pos, QueryError};
pub use page::{Continuation, Page, PageRequest, Value, PAGE_BYTES, PAGE_ROWS_MAX};

use page::check_key;

use crate::graph::{Description, GraphError, GraphStore, Term};
use crate::qindex::QIndex;
use lookup::Lookup;
use std::time::Duration;

/// The query language level this engine accepts (contracts/query-language.md).
pub const LANGUAGE_LEVEL: u32 = 1;
/// Variables the engine adds start with this; user variables may not.
pub const HIDDEN: &str = "__se";
/// A query whose evaluation passes this is stopped (research R7).
pub const QUERY_TIME_LIMIT: Duration = Duration::from_secs(10);

/// Runs one query, and returns one page of its answer (research R6). Without a continuation it
/// answers from the current generation; with one, from the continuation's generation, which
/// must still be kept.
pub fn run(store: &GraphStore, text: &str, req: &PageRequest) -> Result<Page, QueryError> {
    let ast = parse::parse(text)?;
    let folder = store.dir().parent().unwrap_or(store.dir());
    let invalid = || {
        QueryError::new(
            ErrorKind::PageInvalid,
            "the continuation is altered, or belongs to another query; send it back with the \
             query text it came with",
            None,
        )
    };
    let expired = |e: GraphError| match e {
        GraphError::GenerationGone { generation } => QueryError::new(
            ErrorKind::PageExpired { generation },
            format!(
                "generation {generation} of the graph is no longer kept; run the query again \
                 from its first page"
            ),
            None,
        ),
        other => QueryError::from(other),
    };
    // The generation that answers: the current one, or the continuation's.
    let (generation, offset, page_size, limit) = match &req.continuation {
        None => match store.describe() {
            Description::Ready(g) => (g, 0, req.page_size, req.limit),
            Description::NotBuilt => return Err(GraphError::NotBuilt.into()),
            Description::NeedsRebuild(r) => return Err(GraphError::NeedsRebuild(r).into()),
        },
        Some(c) => {
            let key = check_key(folder, false)
                .map_err(|e| QueryError::internal(format!("the check key: {e}")))?
                .ok_or_else(invalid)?;
            let c = Continuation::decode(c, text, &key).ok_or_else(invalid)?;
            let g = store.kept_generation(c.generation).map_err(expired)?;
            (g, c.offset, c.page_size, c.limit)
        }
    };
    // Its query index when it has one (research R12), otherwise the store itself.
    let mut note = None;
    let index = match QIndex::open(&crate::qindex::path(store.dir(), generation.number)) {
        Ok(i) => Some(i),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            note = Some(format!(
                "the query index of generation {} is unreadable ({e}); answered by the store",
                generation.number
            ));
            None
        }
    };
    let reader = match index {
        Some(_) => None,
        None => Some(store.reader_at(generation.number).map_err(expired)?),
    };
    let lookup: &dyn Lookup = match (&index, &reader) {
        (Some(i), _) => i,
        (None, Some(r)) => r,
        (None, None) => unreachable!("one of the two is open"),
    };
    let resolved = names::resolve(&ast, lookup, store.server_id())?;
    let plan = rewrite::rewrite(&resolved, &mut rewrite::hierarchy_expand(lookup))?;

    let rows_max = u64::from(page_size.unwrap_or(PAGE_ROWS_MAX).min(PAGE_ROWS_MAX));
    let take = match limit {
        Some(l) => rows_max.min(l.saturating_sub(offset)),
        None => rows_max,
    };
    // On the index, in evaluation order, stopping once the page is full; on the store, the
    // whole answer ordered here and sliced (research R4).
    let variables = plan.variables.clone();
    let skip = usize::try_from(offset).unwrap_or(usize::MAX);
    let mut fill = Fill {
        variables: &variables,
        take: take as usize,
        bindings: Vec::new(),
        bytes: 2,
        more: false,
        error: None,
    };
    match (&index, &reader) {
        (Some(index), _) => {
            let mut skipped = 0usize;
            crate::qindex::eval::each(index, &plan, &mut |row| {
                if skipped < skip {
                    skipped += 1;
                    return true;
                }
                fill.offer(row.iter().map(|id| Some(index.term(*id))).collect())
            });
        }
        (None, Some(reader)) => {
            let mut rows = reader
                .select(&print::print(&plan))
                .map_err(|e| QueryError::internal(format!("evaluation failed: {e}")))?
                .rows;
            rows.sort_unstable_by(|a, b| page::compare_rows(a, b));
            for row in rows.into_iter().skip(skip) {
                if !fill.offer(row) {
                    break;
                }
            }
        }
        (None, None) => unreachable!("one of the two is open"),
    }
    if let Some(e) = fill.error {
        return Err(e);
    }
    let (bindings, more) = (fill.bindings, fill.more);
    let next = offset + bindings.len() as u64;
    let continuation = if more && limit.is_none_or(|l| next < l) {
        let key = check_key(folder, true)
            .map_err(|e| QueryError::internal(format!("the check key: {e}")))?
            .ok_or_else(|| QueryError::internal("the check key could not be read"))?;
        Some(Continuation::new(generation.number, next, page_size, limit, text, &key).encode())
    } else {
        None
    };
    Ok(Page {
        variables,
        bindings,
        offset,
        continuation,
        generation,
        note,
    })
}

/// A page being filled: bindings until `take`, or until the next would pass [`PAGE_BYTES`].
struct Fill<'v> {
    variables: &'v [String],
    take: usize,
    bindings: Vec<Vec<Value>>,
    bytes: usize,
    /// A binding was left over: another page follows.
    more: bool,
    error: Option<QueryError>,
}

impl Fill<'_> {
    /// Offers the next binding; false when the page is full or failed.
    fn offer(&mut self, row: Vec<Option<Term>>) -> bool {
        if self.bindings.len() == self.take {
            self.more = true;
            return false;
        }
        let values: Result<Vec<Value>, QueryError> = row
            .iter()
            .map(|t| {
                t.as_ref()
                    .map(Value::from_term)
                    .ok_or_else(|| QueryError::internal("a projected variable is unbound"))
            })
            .collect();
        let values = match values {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e);
                return false;
            }
        };
        let size = page::binding_bytes(self.variables, &values);
        if self.bytes + size > PAGE_BYTES {
            if self.bindings.is_empty() {
                self.error = Some(QueryError::new(
                    ErrorKind::TooLarge,
                    "one binding alone is larger than a page (1 MiB)",
                    None,
                ));
            } else {
                self.more = true;
            }
            return false;
        }
        self.bytes += size;
        self.bindings.push(values);
        true
    }
}
