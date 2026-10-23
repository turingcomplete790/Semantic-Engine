//! Which relations apply to one scene or image (spec 004 FR-006 to FR-008; research R3, R5).
//!
//! For every ordered pair of different performers and every rule of every valid relation, the
//! relation applies when performer a meets end A's condition, performer b meets end B's, and an
//! action pairing joins them (holding the same Action without a pairing does not link them).
//!
//! Role conditions read the roles held in the item. Evaluation starts from the roles set by hand
//! (the stored automatic roles are left out, so a relation can never keep itself alive), then
//! adds the end roles of each pass's matches until nothing changes, at most [`MAX_PASSES`] times.

use super::defs::{json_field, Condition, Rule};
use super::Model;
use serde::Serialize;
use serde_json::{Map, Value};
use stash_adapter::relations::ItemRecord;
use std::collections::{BTreeMap, BTreeSet};

/// The most evaluation passes per item; a chain still changing after this is not written.
pub const MAX_PASSES: usize = 4;

/// Performer id to the tag ids it carries (its Labels among them).
pub type Labels = BTreeMap<String, BTreeSet<String>>;
/// Performer id to role tag ids.
pub type Roles = BTreeMap<String, BTreeSet<String>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Match {
    pub relation: String,
    pub a: String,
    pub b: String,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    /// Sorted by relation, then end a, then end b; one per (relation, a, b).
    pub matches: Vec<Match>,
    pub passes: usize,
    /// False when the matches were still changing after [`MAX_PASSES`]; then nothing is written.
    pub stable: bool,
    /// Roles set by hand: stored roles minus the stored automatic ones.
    pub hand_roles: Roles,
}

/// A stored field auto-apply reads or rewrites could not be parsed; the item is skipped so the
/// plugin's data is never overwritten with a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable(pub String);

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "custom field {} cannot be read", self.0)
    }
}

/// One action pairing: the two performers, the action each holds in it, and its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pairing {
    pub a: String,
    pub a_action: String,
    pub b: String,
    pub b_action: String,
    pub id: String,
}

/// A JSON field of the item; absent or empty is `default`, unparseable is an error.
fn field(cf: &Map<String, Value>, key: &str, default: Value) -> Result<Value, Unreadable> {
    match cf.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(default),
        Some(_) => json_field(cf, key).ok_or_else(|| Unreadable(key.to_string())),
    }
}

fn str_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A `{performerId: [tagId, …]}` field.
pub fn roles_field(cf: &Map<String, Value>, key: &str) -> Result<Roles, Unreadable> {
    let v = field(cf, key, Value::Object(Map::new()))?;
    let obj = v.as_object().ok_or_else(|| Unreadable(key.to_string()))?;
    let mut out = Roles::new();
    for (p, list) in obj {
        let list = list.as_array().ok_or_else(|| Unreadable(key.to_string()))?;
        let ids: BTreeSet<String> = list.iter().filter_map(str_of).collect();
        if !ids.is_empty() {
            out.insert(p.clone(), ids);
        }
    }
    Ok(out)
}

/// The item's action pairings.
pub fn pairings(cf: &Map<String, Value>) -> Result<Vec<Pairing>, Unreadable> {
    let key = "actionPairings";
    let v = field(cf, key, Value::Array(vec![]))?;
    let list = v.as_array().ok_or_else(|| Unreadable(key.to_string()))?;
    Ok(list
        .iter()
        .filter_map(|p| {
            Some(Pairing {
                a: str_of(&p["a"]["performer"])?,
                a_action: str_of(&p["a"]["actionTagId"])?,
                b: str_of(&p["b"]["performer"])?,
                b_action: str_of(&p["b"]["actionTagId"])?,
                id: str_of(&p["pairingId"]).unwrap_or_default(),
            })
        })
        .collect())
}

/// The stored relation instances, as raw JSON entries.
pub fn relation_entries(cf: &Map<String, Value>) -> Result<Vec<Value>, Unreadable> {
    let key = "relationAssignments";
    let v = field(cf, key, Value::Array(vec![]))?;
    v.as_array()
        .cloned()
        .ok_or_else(|| Unreadable(key.to_string()))
}

/// Whether a stored relation entry was applied automatically.
pub fn is_auto(entry: &Value) -> bool {
    match entry.get("auto") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true" || s == "1",
        Some(Value::Number(n)) => n.as_i64() == Some(1),
        _ => false,
    }
}

/// The (relation, a, b) key of a stored entry.
pub fn entry_key(entry: &Value) -> Option<(String, String, String)> {
    Some((
        str_of(&entry["relationTagId"])?,
        str_of(&entry["a"])?,
        str_of(&entry["b"])?,
    ))
}

