//! `query`: open the stored graph and run one of five fixed Phase 1 queries (research R5).
//!
//! Arguments: `store` (folder under `store/`, default `test`), `queryId` 1–5, `option`
//! (`A` live read-only, `B` published snapshot, the default, `C` under the writer lock), and
//! `waitMs` (option C's lock wait, default 30000).

use super::{fail, Ctx};
use crate::store::{self, LockMode, StoreError};
use crate::synth::{tag_id_for, SERVER_ID};
use engine::ErrorCode;
use oxigraph::model::{NamedNodeRef, Term};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use plugin::envelope::Envelope;
use plugin::log::Logger;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Marker subject and predicates a `write` puts first and last, so a reader can tell whether it
/// saw a write part-way (mixed generation).
pub const MARK: &str = "urn:spike:write";
pub const STARTED: &str = "urn:spike:started";
pub const ENDED: &str = "urn:spike:ended";

fn tag(class: &str) -> String {
    format!(
        "<urn:semantic-stash:{SERVER_ID}:tag:{}>",
        tag_id_for(class, 0)
    )
}

/// The five queries, from one to five triple patterns, each capped at 100 rows (first results).
pub fn sparql(id: u64) -> Option<String> {
    let p = "PREFIX sem: <urn:semantic-stash:vocab:>\n";
    let (role, action, tr) = (tag("Role"), tag("Action"), tag("Trait"));
    let body = match id {
        // 1 pattern: every scene.
        1 => "SELECT ?scene WHERE { ?scene a sem:Scene } LIMIT 100".to_string(),
        // 2 patterns: media where someone has role 0.
        2 => format!("SELECT ?media WHERE {{ ?r sem:ofTag {role} . ?r sem:inMedia ?media }} LIMIT 100"),
        // 3 patterns: performers with role 0, and where.
        3 => format!(
            "SELECT ?p ?media WHERE {{ ?p sem:hasRole ?r . ?r sem:ofTag {role} . ?r sem:inMedia ?media }} LIMIT 100"
        ),
        // 4 patterns: scenes with role 0 and action 0 in the same scene (scene scoping).
        4 => format!(
            "SELECT ?scene WHERE {{ ?r sem:ofTag {role} . ?r sem:inMedia ?scene . ?a sem:ofTag {action} . ?a sem:inMedia ?scene }} LIMIT 100"
        ),
        // 5 patterns: scenes where a performer with trait 0 has role 0.
        5 => format!(
            "SELECT ?p ?scene WHERE {{ ?p sem:hasTrait {tr} . ?p sem:hasRole ?r . ?r sem:ofTag {role} . ?r sem:inMedia ?scene . ?scene a sem:Scene }} LIMIT 100"
        ),
        _ => return None,
    };
    Some(format!("{p}{body}"))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The highest generation a write started and the highest it ended, from the markers.
pub fn markers(store: &Store) -> (i64, i64) {
    let max = |pred: &str| {
        store
            .quads_for_pattern(
                Some(NamedNodeRef::new_unchecked(MARK).into()),
                Some(NamedNodeRef::new_unchecked(pred)),
                None,
                None,
            )
            .filter_map(Result::ok)
            .filter_map(|q| match q.object {
                Term::Literal(l) => l.value().parse::<i64>().ok(),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    };
    (max(STARTED), max(ENDED))
}

fn term(t: &Term) -> Value {
    Value::String(match t {
        Term::NamedNode(n) => n.as_str().to_string(),
        Term::Literal(l) => l.value().to_string(),
        other => other.to_string(),
    })
}

pub fn op(ctx: &Ctx, _: &mut Logger) -> Envelope {
    let name = ctx.arg_str("store").unwrap_or("test");
    let qid = ctx.arg_u64("queryId").unwrap_or(1);
    let option = ctx.arg_str("option").unwrap_or("B");
    let Some(text) = sparql(qid) else {
        return fail(
            "query",
            ErrorCode::InvalidInput,
            format!("queryId must be 1 to 5, got {qid}"),
        );
    };
    let dir = ctx.store_dir.join(name);

    let t0 = Instant::now();
    let mut lock_ms = 0.0;
    let mut _lock = None;
    let opened = match option {
        "A" => store::open_live_read_only(&dir).map(|s| (None, s)),
        "B" => store::open_current_read_only(&dir).map(|(g, s)| (Some(g), s)),
        "C" => {
            let wait = Duration::from_millis(ctx.arg_u64("waitMs").unwrap_or(30_000));
            match store::lock_writer(&dir, LockMode::Wait(wait)) {
                Ok(l) => {
                    lock_ms = ms(t0.elapsed());
                    _lock = Some(l);
                    Store::open(store::work_dir(&dir))
                        .map(|s| (None, s))
                        .map_err(|e| StoreError::Storage(e.to_string()))
                }
                Err(e) => Err(e),
            }
        }
        other => {
            return fail(
                "query",
                ErrorCode::InvalidInput,
                format!("option must be A, B, or C, got {other:?}"),
            )
        }
    };
    let (generation, store) = match opened {
        Ok(x) => x,
        // A missing or damaged store is an error, never an empty answer (Principle IV).
        Err(e) => return fail("query", ErrorCode::Internal, format!("store {name:?}: {e}")),
    };
    let store_open = ms(t0.elapsed()) - lock_ms;

    let t1 = Instant::now();
    let mut rows = Vec::new();
    match store.query(text.as_str()) {
        Ok(QueryResults::Solutions(solutions)) => {
            for s in solutions {
                match s {
                    Ok(s) => rows.push(
                        s.values()
                            .iter()
                            .map(|v| v.as_ref().map_or(Value::Null, term))
                            .collect::<Vec<_>>(),
                    ),
                    Err(e) => {
                        return fail("query", ErrorCode::Internal, format!("query {qid}: {e}"))
                    }
                }
            }
        }
        Ok(_) => return fail("query", ErrorCode::Internal, "not a SELECT result"),
        Err(e) => return fail("query", ErrorCode::Internal, format!("query {qid}: {e}")),
    }
    let query_ms = ms(t1.elapsed());
    let (started, ended) = markers(&store);

    let t2 = Instant::now();
    let mut result = json!({
        "queryId": qid,
        "option": option,
        "generation": generation,
        "rowCount": rows.len(),
        "rows": rows,
        "consistency": { "started": started, "ended": ended, "mixed": started > ended },
    });
    // Serialising the rows is what Stash will carry; time it on the real value.
    let serialise_ms = {
        let _ = serde_json::to_vec(&result["rows"]);
        ms(t2.elapsed())
    };
    result["timings"] = json!({
        "lock": lock_ms,
        "storeOpen": store_open,
        "query": query_ms,
        "serialise": serialise_ms,
        "total": ms(t0.elapsed()),
    });
    Envelope::ok("query", result)
}
