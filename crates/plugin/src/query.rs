//! The `query` operation (spec 009; contracts/operations-1.4.md). It reads only the plugin's own
//! store, never Stash, and stops a query that runs past the time limit (research R7).

use crate::build::STORE_DIR;
use crate::envelope::{stash_output, Envelope};
use crate::input::PluginInput;
use crate::journal::Journal;
use crate::log::Logger;
use engine::graph::GraphStore;
use engine::query::{self, PageRequest, QueryError, PAGE_ROWS_MAX, QUERY_TIME_LIMIT};
use engine::{ErrorCode, CONTRACT_VERSION};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const OP: &str = "query";

/// Lowers the time limit in tests only (spec 009 T018).
const TIME_LIMIT_VAR: &str = "SEMANTIC_ENGINE_QUERY_TIME_LIMIT_MS";

fn invalid(message: &str) -> Box<Envelope> {
    Box::new(Envelope::error(OP, ErrorCode::InvalidInput, message, None))
}

fn query_error(e: &QueryError) -> Envelope {
    Envelope::error(OP, e.code(), &e.message, e.details())
}

fn time_limit() -> Duration {
    std::env::var(TIME_LIMIT_VAR)
        .ok()
        .and_then(|v| v.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(QUERY_TIME_LIMIT)
}

/// The MAJOR of `MAJOR.MINOR.PATCH`.
fn major(version: &str) -> Option<u64> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
        return None;
    }
    parts[0].parse().ok()
}

/// An optional whole-number argument within `range`.
fn number(
    input: &PluginInput,
    name: &str,
    range: std::ops::RangeInclusive<u64>,
) -> Result<Option<u64>, Box<Envelope>> {
    match input.args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_u64() {
            Some(n) if range.contains(&n) => Ok(Some(n)),
            _ => Err(invalid(&format!(
                "{name} must be a whole number from {} to {}",
                range.start(),
                range.end()
            ))),
        },
    }
}

/// Reads and checks the arguments: the query text and the page request.
fn arguments(input: &PluginInput) -> Result<(String, PageRequest), Box<Envelope>> {
    let text = match input.args.get("query") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        _ => return Err(invalid("query is required: the query text, as a string")),
    };
    let page_size = number(input, "pageSize", 1..=PAGE_ROWS_MAX as u64)?.map(|n| n as u32);
    let limit = number(input, "limit", 1..=u64::MAX)?;
    let continuation = match input.args.get("continuation") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            return Err(invalid(
                "continuation must be the string the last page gave",
            ))
        }
    };
    match input.args.get("contract") {
        None | Some(Value::Null) => {}
        Some(Value::String(c)) => match major(c) {
            None => return Err(invalid("contract must be a version, MAJOR.MINOR.PATCH")),
            Some(m) if Some(m) != major(CONTRACT_VERSION) => {
                return Err(Box::new(Envelope::error(
                    OP,
                    ErrorCode::ContractMismatch,
                    &format!(
                        "this engine speaks contract {CONTRACT_VERSION}; a client written for {c} \
                         needs an engine of the same major version"
                    ),
                    Some(json!({ "engine": CONTRACT_VERSION, "client": c })),
                )))
            }
            Some(_) => {}
        },
        Some(_) => return Err(invalid("contract must be a version, MAJOR.MINOR.PATCH")),
    }
    Ok((
        text,
        PageRequest {
            page_size,
            limit,
            continuation,
        },
    ))
}

/// Writes the time-limit error as this call's whole output and ends the process. Oxigraph
/// cannot cancel a running query, and the process exists only for this call (research R7).
///
/// It ends with `_exit`, not `std::process::exit`: `exit` runs RocksDB's C++ static destructors
/// while the query thread is still inside RocksDB, and the process then crashes (exit 139) after
/// its output is written. `_exit` ends the process at once; stdout is flushed first, and a
/// reader holds no lock or file that needs closing (spec 006 G1, G2).
fn stop_at_time_limit(limit: Duration) -> ! {
    let env = Envelope::error(
        OP,
        ErrorCode::QueryTimeout,
        &format!(
            "the query ran past the time limit of {:.1} s and was stopped; narrow it, or ask for \
             fewer bindings",
            limit.as_secs_f64()
        ),
        None,
    );
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{}", stash_output(env));
    let _ = out.flush();
    // SAFETY: `_exit` takes no pointers and never returns; everything this call owes Stash has
    // been written and flushed above.
    unsafe { libc::_exit(0) }
}

pub fn query(input: &PluginInput, logger: &mut Logger) -> Envelope {
    let started = Instant::now();
    let (text, req) = match arguments(input) {
        Ok(a) => a,
        Err(env) => return *env,
    };
    let dir = Path::new(&input.server_connection.plugin_dir).join(STORE_DIR);
    let store = match GraphStore::stamped(&dir) {
        Ok(s) => s,
        Err(e) => return query_error(&QueryError::from(e)),
    };

    let limit = time_limit();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(query::run(&store, &text, &req));
    });
    let page = match rx.recv_timeout(limit) {
        Ok(Ok(page)) => {
            if let Some(note) = &page.note {
                logger.log(crate::log::Level::Warning, &format!("query: {note}"));
            }
            page
        }
        Ok(Err(e)) => return query_error(&e),
        Err(mpsc::RecvTimeoutError::Timeout) => stop_at_time_limit(limit),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            return Envelope::error(
                OP,
                ErrorCode::Internal,
                "the query stopped unexpectedly",
                None,
            )
        }
    };
    Envelope::ok(
        OP,
        json!({
            "variables": page.variables,
            "bindings": page.bindings_json(),
            "page": {
                "count": page.bindings.len(),
                "offset": page.offset,
                "continuation": page.continuation,
            },
            "graph": {
                "generation": page.generation.number,
                "publishedAt": page.generation.published_at,
                "pending": Journal::new(&input.server_connection.plugin_dir).graph_pending(),
            },
            "seconds": started.elapsed().as_secs_f64(),
        }),
    )
}
