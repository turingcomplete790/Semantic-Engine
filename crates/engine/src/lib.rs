//! Semantic engine library. It holds the engine's logic and calls Stash only through
//! `stash-adapter` (constitution Principle V).

pub mod errors;
pub mod versions;

pub use errors::ErrorCode;
pub use versions::{versions, Versions, CONTRACT_VERSION, STASH_MINIMUM_VERSION};
