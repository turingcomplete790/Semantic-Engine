//! The library's records mapped to facts (contracts/mapping-rules.md). Mapping is pure: the
//! same records and server id always give the same facts and notes (FR-010).

use super::notes::{Note, NoteReason, Notes};
use super::{labels, timeline};
use crate::graph::{Fact, Term};
use crate::relations::defs::{Condition, Definitions, ProblemKind};
use semantic_vocabulary::iri::{valid_id, ItemIris};
use serde_json::{Map, Value};
use stash_adapter::reader::field::Field;
use stash_adapter::reader::{
    Kind, LibraryRead, MediaKind, MediaRecord, SemanticPerformer, SemanticTag, TagFlag,
};
use stash_adapter::relations::TagRecord;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const SEM: &str = "urn:semantic-stash:vocab:";
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// A vocabulary term.
pub fn sem(local: &str) -> Term {
    Term::iri(format!("{SEM}{local}"))
}

pub fn decimal(n: f64) -> Term {
    Term::typed(format!("{n}"), format!("{XSD}decimal"))
}

pub fn integer(n: i64) -> Term {
    Term::typed(n.to_string(), format!("{XSD}integer"))
}

pub fn non_negative(n: usize) -> Term {
    Term::typed(n.to_string(), format!("{XSD}nonNegativeInteger"))
}

pub fn boolean(b: bool) -> Term {
    Term::typed(b.to_string(), format!("{XSD}boolean"))
}

/// A Stash id: a string or JSON integer of decimal digits.
pub fn stash_id(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
            Some(s.clone())
        }
        Value::Number(n) => n.as_u64().map(|n| n.to_string()),
        _ => None,
    }
}

/// A plugin-made id (pairing, entry, rule): a string the item-IRI scheme accepts.
pub fn plugin_id(v: &Value) -> Option<String> {
    v.as_str().filter(|s| valid_id(s)).map(str::to_string)
}

/// Facts and notes from some records.
#[derive(Debug, Clone, Default)]
pub struct Mapped {
    pub facts: BTreeSet<Fact>,
    pub notes: Notes,
}

impl Mapped {
    pub fn extend(&mut self, other: Mapped) {
        self.facts.extend(other.facts);
        self.notes.extend(other.notes);
    }
}

/// Each tag's flags, by tag id.
#[derive(Debug, Clone, Default)]
pub struct TagIndex {
    pub flags: HashMap<String, BTreeSet<TagFlag>>,
}

impl TagIndex {
    pub fn from_tags(tags: &[SemanticTag]) -> TagIndex {
        TagIndex {
            flags: tags
                .iter()
                .map(|t| (t.id.clone(), t.flags.clone()))
                .collect(),
        }
    }

    /// `None` when the tag is not among the reader's tags.
    pub fn has(&self, id: &str, flag: TagFlag) -> Option<bool> {
        self.flags.get(id).map(|f| f.contains(&flag))
    }

    pub fn is(&self, id: &str, flag: TagFlag) -> bool {
        self.has(id, flag) == Some(true)
    }
}

/// What every record's mapping needs: the IRIs, the tag index, and the relation definitions,
/// parsed as the `relations` operation parses them (Principle III).
pub struct Ctx<'a> {
    pub iris: &'a ItemIris,
    pub index: TagIndex,
    pub defs: Definitions,
}

impl<'a> Ctx<'a> {
    pub fn new(tags: &[SemanticTag], iris: &'a ItemIris) -> Ctx<'a> {
        let records: Vec<TagRecord> = tags.iter().map(tag_record).collect();
        Ctx {
            iris,
            index: TagIndex::from_tags(tags),
            defs: Definitions::from_tags(&records),
        }
    }
}

/// A tag in spec 004's form, with its flags set as `1`.
fn tag_record(t: &SemanticTag) -> TagRecord {
    let mut custom_fields = Map::new();
    for f in &t.flags {
        custom_fields.insert(f.key().to_string(), Value::from(1));
    }
    for (k, f) in &t.fields {
        if let Field::Value(v) = f {
            custom_fields.insert(k.clone(), v.clone());
        }
    }
    TagRecord {
        id: t.id.clone(),
        name: t.name.clone(),
        parents: t.parents.clone(),
        custom_fields,
    }
}

pub fn flag_class(f: TagFlag) -> &'static str {
    match f {
        TagFlag::Pluggable => "Pluggable",
        TagFlag::Action => "Action",
        TagFlag::Role => "Role",
        TagFlag::Trait => "Trait",
        TagFlag::Label => "Label",
        TagFlag::MiseEnScene => "MiseEnScene",
        TagFlag::Variation => "Variation",
        TagFlag::Relation => "Relation",
    }
}

