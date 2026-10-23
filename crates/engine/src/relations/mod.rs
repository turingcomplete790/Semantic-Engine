//! Relations, role and relation hierarchies, and auto-apply (spec 004).
//!
//! Pure functions over data read from Stash: definitions from tags ([`defs`]), the broader and
//! narrower closures ([`hierarchy`]), the matches for one scene or image ([`evaluate`]), the
//! minimal change to write ([`diff`]), and explanations ([`explain`]). All Stash I/O stays in
//! `stash-adapter` (constitution Principle V).
//!
//! Recursive search (spec 004 US3): Phase 3's search operation expands every Relation and Role
//! criterion with [`hierarchy::Hierarchy::expand_criterion`], so "recursive" means the same thing
//! in every client. Spec 004's SC-004 and SC-007 are verified there.

pub mod defs;
pub mod diff;
pub mod evaluate;
pub mod explain;
pub mod hierarchy;

use defs::{Definitions, RelationDef};
use hierarchy::Hierarchy;
use serde_json::{json, Value};
use stash_adapter::relations::TagRecord;
use std::collections::BTreeSet;

/// Definitions and both hierarchies, built once from the tags read from Stash.
pub struct Model {
    pub defs: Definitions,
    pub hierarchy: Hierarchy,
}

impl Model {
    pub fn from_tags(tags: &[TagRecord]) -> Model {
        let mut defs = Definitions::from_tags(tags);
        let hierarchy = Hierarchy::build(&mut defs);
        Model { defs, hierarchy }
    }
}

/// The tag a `relations` request named is not a Role or Relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownTag(pub String);

/// The `relations` operation's result (contracts/operations-1.1.md): every Role and Relation
/// with its broader and narrower terms, and every problem found. With `tag`, only that Role or
/// Relation and its broader and narrower terms are listed. `recursive` makes `narrower` list
/// every depth instead of direct children.
pub fn describe(model: &Model, tag: Option<&str>, recursive: bool) -> Result<Value, UnknownTag> {
    let (d, h) = (&model.defs, &model.hierarchy);
    let keep: Option<BTreeSet<String>> = match tag {
        None => None,
        Some(t) if d.is_role(t) || d.is_relation(t) => {
            let mut s = h.broader_closure(t);
            s.extend(h.narrower(t, recursive));
            s.insert(t.to_string());
            Some(s)
        }
        Some(t) => return Err(UnknownTag(t.to_string())),
    };
    let wanted = |id: &str| keep.as_ref().is_none_or(|k| k.contains(id));
    let name = |id: &str| d.tags.get(id).map(|t| t.name.clone()).unwrap_or_default();

    let roles: Vec<Value> = d
        .roles
        .iter()
        .filter(|r| wanted(r))
        .map(|r| {
            json!({
                "id": r,
                "name": name(r),
                "broader": h.broader(r),
                "narrower": h.narrower(r, recursive),
            })
        })
        .collect();
    let relations: Vec<Value> = d
        .relations
        .values()
        .filter(|r| wanted(&r.id))
        .map(|r: &RelationDef| {
            json!({
                "id": r.id,
                "name": r.name,
                "ends": r.ends.as_ref().map(|(a, b)| json!({ "a": a, "b": b })),
                "broader": h.broader(&r.id),
                "narrower": h.narrower(&r.id, recursive),
                "valid": r.valid,
                "rules": r.rules,
            })
        })
        .collect();
    Ok(json!({ "roles": roles, "relations": relations, "problems": d.problems }))
}
