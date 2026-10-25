//! The oldest Stash release the engine supports, and how a server's version is compared with it
//! (spec 002 FR-017; spec 005 FR-017). The engine re-exports these.

/// Oldest Stash release this engine supports; the schema in `schemas/` is pinned to it.
pub const MINIMUM_STASH_VERSION: &str = "v0.31.1";

/// Parses `vMAJOR.MINOR.PATCH` (an optional leading `v`, ignoring any `-suffix`).
pub fn parse_stash_version(s: &str) -> Option<(u64, u64, u64)> {
    let core = s.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

/// True when the server version is at least the supported minimum. An unparseable version is
/// not supported.
pub fn stash_supported(server: &str) -> bool {
    match (
        parse_stash_version(server),
        parse_stash_version(MINIMUM_STASH_VERSION),
    ) {
        (Some(s), Some(min)) => s >= min,
        _ => false,
    }
}
