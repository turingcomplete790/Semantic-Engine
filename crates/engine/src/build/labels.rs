//! Label formulas and Field Mapping slots (mapping-rules § Label formulas, § Field Mapping).

use super::map::{boolean, decimal, integer, plugin_id, stash_id, Rec};
use super::notes::NoteReason;
use crate::graph::Term;
use semantic_vocabulary::iri::FieldMappingSlot;
use serde_json::Value;

/// The threshold when none is stored, or the stored one is not a number above 0: the documented
/// default, and the rule the plugin's own graph applies.
pub const DEFAULT_THRESHOLD: f64 = 100.0;

/// The rule lists of a formula: key, class, and the key of the tag the rule points at.
const RULE_LISTS: [(&str, &str, Option<&str>); 4] = [
    ("traitRules", "TraitRule", Some("tagId")),
    ("fieldRules", "FieldRule", None),
    ("labelRules", "LabelDependencyRule", Some("labelTagId")),
    ("pluggableRules", "PluggableRule", Some("pluggableTagId")),
];

/// A field rule's value as text: strings as they are, numbers and booleans as written.
fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub fn map_label_formula(rec: &mut Rec, tag: &str, v: &Value, is_label: bool) {
    const KEY: &str = "labelFormula";
    let Some(o) = v.as_object() else {
        rec.note(KEY, None, NoteReason::BadShape);
        return;
    };
    if !is_label {
        rec.note(KEY, None, NoteReason::TagWrongType);
    }
    let threshold = o
        .get("threshold")
        .and_then(Value::as_f64)
        .filter(|n| *n > 0.0)
        .unwrap_or(DEFAULT_THRESHOLD);
    rec.add(tag, "hasThreshold", decimal(threshold));
    for (list_key, class, tag_key) in RULE_LISTS {
        let Some(list) = o.get(list_key) else {
            continue;
        };
        if list.is_null() {
            continue;
        }
        let Some(list) = list.as_array() else {
            rec.note(KEY, Some(list_key.into()), NoteReason::BadShape);
            continue;
        };
        for (n, rule) in list.iter().enumerate() {
            let entry = Some(format!("{list_key}/{n}"));
            let Some(rule) = rule.as_object() else {
                rec.note(KEY, entry, NoteReason::BadShape);
                continue;
            };
            let id = match rule.get("id") {
                None | Some(Value::Null) => {
                    rec.note(KEY, entry, NoteReason::BadShape);
                    continue;
                }
                Some(v) => match plugin_id(v) {
                    Some(id) => id,
                    None => {
                        rec.note(KEY, entry, NoteReason::InvalidId);
                        continue;
                    }
                },
            };
            let r = rec.iris().label_rule(tag, &id);
            rec.link(tag, "hasRule", &r);
            rec.class(&r, class);
            rec.add(&r, "ruleId", Term::string(&id));
            if let Some(w) = rule.get("weight").and_then(Value::as_f64) {
                rec.add(&r, "hasWeight", decimal(w));
            }
            if let Some(tag_key) = tag_key {
                match rule.get(tag_key).map(stash_id) {
                    Some(Some(t)) => {
                        let t = rec.iris().tag(&t);
                        rec.link(&r, "ofTag", &t);
                    }
                    _ => rec.note(KEY, entry.clone(), NoteReason::InvalidId),
                }
            }
            match list_key {
                "traitRules" => {
                    if let Some(m) = rule.get("variationMatch").and_then(Value::as_str) {
                        if !m.is_empty() && m != "__any__" {
                            rec.add(&r, "hasVariation", Term::string(m));
                        }
                    }
                    if let Some(b) = rule.get("recursive").and_then(Value::as_bool) {
                        rec.add(&r, "isRecursive", boolean(b));
                    }
                }
                "fieldRules" => {
                    for (field, property) in [
                        ("field", "hasField"),
                        ("modifier", "hasModifier"),
                        ("value", "hasValue"),
                        ("value2", "hasValue2"),
                    ] {
                        if let Some(t) = rule.get(field).and_then(text) {
                            rec.add(&r, property, Term::string(t));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// One slot `{field, priority}`; a slot without a field is noted and skipped.
fn slot(
    rec: &mut Rec,
    tag: &str,
    which: FieldMappingSlot,
    v: &Value,
    entry: String,
) -> Option<String> {
    const KEY: &str = "fieldMapping";
    let field = v
        .get("field")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let Some(field) = field else {
        rec.note(KEY, Some(entry), NoteReason::BadShape);
        return None;
    };
    let f = rec.iris().field_mapping(tag, which);
    rec.link(tag, "hasFieldMapping", &f);
    rec.class(&f, "FieldMappingRule");
    rec.add(&f, "mapsToField", Term::string(field));
    if let Some(p) = v.get("priority").and_then(Value::as_i64) {
        rec.add(&f, "mappingPriority", integer(p));
    }
    Some(f)
}

pub fn map_field_mapping(rec: &mut Rec, tag: &str, v: &Value, is_trait: bool) {
    const KEY: &str = "fieldMapping";
    let Some(o) = v.as_object() else {
        rec.note(KEY, None, NoteReason::BadShape);
        return;
    };
    if let Some(base) = o.get("base").filter(|b| !b.is_null()) {
        slot(rec, tag, FieldMappingSlot::Base, base, "base".into());
    }
    let Some(vars) = o.get("variations").filter(|v| !v.is_null()) else {
        return;
    };
    let Some(vars) = vars.as_object() else {
        rec.note(KEY, Some("variations".into()), NoteReason::BadShape);
        return;
    };
    if !is_trait && !vars.is_empty() {
        rec.note(KEY, Some("variations".into()), NoteReason::TagWrongType);
    }
    let mut names: Vec<&String> = vars.keys().collect();
    names.sort();
    for (n, name) in names.into_iter().enumerate() {
        let which = FieldMappingSlot::Variation(n);
        if let Some(f) = slot(rec, tag, which, &vars[name], format!("variations/{n}")) {
            rec.add(&f, "mappingVariation", Term::string(name));
        }
    }
}