/// One record being mapped: where its notes point, and what it has produced so far.
pub struct Rec<'c, 'a> {
    pub ctx: &'c Ctx<'a>,
    kind: Kind,
    id: String,
    pub out: Mapped,
}

impl<'c, 'a> Rec<'c, 'a> {
    pub fn new(ctx: &'c Ctx<'a>, kind: Kind, id: &str) -> Self {
        Rec {
            ctx,
            kind,
            id: id.to_string(),
            out: Mapped::default(),
        }
    }

    pub fn iris(&self) -> &'a ItemIris {
        self.ctx.iris
    }

    pub fn note(&mut self, key: &str, entry: Option<String>, reason: NoteReason) {
        self.out.notes.push(Note {
            kind: self.kind,
            id: self.id.clone(),
            key: key.to_string(),
            entry,
            reason,
        });
    }

    pub fn add(&mut self, subject: &str, property: &str, object: Term) {
        self.out
            .facts
            .insert(Fact::new(Term::iri(subject), sem(property), object));
    }

    pub fn link(&mut self, subject: &str, property: &str, object: &str) {
        self.add(subject, property, Term::iri(object));
    }

    pub fn class(&mut self, subject: &str, class: &str) {
        self.out.facts.insert(Fact::new(
            Term::iri(subject),
            Term::iri(RDF_TYPE),
            sem(class),
        ));
    }

    /// A field's value; an unreadable field is noted and skipped.
    pub fn field<'f>(
        &mut self,
        fields: &'f BTreeMap<String, Field>,
        key: &str,
    ) -> Option<&'f Value> {
        match fields.get(key)? {
            Field::Value(Value::Null) => None,
            Field::Value(v) => Some(v),
            Field::Unreadable { .. } => {
                self.note(key, None, NoteReason::Unreadable);
                None
            }
        }
    }

    /// Notes a referenced tag that is missing, or lacks `flag`; the caller maps it anyway.
    pub fn check_tag(&mut self, key: &str, entry: Option<String>, tag: &str, flag: TagFlag) {
        match self.ctx.index.has(tag, flag) {
            None => self.note(key, entry, NoteReason::TagMissing),
            Some(false) => self.note(key, entry, NoteReason::TagWrongType),
            Some(true) => {}
        }
    }

    /// A list of Stash ids. A value that is not a list is noted `badShape`; an entry that is
    /// not an id is noted `invalidId` with its position.
    pub fn id_list(&mut self, key: &str, v: &Value) -> Vec<String> {
        let Some(list) = v.as_array() else {
            self.note(key, None, NoteReason::BadShape);
            return Vec::new();
        };
        let mut out = Vec::new();
        for (i, x) in list.iter().enumerate() {
            match stash_id(x) {
                Some(id) => out.push(id),
                None => self.note(key, Some(i.to_string()), NoteReason::InvalidId),
            }
        }
        out
    }

    /// A list of strings, for names. Entries that are not strings are noted `badShape`.
    pub fn string_list(&mut self, key: &str, v: &Value) -> Vec<String> {
        let Some(list) = v.as_array() else {
            self.note(key, None, NoteReason::BadShape);
            return Vec::new();
        };
        let mut out = Vec::new();
        for (i, x) in list.iter().enumerate() {
            match x.as_str() {
                Some(s) => out.push(s.to_string()),
                None => self.note(key, Some(i.to_string()), NoteReason::BadShape),
            }
        }
        out
    }
}

// ---- Tags

