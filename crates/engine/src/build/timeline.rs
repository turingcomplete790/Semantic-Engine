//! The timeline: entries, Stages and their picks, slots, and the media's Mise-en-scène
//! instances (mapping-rules § Timeline). `inStage` is a rule over position and never written.

use super::map::{decimal, instance_iri, non_negative, plugin_id, stash_id, Instances, Rec};
use super::notes::NoteReason;
use crate::graph::Term;
use serde_json::{Map, Value};
use stash_adapter::reader::TagFlag;
use std::collections::{BTreeMap, BTreeSet};

const KEY: &str = "timeline";

/// What a slot places, once resolved against the record's instances and pairings.
enum Target {
    Instance(String),
    Pairing(String),
}

/// Resolves a slot's `ref`. `Err` skips the whole entry; `Ok(None)` keeps the slot without
/// `places`.
fn target(
    rec: &mut Rec,
    media: &str,
    r: &Map<String, Value>,
    inst: &Instances,
    entry: &str,
) -> Result<Option<Target>, ()> {
    let note = |rec: &mut Rec, why| rec.note(KEY, Some(entry.to_string()), why);
    let kind = r.get("kind").and_then(Value::as_str).unwrap_or("");
    match kind {
        "action-instance" | "role-instance" => {
            let (short, tag_key, held) = if kind == "action-instance" {
                ("action", "actionTagId", &inst.actions)
            } else {
                ("role", "roleTagId", &inst.roles)
            };
            let p = r.get("performerId").and_then(stash_id);
            let t = r.get(tag_key).and_then(stash_id);
            let (Some(p), Some(t)) = (p, t) else {
                note(rec, NoteReason::InvalidId);
                return Ok(None);
            };
            if !held.contains(&(p.clone(), t.clone())) {
                note(rec, NoteReason::DanglingRef);
                return Ok(None);
            }
            Ok(Some(Target::Instance(instance_iri(
                rec.iris(),
                media,
                short,
                &p,
                &t,
            ))))
        }
        "action-pairing" | "role-pairing" => {
            let held = if kind == "action-pairing" {
                &inst.action_pairings
            } else {
                &inst.role_pairings
            };
            let Some(id) = r.get("pairingId").and_then(plugin_id) else {
                note(rec, NoteReason::InvalidId);
                return Ok(None);
            };
            if !held.contains(&id) {
                note(rec, NoteReason::DanglingRef);
                return Ok(None);
            }
            Ok(Some(Target::Pairing(rec.iris().pairing(media, &id))))
        }
        "mise-en-scene" => {
            note(rec, NoteReason::Retired);
            Err(())
        }
        _ => {
            note(rec, NoteReason::UnknownKind);
            Err(())
        }
    }
}

pub fn map_timeline(rec: &mut Rec, media: &str, v: &Value, inst: &Instances) {
    let Some(list) = v.as_array() else {
        rec.note(KEY, None, NoteReason::BadShape);
        return;
    };
    if list.is_empty() {
        return;
    }
    let timeline = rec.iris().timeline(media);
    rec.link(media, "hasTimeline", &timeline);
    rec.class(&timeline, "Timeline");
    rec.link(&timeline, "inMedia", media);
    // Mise-en-scène tag to its variations and picks, across every Stage.
    let mut mise: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();

    for (n, e) in list.iter().enumerate() {
        let position = n.to_string();
        let Some(o) = e.as_object() else {
            rec.note(KEY, Some(position), NoteReason::BadShape);
            continue;
        };
        let id = match o.get("id") {
            None | Some(Value::Null) => {
                rec.note(KEY, Some(position), NoteReason::BadShape);
                continue;
            }
            Some(v) => match plugin_id(v) {
                Some(id) => id,
                None => {
                    rec.note(KEY, Some(position), NoteReason::InvalidId);
                    continue;
                }
            },
        };
        let entry = rec.iris().timeline_entry(media, &id);

        if o.get("type").and_then(Value::as_str) == Some("stage") {
            common(rec, &timeline, &entry, &id, n, "Stage");
            map_picks(rec, &entry, &id, o.get("miseEnScenePicks"), &mut mise);
            continue;
        }

        let Some(r) = o.get("ref").and_then(Value::as_object) else {
            rec.note(KEY, Some(id), NoteReason::BadShape);
            continue;
        };
        let Ok(target) = target(rec, media, r, inst, &id) else {
            continue;
        };
        common(rec, &timeline, &entry, &id, n, "Slot");
        let seconds = o.get("seconds").and_then(Value::as_f64);
        if let Some(s) = seconds {
            rec.add(&entry, "slotSeconds", decimal(s));
        }
        match o.get("markerId") {
            None | Some(Value::Null) => {}
            Some(m) => match stash_id(m) {
                Some(m) => {
                    let marker = rec.iris().marker(&m);
                    rec.link(&entry, "linkedMarker", &marker);
                }
                None => rec.note(KEY, Some(id.clone()), NoteReason::InvalidId),
            },
        }
        match target {
            Some(Target::Instance(i)) => {
                rec.link(&entry, "places", &i);
                if let Some(s) = seconds {
                    rec.add(&i, "atTime", decimal(s));
                }
            }
            Some(Target::Pairing(q)) => rec.link(&entry, "places", &q),
            None => {}
        }
    }

    for (tag, (variations, picks)) in mise {
        let node = rec.iris().mise_en_scene_instance(media, &tag);
        let t = rec.iris().tag(&tag);
        rec.link(media, "hasMiseEnScene", &node);
        rec.class(&node, "MiseEnSceneInstance");
        rec.link(&node, "inMedia", media);
        rec.link(&node, "ofTag", &t);
        for v in variations {
            rec.add(&node, "hasVariation", Term::string(v));
        }
        for k in picks {
            rec.link(&node, "fromPick", &k);
        }
    }
}

/// The facts every entry has.
fn common(rec: &mut Rec, timeline: &str, entry: &str, id: &str, n: usize, class: &str) {
    rec.link(timeline, "hasEntry", entry);
    rec.class(entry, class);
    rec.add(entry, "position", non_negative(n));
    rec.add(entry, "entryId", Term::string(id));
}

fn map_picks(
    rec: &mut Rec,
    entry: &str,
    id: &str,
    picks: Option<&Value>,
    mise: &mut BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)>,
) {
    let Some(picks) = picks.filter(|p| !p.is_null()) else {
        return;
    };
    let Some(picks) = picks.as_array() else {
        rec.note(KEY, Some(id.to_string()), NoteReason::BadShape);
        return;
    };
    for p in picks {
        let Some(tag) = p.get("tagId").and_then(stash_id) else {
            rec.note(KEY, Some(id.to_string()), NoteReason::InvalidId);
            continue;
        };
        let k = rec.iris().stage_pick(entry, &tag);
        let t = rec.iris().tag(&tag);
        rec.link(entry, "hasPick", &k);
        rec.class(&k, "StagePick");
        rec.link(&k, "ofTag", &t);
        rec.check_tag(KEY, Some(id.to_string()), &tag, TagFlag::MiseEnScene);
        let slot = mise.entry(tag).or_default();
        if let Some(v) = p
            .get("variation")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
        {
            rec.add(&k, "hasVariation", Term::string(v));
            slot.0.insert(v.to_string());
        }
        slot.1.insert(k);
    }
}
