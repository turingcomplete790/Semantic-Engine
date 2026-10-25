//! Roles, Relations, and auto-apply rules parsed from tags (spec 004 data-model; research R2).
//!
//! Tag types come from their flags, stored as `1` (number or string); any other value means not
//! set. JSON fields (`relationEnds`, `relationRules`) are JSON-encoded strings as the plugin
//! stores them, or plain JSON values. Problems are reported, never silently applied (FR-003).

use serde::Serialize;
use serde_json::{Map, Value};
use stash_adapter::relations::TagRecord;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// The kinds of definition problem the `relations` operation reports (contracts/operations-1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProblemKind {
    Loop,
    MissingEnd,
    EndNotRole,
    InvalidEnds,
    BadRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub kind: ProblemKind,
    /// The tags on a loop.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The relation the problem is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

impl Problem {
    fn about(kind: ProblemKind, relation: &str, detail: String) -> Problem {
        Problem {
            kind,
            tags: Vec::new(),
            relation: Some(relation.to_string()),
            detail,
        }
    }
}

/// One end's condition in an auto-apply rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Condition {
    /// The performer has this Label.
    Label(String),
    /// The performer has this Role, or a narrower one, in the scene or image.
    Role(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rule {
    pub id: String,
    pub a: Condition,
    pub b: Condition,
    /// Action tag ids whose pairing may link the pair; empty means any Action.
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelationDef {
    pub id: String,
    pub name: String,
    /// The Roles at ends a and b, in order; `None` when either is missing.
    pub ends: Option<(String, String)>,
    pub rules: Vec<Rule>,
    /// Direct Stash parents (not yet filtered to Relations; see `hierarchy`).
    pub parents: Vec<String>,
    /// False when the relation must not be applied (missing or non-Role end, loop, bad ends).
    pub valid: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Definitions {
    /// Every tag read, by id.
    pub tags: BTreeMap<String, TagRecord>,
    pub roles: BTreeSet<String>,
    pub labels: BTreeSet<String>,
    pub actions: BTreeSet<String>,
    pub pluggables: BTreeSet<String>,
    pub mise_en_scene: BTreeSet<String>,
    pub relations: BTreeMap<String, RelationDef>,
    /// Common variant tag id to the tag it is a variant of (`variantOfTagId`).
    pub variants: BTreeMap<String, String>,
    pub problems: Vec<Problem>,
}

/// True when a type flag is set: `1` or `true`, as a value or as text (the plugin's own rule,
/// for example `isActionTagValue` in its `actions.js`). The rule lives in the reader (spec 005).
pub fn flag_set(cf: &Map<String, Value>, flag: &str) -> bool {
    cf.get(flag)
        .is_some_and(stash_adapter::reader::field::flag_set)
}

/// A JSON field: a JSON-encoded string, or the JSON value itself. `None` when absent or not
/// parseable.
pub fn json_field(cf: &Map<String, Value>, key: &str) -> Option<Value> {
    match cf.get(key)? {
        Value::String(s) => serde_json::from_str(s).ok(),
        Value::Null => None,
        v => Some(v.clone()),
    }
}

/// A tag id from a JSON value: a string or a number.
fn id_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

impl Definitions {
    pub fn from_tags(tags: &[TagRecord]) -> Definitions {
        let mut d = Definitions::default();
        for t in tags {
            d.tags.insert(t.id.clone(), t.clone());
            let cf = &t.custom_fields;
            let id = t.id.clone();
            if flag_set(cf, "isRole") {
                d.roles.insert(id.clone());
            }
            if flag_set(cf, "isLabel") {
                d.labels.insert(id.clone());
            }
            if flag_set(cf, "isAction") {
                d.actions.insert(id.clone());
            }
            if flag_set(cf, "plugtag") {
                d.pluggables.insert(id.clone());
            }
            if flag_set(cf, "isMiseEnScene") {
                d.mise_en_scene.insert(id.clone());
            }
            if let Some(of) = cf.get("variantOfTagId").and_then(id_of) {
                d.variants.insert(id.clone(), of);
            }
        }
        // Relations after every tag's type is known, so ends and rules can be checked.
        for t in tags {
            if flag_set(&t.custom_fields, "isRelation") {
                let r = d.relation(t);
                d.relations.insert(r.id.clone(), r);
            }
        }
        d
    }

    pub fn is_role(&self, id: &str) -> bool {
        self.roles.contains(id)
    }
    pub fn is_label(&self, id: &str) -> bool {
        self.labels.contains(id)
    }
    pub fn is_action(&self, id: &str) -> bool {
        self.actions.contains(id)
    }
    pub fn is_relation(&self, id: &str) -> bool {
        self.relations.contains_key(id)
    }
    /// The tag a common variant stands for, if `id` is one.
    pub fn variant_of(&self, id: &str) -> Option<&str> {
        self.variants.get(id).map(String::as_str)
    }

    fn relation(&mut self, t: &TagRecord) -> RelationDef {
        let mut valid = true;
        let ends = json_field(&t.custom_fields, "relationEnds");
        let end = |k: &str| ends.as_ref().and_then(|e| e.get(k)).and_then(id_of);
        let ends = match (end("a"), end("b")) {
            (Some(a), Some(b)) => {
                for (name, role) in [("a", &a), ("b", &b)] {
                    if !self.is_role(role) {
                        valid = false;
                        self.problems.push(Problem::about(
                            ProblemKind::EndNotRole,
                            &t.id,
                            format!("end {name} {role} is not a Role tag"),
                        ));
                    }
                }
                Some((a, b))
            }
            (a, b) => {
                valid = false;
                let missing: Vec<&str> = [("a", a.is_none()), ("b", b.is_none())]
                    .into_iter()
                    .filter(|(_, m)| *m)
                    .map(|(n, _)| n)
                    .collect();
                self.problems.push(Problem::about(
                    ProblemKind::MissingEnd,
                    &t.id,
                    format!("relationEnds has no end {}", missing.join(" or ")),
                ));
                None
            }
        };
        let rules = self.rules(t);
        RelationDef {
            id: t.id.clone(),
            name: t.name.clone(),
            ends,
            rules,
            parents: t.parents.clone(),
            valid,
        }
    }

    fn rules(&mut self, t: &TagRecord) -> Vec<Rule> {
        let Some(raw) = json_field(&t.custom_fields, "relationRules") else {
            return Vec::new();
        };
        let Some(list) = raw.as_array() else {
            self.problems.push(Problem::about(
                ProblemKind::BadRule,
                &t.id,
                "relationRules is not a list".into(),
            ));
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for (i, r) in list.iter().enumerate() {
            match self.rule(r) {
                Ok(rule) if seen.insert(rule.id.clone()) => out.push(rule),
                Ok(rule) => self.problems.push(Problem::about(
                    ProblemKind::BadRule,
                    &t.id,
                    format!(
                        "rule id {:?} is used twice; the later rule is ignored",
                        rule.id
                    ),
                )),
                Err(why) => self.problems.push(Problem::about(
                    ProblemKind::BadRule,
                    &t.id,
                    format!("rule {}: {why}", i + 1),
                )),
            }
        }
        out
    }

    fn rule(&self, r: &Value) -> Result<Rule, String> {
        let id = r.get("id").and_then(id_of).ok_or("it has no id")?;
        let a = self.condition(r.get("a"), "a")?;
        let b = self.condition(r.get("b"), "b")?;
        let actions = match r.get("actions") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(xs)) => {
                let mut ids = Vec::new();
                for x in xs {
                    let a = id_of(x).ok_or("an action is not a tag id")?;
                    if !self.is_action(&a) {
                        return Err(format!("action {a} is not an Action tag"));
                    }
                    ids.push(a);
                }
                ids
            }
            Some(_) => return Err("actions is not a list".into()),
        };
        Ok(Rule { id, a, b, actions })
    }

    fn condition(&self, c: Option<&Value>, end: &str) -> Result<Condition, String> {
        let c = c.ok_or(format!("end {end} has no condition"))?;
        let label = c.get("label").and_then(id_of);
        let role = c.get("role").and_then(id_of);
        match (label, role) {
            (Some(l), None) if self.is_label(&l) => Ok(Condition::Label(l)),
            (Some(l), None) => Err(format!("end {end} label {l} is not a Label tag")),
            (None, Some(r)) if self.is_role(&r) => Ok(Condition::Role(r)),
            (None, Some(r)) => Err(format!("end {end} role {r} is not a Role tag")),
            _ => Err(format!("end {end} must have exactly one of label or role")),
        }
    }
}
