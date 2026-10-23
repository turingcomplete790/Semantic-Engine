//! The minimal change to write for one item (spec 004 FR-008 to FR-010; research R5).
//!
//! Automatic relation instances and the end roles they add are recomputed from the evaluation;
//! anything applied by hand is kept as it is and never duplicated. Only the keys whose content
//! changed are written, so an item that is already up to date gives no patch at all.

use super::evaluate::{
    end_roles, entry_key, is_auto, relation_entries, roles_field, Evaluation, Match, Roles,
    Unreadable,
};
use super::Model;
use serde_json::{json, Map, Value};
use stash_adapter::relations::ItemRecord;
use std::collections::BTreeSet;

pub const RELATIONS_KEY: &str = "relationAssignments";
pub const ROLES_KEY: &str = "roleTagAssignments";
pub const AUTO_ROLES_KEY: &str = "autoRoleAssignments";

/// A partial `custom_fields` update: each value is JSON encoded as a string, as the plugin stores
/// these keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    fields: Map<String, Value>,
}

impl Patch {
    pub fn partial(&self) -> Map<String, Value> {
        self.fields.clone()
    }
    pub fn keys(&self) -> Vec<String> {
        self.fields.keys().cloned().collect()
    }
}

fn auto_entry(m: &Match) -> Value {
    json!({ "relationTagId": m.relation, "a": m.a, "b": m.b, "auto": true, "ruleId": m.rule })
}

fn roles_json(roles: &Roles) -> Value {
    Value::Object(
        roles
            .iter()
            .filter(|(_, rs)| !rs.is_empty())
            .map(|(p, rs)| (p.clone(), json!(rs.iter().collect::<Vec<_>>())))
            .collect(),
    )
}

fn without_empty(roles: Roles) -> Roles {
    roles.into_iter().filter(|(_, rs)| !rs.is_empty()).collect()
}

/// The patch that brings `item` in line with `eval`, or `None` when nothing changes or the
/// evaluation did not settle.
pub fn plan(
    item: &ItemRecord,
    eval: &Evaluation,
    model: &Model,
) -> Result<Option<Patch>, Unreadable> {
    if !eval.stable {
        return Ok(None);
    }
    let cf = &item.custom_fields;
    let stored_entries = relation_entries(cf)?;
    let stored_roles = without_empty(roles_field(cf, ROLES_KEY)?);
    let stored_auto_roles = without_empty(roles_field(cf, AUTO_ROLES_KEY)?);

    let hand: Vec<&Value> = stored_entries.iter().filter(|e| !is_auto(e)).collect();
    let hand_keys: BTreeSet<_> = hand.iter().filter_map(|e| entry_key(e)).collect();
    let desired: Vec<Match> = eval
        .matches
        .iter()
        .filter(|m| !hand_keys.contains(&(m.relation.clone(), m.a.clone(), m.b.clone())))
        .cloned()
        .collect();

    // Relation instances: hand entries as stored, then the automatic ones in a fixed order.
    let mut new_entries: Vec<Value> = hand.iter().map(|e| (*e).clone()).collect();
    new_entries.extend(desired.iter().map(auto_entry));
    let stored_auto: BTreeSet<String> = stored_entries
        .iter()
        .filter(|e| is_auto(e))
        .map(|e| {
            let (r, a, b) = entry_key(e).unwrap_or_default();
            format!("{r}:{a}:{b}:{}", e["ruleId"].as_str().unwrap_or(""))
        })
        .collect();
    let desired_auto: BTreeSet<String> = desired
        .iter()
        .map(|m| format!("{}:{}:{}:{}", m.relation, m.a, m.b, m.rule))
        .collect();
    let auto_count = stored_entries.iter().filter(|e| is_auto(e)).count();
    let relations_changed = stored_auto != desired_auto || auto_count != desired.len();

    // End roles: added only where the performer does not already hold the role by hand.
    let mut new_auto = Roles::new();
    for (p, rs) in end_roles(model, &desired) {
        let own = eval.hand_roles.get(&p);
        let added: BTreeSet<String> = rs
            .into_iter()
            .filter(|r| !own.is_some_and(|o| o.contains(r)))
            .collect();
        if !added.is_empty() {
            new_auto.insert(p, added);
        }
    }
    let mut new_roles = eval.hand_roles.clone();
    for (p, rs) in &new_auto {
        new_roles
            .entry(p.clone())
            .or_default()
            .extend(rs.iter().cloned());
    }
    let new_roles = without_empty(new_roles);

    let mut fields = Map::new();
    if relations_changed {
        fields.insert(
            RELATIONS_KEY.into(),
            Value::String(Value::Array(new_entries).to_string()),
        );
    }
    if new_roles != stored_roles {
        fields.insert(
            ROLES_KEY.into(),
            Value::String(roles_json(&new_roles).to_string()),
        );
    }
    if new_auto != stored_auto_roles {
        fields.insert(
            AUTO_ROLES_KEY.into(),
            Value::String(roles_json(&new_auto).to_string()),
        );
    }
    Ok((!fields.is_empty()).then_some(Patch { fields }))
}
