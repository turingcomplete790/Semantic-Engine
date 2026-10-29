//! Semantic engine library. It holds the engine's logic and calls Stash only through
//! `stash-adapter` (constitution Principle V).

pub mod build;
pub mod errors;
pub mod graph;
pub mod qindex;
pub mod query;
pub mod recommend;
pub mod relations;
pub mod server_id;
pub mod versions;

pub use errors::ErrorCode;
pub use versions::{versions, Versions, CONTRACT_VERSION, STASH_MINIMUM_VERSION};
