//! Explaining one relation instance on one scene or image (spec 004 FR-014; research R10).
//!
//! The explanation re-runs the same evaluation that applied the relation, so it cannot disagree
//! with what was written (constitution Principle III).

use super::defs::Condition;
use super::evaluate::{
    entry_key, is_auto, link, matches, pairings, relation_entries, roles_field, Labels, Roles,
    Unreadable,
};
use super::Model;
use serde_json::{json, Value};
use stash_adapter::relations::ItemRecord;

fn met(model: &Model, hand: &Roles, all: &Roles, performer: &str, c: &Condition) -> Value {
    match c {
        Condition::Label(l) => json!({ "label": l }),
        Condition::Role(r) => {
            // Prefer roles set by hand; fall back to roles another relation added.
            let mut held = super::evaluate::roles_meeting(model, hand, performer, r);
            if held.is_empty() {
                held = super::evaluate::roles_meeting(model, all, performer, r);
            }
            let via: Vec<&String> = held.iter().filter(|h| *h != r).collect();
            if held.iter().any(|h| h == r) || via.is_empty() {
                json!({ "role": r })
            } else {
                json!({ "role": r, "via": via })
            }
        }
    }
}

/// The explanation of relation `relation` between performers `a` and `b` on `item`, in the
/// `explainRelation` result shape (contracts/operations-1.1.md).
pub fn explain(
    item: &ItemRecord,
    model: &Model,
    labels: &Labels,
    relation: &str,
    a: &str,
    b: &str,
) -> Result<Value, Unreadable> {
    let cf = &item.custom_fields;
    let key = (relation.to_string(), a.to_string(), b.to_string());
    let entries = relation_entries(cf)?;
    let stored: Vec<&Value> = entries
        .iter()
        .filter(|e| entry_key(e).as_ref() == Some(&key))
        .collect();
    if stored.iter().any(|e| !is_auto(e)) {
        return Ok(json!({ "applied": "hand" }));
    }
    let Some(entry) = stored.first() else {
        return Ok(json!({ "applied": "none" }));
    };

    let eval = matches(item, model, labels)?;
    let Some(m) = eval
        .matches
        .iter()
        .find(|m| m.relation == relation && m.a == a && m.b == b)
    else {
        return Ok(json!({
            "applied": "auto",
            "holds": false,
            "ruleId": entry["ruleId"],
        }));
    };
    let r = &model.defs.relations[relation];
    let rule = r
        .rules
        .iter()
        .find(|x| x.id == m.rule)
        .expect("matched rule exists");
    let all = roles_field(cf, "roleTagAssignments")?;
    let pairs = pairings(cf)?;
    let pairing = link(model, &pairs, rule, a, b).expect("a match has a linking pairing");
    let action = if pairing.a == a {
        &pairing.a_action
    } else {
        &pairing.b_action
    };
    let auto = roles_field(cf, "autoRoleAssignments")?;
    let (ea, eb) = r.ends.clone().expect("valid relations have ends");
    let mut added = serde_json::Map::new();
    for (p, end) in [(a, &ea), (b, &eb)] {
        if auto.get(p).is_some_and(|rs| rs.contains(end)) {
            added.insert(p.to_string(), json!([end]));
        }
    }
    Ok(json!({
        "applied": "auto",
        "holds": true,
        "ruleId": m.rule,
        "endA": { "performer": a, "met": met(model, &eval.hand_roles, &all, a, &rule.a) },
        "endB": { "performer": b, "met": met(model, &eval.hand_roles, &all, b, &rule.b) },
        "link": { "action": action, "by": "pairing", "pairingId": pairing.id },
        "addedRoles": added,
    }))
}
