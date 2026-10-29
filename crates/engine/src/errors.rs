//! Error codes of the operation contract (contracts/operations.md, api/operations.json).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidInput,
    UnknownOperation,
    StashUnreachable,
    StashTooOld,
    ServerIdNotGenerated,
    ServerIdInvalid,
    /// A scene, image, or tag named in the arguments does not exist (since 1.1.0).
    NotFound,
    /// Another build, or another writer, holds the graph store (since 1.2.0).
    Busy,
    /// The query text does not parse (since 1.4.0).
    QuerySyntax,
    /// The query uses a construct outside the query language (since 1.4.0).
    QueryUnsupported,
    /// The query names an unknown prefix, term, tag or item (since 1.4.0).
    QueryUnknown,
    /// A short-form scene-scoped fact has no single scene (since 1.4.0).
    QueryScopeAmbiguous,
    /// A recursive mark on a tag that is neither a Role nor a Relation (since 1.4.0).
    QueryNotRecursive,
    /// The query ran past the time limit (since 1.4.0).
    QueryTimeout,
    /// One binding alone is larger than a page (since 1.4.0).
    ResultTooLarge,
    /// A continuation's generation is no longer kept (since 1.4.0).
    PageExpired,
    /// A continuation is altered or belongs to another query (since 1.4.0).
    PageInvalid,
    /// No graph has been published yet (since 1.4.0).
    GraphNotBuilt,
    /// The graph store needs a rebuild (since 1.4.0).
    GraphNeedsRebuild,
    /// The client was written for another major contract version (since 1.4.0).
    ContractMismatch,
    Internal,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 21] = [
        ErrorCode::InvalidInput,
        ErrorCode::UnknownOperation,
        ErrorCode::StashUnreachable,
        ErrorCode::StashTooOld,
        ErrorCode::ServerIdNotGenerated,
        ErrorCode::ServerIdInvalid,
        ErrorCode::NotFound,
        ErrorCode::Busy,
        ErrorCode::QuerySyntax,
        ErrorCode::QueryUnsupported,
        ErrorCode::QueryUnknown,
        ErrorCode::QueryScopeAmbiguous,
        ErrorCode::QueryNotRecursive,
        ErrorCode::QueryTimeout,
        ErrorCode::ResultTooLarge,
        ErrorCode::PageExpired,
        ErrorCode::PageInvalid,
        ErrorCode::GraphNotBuilt,
        ErrorCode::GraphNeedsRebuild,
        ErrorCode::ContractMismatch,
        ErrorCode::Internal,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::InvalidInput => "INVALID_INPUT",
            ErrorCode::UnknownOperation => "UNKNOWN_OPERATION",
            ErrorCode::StashUnreachable => "STASH_UNREACHABLE",
            ErrorCode::StashTooOld => "STASH_TOO_OLD",
            ErrorCode::ServerIdNotGenerated => "SERVER_ID_NOT_GENERATED",
            ErrorCode::ServerIdInvalid => "SERVER_ID_INVALID",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Busy => "BUSY",
            ErrorCode::QuerySyntax => "QUERY_SYNTAX",
            ErrorCode::QueryUnsupported => "QUERY_UNSUPPORTED",
            ErrorCode::QueryUnknown => "QUERY_UNKNOWN",
            ErrorCode::QueryScopeAmbiguous => "QUERY_SCOPE_AMBIGUOUS",
            ErrorCode::QueryNotRecursive => "QUERY_NOT_RECURSIVE",
            ErrorCode::QueryTimeout => "QUERY_TIMEOUT",
            ErrorCode::ResultTooLarge => "RESULT_TOO_LARGE",
            ErrorCode::PageExpired => "PAGE_EXPIRED",
            ErrorCode::PageInvalid => "PAGE_INVALID",
            ErrorCode::GraphNotBuilt => "GRAPH_NOT_BUILT",
            ErrorCode::GraphNeedsRebuild => "GRAPH_NEEDS_REBUILD",
            ErrorCode::ContractMismatch => "CONTRACT_MISMATCH",
            ErrorCode::Internal => "INTERNAL",
        }
    }
}