pub fn map_tag(ctx: &Ctx, t: &SemanticTag) -> Mapped {
    let mut rec = Rec::new(ctx, Kind::Tag, &t.id);
    let Ok(tag) = ctx.iris.try_tag(&t.id) else {
        rec.note("id", None, NoteReason::InvalidId);
        return rec.out;
    };
    if t.flags.is_empty() {
        rec.class(&tag, "Tag");
    }
    for f in &t.flags {
        rec.class(&tag, flag_class(*f));
    }
    rec.out.facts.insert(Fact::new(
        Term::iri(&tag),
        Term::iri(RDFS_LABEL),
        Term::string(&t.name),
    ));
    let is = |f| t.flags.contains(&f);

    for p in &t.parents {
        let Some(p) = stash_id(&Value::String(p.clone())) else {
            rec.note("parents", Some(p.clone()), NoteReason::InvalidId);
            continue;
        };
        let parent = ctx.iris.tag(&p);
        rec.link(&tag, "parentTag", &parent);
        if is(TagFlag::Role) && ctx.index.is(&p, TagFlag::Role) {
            rec.link(&tag, "broaderRole", &parent);
        }
        if is(TagFlag::Relation) && ctx.index.is(&p, TagFlag::Relation) {
            rec.link(&tag, "broaderRelation", &parent);
        }
    }

    if let Some(v) = rec.field(&t.fields, "categories") {
        if !(is(TagFlag::Role) || is(TagFlag::Trait) || is(TagFlag::MiseEnScene)) {
            rec.note("categories", None, NoteReason::TagWrongType);
        }
        for c in rec.string_list("categories", v) {
            rec.add(&tag, "hasCategory", Term::string(c));
        }
    }
    if let Some(v) = rec.field(&t.fields, "variations") {
        if !(is(TagFlag::Action) || is(TagFlag::Trait) || is(TagFlag::MiseEnScene)) {
            rec.note("variations", None, NoteReason::TagWrongType);
        }
        for c in rec.string_list("variations", v) {
            rec.add(&tag, "hasTagVariation", Term::string(c));
        }
    }
    if let Some(v) = rec.field(&t.fields, "variantOfTagId") {
        match stash_id(v) {
            Some(of) => rec.link(&tag, "variantOfTag", &ctx.iris.tag(&of)),
            None => rec.note("variantOfTagId", None, NoteReason::InvalidId),
        }
    }
    if let Some(v) = rec.field(&t.fields, "labelFormula") {
        labels::map_label_formula(&mut rec, &tag, v, is(TagFlag::Label));
    }
    if let Some(v) = rec.field(&t.fields, "variationData") {
        map_variation_data(&mut rec, &tag, v, is(TagFlag::Variation));
    }
    if let Some(v) = rec.field(&t.fields, "fieldMapping") {
        labels::map_field_mapping(&mut rec, &tag, v, is(TagFlag::Trait));
    }
    map_relation(&mut rec, t, &tag);
    rec.out
}

fn map_variation_data(rec: &mut Rec, tag: &str, v: &Value, is_variation: bool) {
    const KEY: &str = "variationData";
    let Some(o) = v.as_object() else {
        rec.note(KEY, None, NoteReason::BadShape);
        return;
    };
    if !is_variation {
        rec.note(KEY, None, NoteReason::TagWrongType);
    }
    match o.get("performerId").map(stash_id) {
        Some(Some(p)) => {
            let p = rec.iris().performer(&p);
            rec.link(tag, "variationOf", &p);
        }
        Some(None) => rec.note(KEY, Some("performerId".into()), NoteReason::InvalidId),
        None => {}
    }
    for (field, property) in [
        ("pluggableTagIds", "hasVariationPluggable"),
        ("labelTagIds", "hasVariationLabel"),
    ] {
        let Some(list) = o.get(field) else { continue };
        let Some(list) = list.as_array() else {
            rec.note(KEY, Some(field.into()), NoteReason::BadShape);
            continue;
        };
        for x in list {
            match stash_id(x) {
                Some(id) => {
                    let t = rec.iris().tag(&id);
                    rec.link(tag, property, &t);
                }
                None => rec.note(KEY, Some(field.into()), NoteReason::InvalidId),
            }
        }
    }
}

