//! Is the store usable, and if not, why (spec 006 FR-014, FR-015; research R4)? The checks
//! run in a fixed order, and each costs at most one small file read, apart from opening the
//! generation, which a read does anyway.

use super::error::{GraphError, Health, RebuildReason};
use super::layout::{self, Generation};
use super::STORE_FORMAT;
use oxigraph::store::Store;
use std::path::Path;

/// What a reader needs: the current generation and the store opened on it.
pub(crate) enum Inspection {
    Ready(Generation, Store),
    NotBuilt,
    NeedsRebuild(RebuildReason),
}

fn damaged(detail: impl Into<String>) -> Inspection {
    Inspection::NeedsRebuild(RebuildReason::Damaged {
        detail: detail.into(),
    })
}

/// The stamp checks, steps 1 to 4. `None` when the stamp is good.
pub(crate) fn stamp_problem(dir: &Path, server_id: &str) -> Option<RebuildReason> {
    match layout::read_stamp(dir) {
        Ok(None) => Some(RebuildReason::Missing),
        Err(detail) => Some(RebuildReason::Damaged { detail }),
        Ok(Some(s)) if s.format != STORE_FORMAT => Some(RebuildReason::OtherFormat {
            found: s.format,
            expected: STORE_FORMAT,
        }),
        Ok(Some(s)) if s.server_id != server_id => Some(RebuildReason::OtherServer),
        Ok(Some(_)) => None,
    }
}

/// Runs every check, steps 1 to 7, and opens the current generation read-only when they pass.
pub(crate) fn inspect(dir: &Path, server_id: &str) -> Inspection {
    if let Some(r) = stamp_problem(dir, server_id) {
        return Inspection::NeedsRebuild(r);
    }
    let n = match layout::read_current(dir) {
        Ok(None) => return Inspection::NotBuilt,
        Ok(Some(n)) => n,
        Err(detail) => return damaged(detail),
    };
    let path = layout::generation_dir(dir, n);
    if !path.is_dir() {
        return damaged(format!("generation {n} is missing from snapshots/"));
    }
    let generation = match layout::read_generation(dir, n) {
        Ok(g) => g,
        Err(detail) => return damaged(detail),
    };
    match Store::open_read_only(&path) {
        Ok(store) => Inspection::Ready(generation, store),
        Err(e) => damaged(format!("generation {n}: {e}")),
    }
}

pub(crate) fn health(dir: &Path, server_id: &str) -> Result<Health, GraphError> {
    if server_id.is_empty() {
        return Err(GraphError::NoServerId);
    }
    Ok(match inspect(dir, server_id) {
        Inspection::Ready(g, _) => Health::Ready {
            generation: g.number,
        },
        Inspection::NotBuilt => Health::NotBuilt,
        Inspection::NeedsRebuild(r) => Health::NeedsRebuild(r),
    })
}
