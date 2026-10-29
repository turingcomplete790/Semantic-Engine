//! Store failures and health (spec 006 FR-015, FR-019; data-model "Health", "GraphError").
//! No message includes a server id or a credential.

use serde::Serialize;

/// Why a store must be rebuilt before it can be used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "camelCase")]
pub enum RebuildReason {
    /// No store folder, or no stamp in it.
    Missing,
    /// The stamp, `current`, or the current generation could not be read.
    Damaged { detail: String },
    /// Written by another store format.
    OtherFormat { found: u32, expected: u32 },
    /// Stamped for another Stash server.
    OtherServer,
}

impl std::fmt::Display for RebuildReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RebuildReason::Missing => write!(f, "missing"),
            RebuildReason::Damaged { detail } => write!(f, "damaged: {detail}"),
            RebuildReason::OtherFormat { found, expected } => {
                write!(f, "store format {found}, this engine uses {expected}")
            }
            RebuildReason::OtherServer => write!(f, "it belongs to another Stash server"),
        }
    }
}

/// Is the store usable?
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Health {
    Ready {
        generation: u64,
    },
    /// Stamped, but nothing published yet.
    NotBuilt,
    NeedsRebuild(RebuildReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// Another writer holds the lock.
    Busy,
    /// Nothing has been published yet; distinct from an empty graph.
    NotBuilt,
    NeedsRebuild(RebuildReason),
    InvalidQuery {
        message: String,
        line: Option<u32>,
        column: Option<u32>,
    },
    InvalidFact {
        message: String,
    },
    /// The plugin has no server id yet, so the store cannot be stamped or checked.
    NoServerId,
    /// The generation asked for is not kept (G7), or was never published (spec 009 R6).
    GenerationGone {
        generation: u64,
    },
    Io {
        message: String,
    },
    Internal {
        message: String,
    },
}

impl GraphError {
    /// A short name of the kind, for logs.
    pub fn kind(&self) -> &'static str {
        match self {
            GraphError::Busy => "busy",
            GraphError::NotBuilt => "not built",
            GraphError::NeedsRebuild(_) => "needs rebuild",
            GraphError::InvalidQuery { .. } => "invalid query",
            GraphError::InvalidFact { .. } => "invalid fact",
            GraphError::NoServerId => "no server id",
            GraphError::GenerationGone { .. } => "generation gone",
            GraphError::Io { .. } => "io",
            GraphError::Internal { .. } => "internal",
        }
    }

    pub(crate) fn io(e: impl std::fmt::Display) -> GraphError {
        GraphError::Io {
            message: e.to_string(),
        }
    }

    pub(crate) fn internal(e: impl std::fmt::Display) -> GraphError {
        GraphError::Internal {
            message: e.to_string(),
        }
    }
}

impl From<std::io::Error> for GraphError {
    fn from(e: std::io::Error) -> GraphError {
        GraphError::io(e)
    }
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::Busy => write!(f, "another writer is updating the graph store"),
            GraphError::NotBuilt => write!(f, "the graph has not been built yet"),
            GraphError::NeedsRebuild(r) => write!(f, "the graph store needs a rebuild: {r}"),
            GraphError::InvalidQuery {
                message,
                line,
                column,
            } => match (line, column) {
                (Some(l), Some(c)) => write!(f, "invalid query at {l}:{c}: {message}"),
                _ => write!(f, "invalid query: {message}"),
            },
            GraphError::InvalidFact { message } => write!(f, "invalid fact: {message}"),
            GraphError::NoServerId => write!(
                f,
                "this Stash server has no identifier yet; run \"Generate server identifier\""
            ),
            GraphError::GenerationGone { generation } => {
                write!(f, "generation {generation} is no longer kept")
            }
            GraphError::Io { message } => write!(f, "graph store file error: {message}"),
            GraphError::Internal { message } => write!(f, "graph store error: {message}"),
        }
    }
}

impl std::error::Error for GraphError {}