/// Relation ends and rules, from spec 004's parsed definitions.
fn map_relation(rec: &mut Rec, t: &SemanticTag, tag: &str) {
    let present = |k: &str| t.fields.contains_key(k);
    let Some(def) = rec.ctx.defs.relations.get(&t.id) else {
        for key in ["relationEnds", "relationRules"] {
            if present(key) {
                rec.note(key, None, NoteReason::TagWrongType);
            }
        }
        return;
    };
    for key in ["relationEnds", "relationRules"] {
        if matches!(t.fields.get(key), Some(Field::Unreadable { .. })) {
            rec.note(key, None, NoteReason::Unreadable);
        }
    }
    let problems: Vec<ProblemKind> = rec
        .ctx
        .defs
        .problems
        .iter()
        .filter(|p| p.relation.as_deref() == Some(t.id.as_str()))
        .map(|p| p.kind)
        .collect();
    for kind in problems {
        match kind {
            ProblemKind::EndNotRole => rec.note("relationEnds", None, NoteReason::TagWrongType),
            ProblemKind::MissingEnd if present("relationEnds") => {
                rec.note("relationEnds", None, NoteReason::BadShape)
            }
            ProblemKind::BadRule => rec.note("relationRules", None, NoteReason::BadShape),
            _ => {}
        }
    }
    if let Some((a, b)) = &def.ends {
        let (a, b) = (rec.iris().tag(a), rec.iris().tag(b));
        rec.link(tag, "endARole", &a);
        rec.link(tag, "endBRole", &b);
    }
    for rule in &def.rules {
        if !valid_id(&rule.id) {
            rec.note(
                "relationRules",
                Some(rule.id.clone()),
                NoteReason::InvalidId,
            );
            continue;
        }
        let r = rec.iris().relation_rule(tag, &rule.id);
        rec.link(tag, "hasRelationRule", &r);
        rec.class(&r, "RelationRule");
        rec.add(&r, "relationRuleId", Term::string(&rule.id));
        for (cond, label, role) in [
            (&rule.a, "endALabel", "endARoleCondition"),
            (&rule.b, "endBLabel", "endBRoleCondition"),
        ] {
            let (property, id) = match cond {
                Condition::Label(l) => (label, l),
                Condition::Role(r) => (role, r),
            };
            let target = rec.iris().tag(id);
            rec.link(&r, property, &target);
        }
        for a in &rule.actions {
            let action = rec.iris().tag(a);
            rec.link(&r, "linkingAction", &action);
        }
    }
}

// ---- Performers

pub fn map_performer(ctx: &Ctx, p: &SemanticPerformer) -> Mapped {
    let mut rec = Rec::new(ctx, Kind::Performer, &p.id);
    let Ok(performer) = ctx.iris.try_performer(&p.id) else {
        rec.note("id", None, NoteReason::InvalidId);
        return rec.out;
    };
    rec.class(&performer, "Performer");
    for t in &p.tags {
        for (flag, property) in [
            (TagFlag::Trait, "hasTrait"),
            (TagFlag::Label, "hasLabel"),
            (TagFlag::Variation, "hasPerformerVariation"),
        ] {
            if ctx.index.is(t, flag) {
                rec.link(&performer, property, &ctx.iris.tag(t));
            }
        }
    }
    if p.archetype {
        rec.class(&performer, "Archetype");
    }
    match &p.archetype_parents {
        Some(Field::Value(Value::Null)) | None => {}
        Some(Field::Value(v)) => {
            if !p.archetype {
                rec.note("archetypeParents", None, NoteReason::TagWrongType);
            }
            for parent in rec.id_list("archetypeParents", v) {
                rec.link(&performer, "parentArchetype", &ctx.iris.performer(&parent));
            }
        }
        Some(Field::Unreadable { .. }) => {
            rec.note("archetypeParents", None, NoteReason::Unreadable)
        }
    }
    if let Some(m) = &p.measurements {
        if let Some(b) = m.bust_size {
            rec.add(&performer, "bustSize", decimal(b));
        }
        if let Some(w) = m.waist {
            rec.add(&performer, "waist", integer(w));
        }
        if let Some(h) = m.hip {
            rec.add(&performer, "hip", integer(h));
        }
        if let Some(r) = m.waist_to_hip_ratio {
            rec.add(&performer, "waistToHipRatio", decimal(r));
        }
    }
    rec.out
}

// ---- Media

