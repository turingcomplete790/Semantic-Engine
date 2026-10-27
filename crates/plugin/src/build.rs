//! The "Build semantic graph" and "Update semantic graph" tasks (specs 007 and 008;
//! contracts/operations-1.2.md and 1.3.md). They wire Stash's input, the log and the store
//! folder to `engine::build::run` and `engine::build::update::update`.

use crate::envelope::Envelope;
use crate::input::PluginInput;
use crate::log::{Level, Logger};
use crate::relations::stash_error;
use engine::build::update::update;
use engine::build::{run, BuildError, BuildOptions, LogLevel};
use engine::graph::GraphStore;
use engine::server_id;
use engine::ErrorCode;
use serde::Serialize;
use stash_adapter::reader::LibraryReader;
use std::cell::RefCell;
use std::path::Path;

/// The graph store's folder under the plugin's own folder (spec 006).
pub const STORE_DIR: &str = "graph";

fn error_code(e: &BuildError) -> ErrorCode {
    match e {
        BuildError::NoServerId => ErrorCode::ServerIdNotGenerated,
        BuildError::BadServerId => ErrorCode::ServerIdInvalid,
        BuildError::Busy => ErrorCode::Busy,
        BuildError::Stash(e) => stash_error(e),
        BuildError::Store(_) => ErrorCode::Internal,
    }
}

/// What a graph task runs: a full build or an update, with the same arguments.
type Task<R> = fn(
    &dyn LibraryReader,
    &GraphStore,
    &str,
    &BuildOptions,
    &mut dyn FnMut(f64),
    &mut dyn FnMut(LogLevel, &str),
) -> Result<R, BuildError>;

/// Reads the server id, connects, opens the store, and runs `task`, with progress and log lines
/// sent to Stash as they happen.
fn graph_task<R: Serialize>(
    op: &str,
    task: Task<R>,
    input: &PluginInput,
    logger: &mut Logger,
) -> Envelope {
    let fail = |code: ErrorCode, message: &str, logger: &mut Logger| {
        logger.error(&format!("{op}: {message}"));
        Envelope::error(op, code, message, None)
    };
    let server = match crate::ops::settings_store(input).and_then(|s| server_id::read_server_id(&s))
    {
        Ok(id) => id,
        Err(e) => return fail(e.code(), &e.message(), logger),
    };
    let client = match stash_adapter::Client::from_server_connection(&input.server_connection) {
        Ok(c) => c,
        Err(e) => return fail(stash_error(&e), &e.to_string(), logger),
    };
    let store = GraphStore::new(
        Path::new(&input.server_connection.plugin_dir).join(STORE_DIR),
        &server,
    );
    // Both closures share the logger.
    let shared = RefCell::new(&mut *logger);
    let result = task(
        &client,
        &store,
        &server,
        &BuildOptions::default(),
        &mut |p| shared.borrow_mut().log(Level::Progress, &format!("{p:.2}")),
        &mut |level, m| {
            let level = match level {
                LogLevel::Info => Level::Info,
                LogLevel::Warning => Level::Warning,
            };
            shared.borrow_mut().log(level, m)
        },
    );
    match result {
        Ok(report) => Envelope::ok(
            op,
            serde_json::to_value(&report).expect("the report serializes"),
        ),
        Err(e) => fail(error_code(&e), &e.to_string(), logger),
    }
}

/// "Build semantic graph": a full rebuild (spec 007).
pub fn build_graph(input: &PluginInput, logger: &mut Logger) -> Envelope {
    graph_task("buildGraph", run, input, logger)
}

/// "Update semantic graph": what changed since the last build or update (spec 008).
pub fn update_graph(input: &PluginInput, logger: &mut Logger) -> Envelope {
    graph_task("updateGraph", update, input, logger)
}