/// The action pairing that links `p` and `q` under `rule`, if any: either direction, an
/// allowed action (a variant counts as its base Action).
pub fn link<'a>(
    model: &Model,
    pairs: &'a [Pairing],
    rule: &Rule,
    p: &str,
    q: &str,
) -> Option<&'a Pairing> {
    let base = |a: &str| model.defs.variant_of(a).unwrap_or(a).to_string();
    let allowed = |action: &str| {
        rule.actions.is_empty()
            || rule
                .actions
                .iter()
                .any(|x| *x == action || *x == base(action))
    };
    pairs.iter().find(|pr| {
        let joins = (pr.a == p && pr.b == q) || (pr.a == q && pr.b == p);
        joins && (allowed(&pr.a_action) || allowed(&pr.b_action))
    })
}

/// The roles `performer` holds (from `roles`) that meet a required role: the role itself or a
/// narrower one.
pub fn roles_meeting(model: &Model, roles: &Roles, performer: &str, required: &str) -> Vec<String> {
    roles
        .get(performer)
        .into_iter()
        .flatten()
        .filter(|r| model.hierarchy.is_same_or_narrower(r, required))
        .cloned()
        .collect()
}

pub fn meets(
    model: &Model,
    labels: &Labels,
    roles: &Roles,
    performer: &str,
    c: &Condition,
) -> bool {
    match c {
        Condition::Label(l) => labels.get(performer).is_some_and(|t| t.contains(l)),
        Condition::Role(r) => !roles_meeting(model, roles, performer, r).is_empty(),
    }
}

fn pass(
    model: &Model,
    labels: &Labels,
    roles: &Roles,
    performers: &[String],
    pairs: &[Pairing],
) -> Vec<Match> {
    let mut out: BTreeMap<(String, String, String), Match> = BTreeMap::new();
    for r in model
        .defs
        .relations
        .values()
        .filter(|r| r.valid && r.ends.is_some())
    {
        for rule in &r.rules {
            for p in performers {
                for q in performers {
                    if p == q {
                        continue;
                    }
                    if meets(model, labels, roles, p, &rule.a)
                        && meets(model, labels, roles, q, &rule.b)
                        && link(model, pairs, rule, p, q).is_some()
                    {
                        out.entry((r.id.clone(), p.clone(), q.clone()))
                            .or_insert_with(|| Match {
                                relation: r.id.clone(),
                                a: p.clone(),
                                b: q.clone(),
                                rule: rule.id.clone(),
                            });
                    }
                }
            }
        }
    }
    out.into_values().collect()
}

/// The end roles a set of matches gives each performer.
pub fn end_roles(model: &Model, ms: &[Match]) -> Roles {
    let mut out = Roles::new();
    for m in ms {
        if let Some((ea, eb)) = model
            .defs
            .relations
            .get(&m.relation)
            .and_then(|r| r.ends.clone())
        {
            out.entry(m.a.clone()).or_default().insert(ea);
            out.entry(m.b.clone()).or_default().insert(eb);
        }
    }
    out
}

fn union(a: &Roles, b: &Roles) -> Roles {
    let mut out = a.clone();
    for (p, rs) in b {
        out.entry(p.clone()).or_default().extend(rs.iter().cloned());
    }
    out
}

/// Every relation that applies to `item`, by its rules (see the module documentation).
pub fn matches(
    item: &ItemRecord,
    model: &Model,
    labels: &Labels,
) -> Result<Evaluation, Unreadable> {
    let cf = &item.custom_fields;
    let stored = roles_field(cf, "roleTagAssignments")?;
    let auto = roles_field(cf, "autoRoleAssignments")?;
    let pairs = pairings(cf)?;
    relation_entries(cf)?;
    let mut hand = Roles::new();
    for (p, rs) in &stored {
        let own: BTreeSet<String> = rs
            .iter()
            .filter(|r| !auto.get(p).is_some_and(|a| a.contains(*r)))
            .cloned()
            .collect();
        if !own.is_empty() {
            hand.insert(p.clone(), own);
        }
    }
    let mut performers = item.performers.clone();
    performers.sort();
    performers.dedup();

    let mut current = pass(model, labels, &hand, &performers, &pairs);
    let mut passes = 1;
    let mut stable = false;
    while passes < MAX_PASSES {
        let roles = union(&hand, &end_roles(model, &current));
        let next = pass(model, labels, &roles, &performers, &pairs);
        passes += 1;
        if next == current {
            stable = true;
            break;
        }
        current = next;
    }
    Ok(Evaluation {
        matches: current,
        passes: if stable { passes - 1 } else { passes },
        stable,
        hand_roles: hand,
    })
}