/// The instances and pairings a media record holds, so pairings and timeline slots can tell
/// whether what they name exists.
#[derive(Debug, Default)]
pub struct Instances {
    pub roles: BTreeSet<(String, String)>,
    pub actions: BTreeSet<(String, String)>,
    pub pluggables: BTreeSet<(String, String)>,
    pub action_pairings: BTreeSet<String>,
    pub role_pairings: BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum InstanceKind {
    Role,
    Action,
    Pluggable,
}

impl InstanceKind {
    fn class(self) -> &'static str {
        match self {
            InstanceKind::Role => "RoleInstance",
            InstanceKind::Action => "ActionInstance",
            InstanceKind::Pluggable => "PluggableInstance",
        }
    }
    fn property(self) -> &'static str {
        match self {
            InstanceKind::Role => "hasRole",
            InstanceKind::Action => "hasAction",
            InstanceKind::Pluggable => "hasPluggable",
        }
    }
    fn flag(self) -> TagFlag {
        match self {
            InstanceKind::Role => TagFlag::Role,
            InstanceKind::Action => TagFlag::Action,
            InstanceKind::Pluggable => TagFlag::Pluggable,
        }
    }
}

pub fn instance_iri(iris: &ItemIris, media: &str, kind: &str, p: &str, t: &str) -> String {
    match kind {
        "role" => iris.role_instance(media, p, t),
        "action" => iris.action_instance(media, p, t),
        _ => iris.pluggable_instance(media, p, t),
    }
}

struct Media<'m> {
    iri: String,
    performers: BTreeSet<&'m str>,
}

/// Writes one instance with its media, performer and tag, and the performer's link to it.
fn instance(rec: &mut Rec, m: &Media, kind: InstanceKind, key: &str, p: &str, t: &str) -> String {
    let i = match kind {
        InstanceKind::Role => rec.iris().role_instance(&m.iri, p, t),
        InstanceKind::Action => rec.iris().action_instance(&m.iri, p, t),
        InstanceKind::Pluggable => rec.iris().pluggable_instance(&m.iri, p, t),
    };
    let (performer, tag) = (rec.iris().performer(p), rec.iris().tag(t));
    rec.class(&i, kind.class());
    rec.link(&i, "inMedia", &m.iri);
    rec.link(&i, "forPerformer", &performer);
    rec.link(&i, "ofTag", &tag);
    rec.link(&performer, kind.property(), &i);
    rec.check_tag(key, Some(p.to_string()), t, kind.flag());
    if !m.performers.contains(p) {
        rec.note(key, Some(p.to_string()), NoteReason::PerformerNotInMedia);
    }
    i
}

/// A map of performer id to a list of entries. Yields (performer, entry) pairs; bad keys and
/// non-list values are noted.
fn per_performer<'v>(rec: &mut Rec, key: &str, v: &'v Value) -> Vec<(String, &'v Value)> {
    let Some(o) = v.as_object() else {
        rec.note(key, None, NoteReason::BadShape);
        return Vec::new();
    };
    let mut out = Vec::new();
    for (p, list) in o {
        let Some(p) = stash_id(&Value::String(p.clone())) else {
            rec.note(key, Some(p.clone()), NoteReason::InvalidId);
            continue;
        };
        let Some(list) = list.as_array() else {
            rec.note(key, Some(p.clone()), NoteReason::BadShape);
            continue;
        };
        out.extend(list.iter().map(|e| (p.clone(), e)));
    }
    out
}

