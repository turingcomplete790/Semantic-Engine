//! Query errors (spec 009 FR-027; data-model "QueryError"). Every error that has a place in the
//! query text carries its 1-based line and column.

use crate::graph::{GraphError, RebuildReason};
use crate::ErrorCode;
use serde_json::{json, Value};

/// A place in the query text, 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax,
    Unsupported {
        construct: String,
    },
    Unknown {
        name: String,
        why: String,
    },
    ScopeAmbiguous {
        performer: String,
    },
    NotRecursive {
        name: String,
    },
    TooLarge,
    PageExpired {
        generation: u64,
    },
    PageInvalid,
    NotBuilt,
    /// `damaged`, `otherFormat` or `otherServer`.
    NeedsRebuild {
        reason: String,
    },
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    pub kind: ErrorKind,
    pub message: String,
    pub pos: Option<Pos>,
}

impl QueryError {
    pub fn new(kind: ErrorKind, message: impl Into<String>, pos: Option<Pos>) -> QueryError {
        QueryError {
            kind,
            message: message.into(),
            pos,
        }
    }

    pub fn syntax(message: impl Into<String>, pos: Pos) -> QueryError {
        QueryError::new(ErrorKind::Syntax, message, Some(pos))
    }

    pub fn unsupported(construct: impl Into<String>, pos: Pos) -> QueryError {
        let construct = construct.into();
        let message = format!(
            "{construct} is not part of the query language (level {})",
            super::LANGUAGE_LEVEL
        );
        QueryError::new(ErrorKind::Unsupported { construct }, message, Some(pos))
    }

    pub fn unknown(name: impl Into<String>, why: impl Into<String>, pos: Pos) -> QueryError {
        let (name, why) = (name.into(), why.into());
        let message = format!("{name}: {why}");
        QueryError::new(ErrorKind::Unknown { name, why }, message, Some(pos))
    }

    pub fn internal(message: impl Into<String>) -> QueryError {
        QueryError::new(ErrorKind::Internal, message, None)
    }

    pub fn code(&self) -> ErrorCode {
        match self.kind {
            ErrorKind::Syntax => ErrorCode::QuerySyntax,
            ErrorKind::Unsupported { .. } => ErrorCode::QueryUnsupported,
            ErrorKind::Unknown { .. } => ErrorCode::QueryUnknown,
            ErrorKind::ScopeAmbiguous { .. } => ErrorCode::QueryScopeAmbiguous,
            ErrorKind::NotRecursive { .. } => ErrorCode::QueryNotRecursive,
            ErrorKind::TooLarge => ErrorCode::ResultTooLarge,
            ErrorKind::PageExpired { .. } => ErrorCode::PageExpired,
            ErrorKind::PageInvalid => ErrorCode::PageInvalid,
            ErrorKind::NotBuilt => ErrorCode::GraphNotBuilt,
            ErrorKind::NeedsRebuild { .. } => ErrorCode::GraphNeedsRebuild,
            ErrorKind::Internal => ErrorCode::Internal,
        }
    }

    /// The envelope's `error.details` (contracts/operations-1.4.md), or `None`.
    pub fn details(&self) -> Option<Value> {
        let mut d = match &self.kind {
            ErrorKind::Unsupported { construct } => json!({ "construct": construct }),
            ErrorKind::Unknown { name, .. } | ErrorKind::NotRecursive { name } => {
                json!({ "name": name })
            }
            ErrorKind::ScopeAmbiguous { performer } => json!({ "performer": performer }),
            ErrorKind::PageExpired { generation } => json!({ "generation": generation }),
            ErrorKind::NeedsRebuild { reason } => json!({ "reason": reason }),
            _ => json!({}),
        };
        if let Some(p) = self.pos {
            d["line"] = json!(p.line);
            d["column"] = json!(p.column);
        }
        match d.as_object() {
            Some(m) if m.is_empty() => None,
            _ => Some(d),
        }
    }
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.pos {
            Some(p) => write!(f, "{} (line {}, column {})", self.message, p.line, p.column),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for QueryError {}

impl From<GraphError> for QueryError {
    fn from(e: GraphError) -> QueryError {
        let rebuild = |reason: &str| {
            QueryError::new(
                ErrorKind::NeedsRebuild {
                    reason: reason.into(),
                },
                format!(
                    "{e}; run \"Update semantic graph\" or \"Build semantic graph\" to rebuild it"
                ),
                None,
            )
        };
        match &e {
            GraphError::NotBuilt | GraphError::NeedsRebuild(RebuildReason::Missing) => {
                QueryError::new(
                    ErrorKind::NotBuilt,
                    "the semantic graph has not been built yet; run \"Build semantic graph\" or \"Update semantic graph\"",
                    None,
                )
            }
            GraphError::NeedsRebuild(RebuildReason::Damaged { .. }) => rebuild("damaged"),
            GraphError::NeedsRebuild(RebuildReason::OtherFormat { .. }) => rebuild("otherFormat"),
            GraphError::NeedsRebuild(RebuildReason::OtherServer) => rebuild("otherServer"),
            _ => QueryError::internal(e.to_string()),
        }
    }
}
