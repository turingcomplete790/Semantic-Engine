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
    Internal,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 9] = [
        ErrorCode::InvalidInput,
        ErrorCode::UnknownOperation,
        ErrorCode::StashUnreachable,
        ErrorCode::StashTooOld,
        ErrorCode::ServerIdNotGenerated,
        ErrorCode::ServerIdInvalid,
        ErrorCode::NotFound,
        ErrorCode::Busy,
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
            ErrorCode::Internal => "INTERNAL",
        }
    }
}