pub fn map_media(ctx: &Ctx, r: &MediaRecord) -> Mapped {
    let kind = Kind::from(r.kind);
    let mut rec = Rec::new(ctx, kind, &r.id);
    let iri = match r.kind {
        MediaKind::Scene => ctx.iris.try_scene(&r.id),
        MediaKind::Image => ctx.iris.try_image(&r.id),
    };
    let Ok(iri) = iri else {
        rec.note("id", None, NoteReason::InvalidId);
        return rec.out;
    };
    rec.class(
        &iri,
        match r.kind {
            MediaKind::Scene => "Scene",
            MediaKind::Image => "Image",
        },
    );
    for p in &r.performers {
        match ctx.iris.try_performer(p) {
            Ok(perf) => rec.link(&iri, "hasPerformer", &perf),
            Err(_) => rec.note("performers", Some(p.clone()), NoteReason::InvalidId),
        }
    }
    let m = Media {
        iri,
        performers: r.performers.iter().map(String::as_str).collect(),
    };
    let mut inst = Instances::default();

    // Role, action and pluggable assignments.
    if let Some(v) = rec.field(&r.fields, "roleTagAssignments") {
        let key = "roleTagAssignments";
        for (p, e) in per_performer(&mut rec, key, v) {
            match stash_id(e) {
                Some(t) => {
                    instance(&mut rec, &m, InstanceKind::Role, key, &p, &t);
                    inst.roles.insert((p, t));
                }
                None => rec.note(key, Some(p), NoteReason::InvalidId),
            }
        }
    }
    if let Some(v) = rec.field(&r.fields, "autoRoleAssignments") {
        let key = "autoRoleAssignments";
        for (p, e) in per_performer(&mut rec, key, v) {
            match stash_id(e) {
                Some(t) => {
                    let i = instance(&mut rec, &m, InstanceKind::Role, key, &p, &t);
                    rec.add(&i, "roleAppliedAutomatically", boolean(true));
                    if !inst.roles.contains(&(p.clone(), t.clone())) {
                        rec.note(key, Some(p.clone()), NoteReason::NotRecordedRole);
                    }
                    inst.roles.insert((p, t));
                }
                None => rec.note(key, Some(p), NoteReason::InvalidId),
            }
        }
    }
    if let Some(v) = rec.field(&r.fields, "actionAssignments") {
        let key = "actionAssignments";
        for (p, e) in per_performer(&mut rec, key, v) {
            let Some(o) = e.as_object() else {
                rec.note(key, Some(p), NoteReason::BadShape);
                continue;
            };
            let Some(t) = o.get("actionTagId").and_then(stash_id) else {
                rec.note(key, Some(p), NoteReason::InvalidId);
                continue;
            };
            let i = instance(&mut rec, &m, InstanceKind::Action, key, &p, &t);
            match o.get("variations") {
                None | Some(Value::Null) => {}
                Some(Value::Array(vs)) => {
                    for x in vs {
                        match x.as_str() {
                            Some(s) if !s.is_empty() => {
                                rec.add(&i, "hasVariation", Term::string(s))
                            }
                            Some(_) => {}
                            None => rec.note(key, Some(p.clone()), NoteReason::BadShape),
                        }
                    }
                }
                Some(_) => rec.note(key, Some(p.clone()), NoteReason::BadShape),
            }
            inst.actions.insert((p, t));
        }
    }
    if let Some(v) = rec.field(&r.fields, "pluggableAssignments") {
        let key = "pluggableAssignments";
        for (p, e) in per_performer(&mut rec, key, v) {
            match stash_id(e) {
                Some(t) => {
                    instance(&mut rec, &m, InstanceKind::Pluggable, key, &p, &t);
                    inst.pluggables.insert((p, t));
                }
                None => rec.note(key, Some(p), NoteReason::InvalidId),
            }
        }
    }

    // Pairings.
    for (key, class, tag_key, kind) in [
        ("actionPairings", "ActionPairing", "actionTagId", "action"),
        ("roleTagPairings", "RolePairing", "roleTagId", "role"),
    ] {
        let Some(v) = rec.field(&r.fields, key) else {
            continue;
        };
        let Some(list) = v.as_array() else {
            rec.note(key, None, NoteReason::BadShape);
            continue;
        };
        for (n, e) in list.iter().enumerate() {
            let entry = Some(n.to_string());
            let Some(o) = e.as_object() else {
                rec.note(key, entry, NoteReason::BadShape);
                continue;
            };
            let id = match o.get("pairingId") {
                Some(v) => match plugin_id(v) {
                    Some(id) => id,
                    None => {
                        rec.note(key, entry, NoteReason::InvalidId);
                        continue;
                    }
                },
                None => {
                    rec.note(key, entry, NoteReason::BadShape);
                    continue;
                }
            };
            let end = |side: &str| -> Result<(String, String), NoteReason> {
                let o = o
                    .get(side)
                    .and_then(Value::as_object)
                    .ok_or(NoteReason::BadShape)?;
                let p = o.get("performer").ok_or(NoteReason::BadShape)?;
                let t = o.get(tag_key).ok_or(NoteReason::BadShape)?;
                Ok((
                    stash_id(p).ok_or(NoteReason::InvalidId)?,
                    stash_id(t).ok_or(NoteReason::InvalidId)?,
                ))
            };
            let (a, b) = match (end("a"), end("b")) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(why), _) | (_, Err(why)) => {
                    rec.note(key, entry, why);
                    continue;
                }
            };
            let held = if kind == "action" {
                &inst.actions
            } else {
                &inst.roles
            };
            if !held.contains(&a) || !held.contains(&b) {
                rec.note(key, entry, NoteReason::DanglingRef);
                continue;
            }
            let q = rec.iris().pairing(&m.iri, &id);
            let ia = instance_iri(rec.iris(), &m.iri, kind, &a.0, &a.1);
            let ib = instance_iri(rec.iris(), &m.iri, kind, &b.0, &b.1);
            rec.class(&q, class);
            rec.link(&q, "inMedia", &m.iri);
            rec.add(&q, "pairingId", Term::string(&id));
            rec.link(&q, "pairsA", &ia);
            rec.link(&q, "pairsB", &ib);
            match o.get("direction") {
                None | Some(Value::Null) if kind == "action" => {
                    rec.add(&q, "pairingDirection", Term::string("bidirectional"))
                }
                None | Some(Value::Null) => {}
                Some(Value::String(d))
                    if ["bidirectional", "give", "receive"].contains(&d.as_str()) =>
                {
                    rec.add(&q, "pairingDirection", Term::string(d))
                }
                Some(_) => rec.note(key, entry, NoteReason::BadShape),
            }
            if kind == "action" {
                inst.action_pairings.insert(id);
            } else {
                inst.role_pairings.insert(id);
            }
        }
    }

    // Relation instances.
    if let Some(v) = rec.field(&r.fields, "relationAssignments") {
        map_relation_assignments(&mut rec, &m.iri, v);
    }

    if let Some(v) = rec.field(&r.fields, "timeline") {
        timeline::map_timeline(&mut rec, &m.iri, v, &inst);
    }
    rec.out
}

fn map_relation_assignments(rec: &mut Rec, media: &str, v: &Value) {
    const KEY: &str = "relationAssignments";
    let Some(list) = v.as_array() else {
        rec.note(KEY, None, NoteReason::BadShape);
        return;
    };
    for (n, e) in list.iter().enumerate() {
        let entry = Some(n.to_string());
        let Some(o) = e.as_object() else {
            rec.note(KEY, entry, NoteReason::BadShape);
            continue;
        };
        let ids: Option<Vec<String>> = ["relationTagId", "a", "b"]
            .iter()
            .map(|k| o.get(*k).and_then(stash_id))
            .collect();
        let Some([r, a, b]) = ids.map(|v| <[String; 3]>::try_from(v).expect("three ids")) else {
            rec.note(KEY, entry, NoteReason::InvalidId);
            continue;
        };
        let rule = match o.get("ruleId") {
            None | Some(Value::Null) => None,
            Some(v) => match plugin_id(v) {
                Some(id) => Some(id),
                None => {
                    rec.note(KEY, entry, NoteReason::InvalidId);
                    continue;
                }
            },
        };
        let relation = rec.iris().tag(&r);
        let x = rec.iris().relation_instance(media, &r, &a, &b);
        let (pa, pb) = (rec.iris().performer(&a), rec.iris().performer(&b));
        rec.class(&x, "RelationInstance");
        rec.link(&x, "inMedia", media);
        rec.link(&x, "ofRelation", &relation);
        rec.link(&x, "atEndA", &pa);
        rec.link(&x, "atEndB", &pb);
        let auto = o.get("auto").and_then(Value::as_bool) == Some(true);
        rec.add(&x, "appliedAutomatically", boolean(auto));
        if let Some(rule) = rule {
            let r = rec.iris().relation_rule(&relation, &rule);
            rec.link(&x, "appliedByRule", &r);
        }
        rec.check_tag(KEY, entry, &r, TagFlag::Relation);
    }
}

// ---- The whole library

/// Maps every record: tags, performers, scenes, images, in that order.
pub fn map_library(read: &LibraryRead, iris: &ItemIris) -> Mapped {
    let ctx = Ctx::new(&read.tags, iris);
    let mut out = Mapped::default();
    for t in &read.tags {
        out.extend(map_tag(&ctx, t));
    }
    for p in &read.performers {
        out.extend(map_performer(&ctx, p));
    }
    for m in read.scenes.iter().chain(&read.images) {
        out.extend(map_media(&ctx, m));
    }
    out
}
