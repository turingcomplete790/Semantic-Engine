//! The Apply and Re-apply task bodies against an in-memory store (spec 004 T026, T027).

use plugin::apply::{apply_items, apply_journal, Report, Store};
use plugin::journal::Journal;
use serde_json::{Map, Value};
use stash_adapter::relations::{ItemRecord, MediaKind, PerformerRecord, TagRecord};
use stash_adapter::Error;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../engine/tests/fixtures/relations")
            .join(name),
    )
    .unwrap()
}

/// Edits an item as it is read again just before a write.
type Reread = Box<dyn FnMut(&mut ItemRecord)>;

/// Stash as a map of scenes (no images); every write bumps the item's `updated_at` like Stash does.
struct Memory {
    tags: Vec<TagRecord>,
    performers: Vec<PerformerRecord>,
    items: RefCell<BTreeMap<String, ItemRecord>>,
    writes: RefCell<Vec<String>>,
    clock: RefCell<u32>,
    /// Called on each read of one item by id, to simulate an edit between read and write.
    on_reread: RefCell<Option<Reread>>,
}

impl Memory {
    fn new() -> Memory {
        let mut tags: Vec<TagRecord> = serde_json::from_str(&fixture("example1.json")).unwrap();
        tags.extend(serde_json::from_str::<Vec<TagRecord>>(&fixture("example2.json")).unwrap());
        let items: Vec<ItemRecord> = serde_json::from_str(&fixture("scenes.json")).unwrap();
        Memory {
            tags,
            performers: serde_json::from_str(&fixture("performers.json")).unwrap(),
            items: RefCell::new(items.into_iter().map(|i| (i.id.clone(), i)).collect()),
            writes: RefCell::new(Vec::new()),
            clock: RefCell::new(0),
            on_reread: RefCell::new(None),
        }
    }
    fn item(&self, id: &str) -> ItemRecord {
        self.items.borrow()[id].clone()
    }
    fn field(&self, id: &str, key: &str) -> Value {
        let item = self.item(id);
        match item.custom_fields.get(key) {
            Some(Value::String(s)) => serde_json::from_str(s).unwrap(),
            _ => Value::Null,
        }
    }
    fn all_ids(&self) -> Vec<(MediaKind, String)> {
        self.items
            .borrow()
            .keys()
            .map(|id| (MediaKind::Scene, id.clone()))
            .collect()
    }
}

impl Store for Memory {
    fn definitions(&self) -> Result<Vec<TagRecord>, Error> {
        Ok(self.tags.clone())
    }
    fn items(&self, kind: MediaKind, ids: &[String]) -> Result<Vec<ItemRecord>, Error> {
        let mut out = Vec::new();
        if kind != MediaKind::Scene {
            return Ok(out);
        }
        for id in ids {
            let mut item = self.items.borrow().get(id).cloned();
            if ids.len() == 1 {
                if let (Some(f), Some(i)) = (self.on_reread.borrow_mut().as_mut(), item.as_mut()) {
                    f(i);
                    self.items.borrow_mut().insert(id.clone(), i.clone());
                }
            }
            out.extend(item);
        }
        Ok(out)
    }
    fn performer_tags(&self, ids: &[String]) -> Result<Vec<PerformerRecord>, Error> {
        Ok(self
            .performers
            .iter()
            .filter(|p| ids.contains(&p.id))
            .cloned()
            .collect())
    }
    fn changed_since(&self, kind: MediaKind, threshold: &str) -> Result<Vec<String>, Error> {
        if kind != MediaKind::Scene {
            return Ok(Vec::new());
        }
        Ok(self
            .items
            .borrow()
            .values()
            .filter(|i| i.updated_at.as_str() > threshold)
            .map(|i| i.id.clone())
            .collect())
    }
    fn latest_updated_at(&self, kind: MediaKind) -> Result<Option<String>, Error> {
        if kind != MediaKind::Scene {
            return Ok(None);
        }
        Ok(self
            .items
            .borrow()
            .values()
            .map(|i| i.updated_at.clone())
            .max())
    }
    fn ids_with_relation_data(&self, kind: MediaKind) -> Result<Vec<String>, Error> {
        if kind != MediaKind::Scene {
            return Ok(Vec::new());
        }
        Ok(self.items.borrow().keys().cloned().collect())
    }
    fn update(&self, _: MediaKind, id: &str, partial: Map<String, Value>) -> Result<(), Error> {
        let mut t = self.clock.borrow_mut();
        *t += 1;
        let mut items = self.items.borrow_mut();
        let item = items.get_mut(id).unwrap();
        item.custom_fields.extend(partial);
        item.updated_at = format!("2026-10-21T11:00:{:02}-06:00", *t);
        self.writes.borrow_mut().push(id.to_string());
        Ok(())
    }
}

fn run(store: &Memory, items: &[(MediaKind, String)]) -> Report {
    apply_items(store, items, &mut |_| {}).unwrap()
}

#[test]
fn applying_every_scene_writes_only_what_changes_and_a_rerun_writes_nothing() {
    let store = Memory::new();
    let first = run(&store, &store.all_ids());
    assert_eq!(first.items, 12);
    // 101, 102, 103, 104, 106, 108, 110, 111 gain relations; 112 loses its stale one.
    assert_eq!(first.changed, 9, "{first:?}");
    assert_eq!(
        store.field("101", "relationAssignments"),
        serde_json::json!([{ "relationTagId": "45", "a": "7", "b": "8", "auto": true, "ruleId": "r1" }])
    );
    store.writes.borrow_mut().clear();
    let again = run(&store, &store.all_ids());
    assert_eq!(again.changed, 0, "{again:?}");
    assert!(store.writes.borrow().is_empty());
}

#[test]
fn a_stopped_run_completed_later_gives_the_same_result() {
    let full = Memory::new();
    run(&full, &full.all_ids());
    let partial = Memory::new();
    let ids = partial.all_ids();
    run(&partial, &ids[..5]);
    run(&partial, &ids);
    for (_, id) in &ids {
        assert_eq!(
            partial.item(id).custom_fields,
            full.item(id).custom_fields,
            "scene {id}"
        );
    }
}

#[test]
fn an_item_edited_between_read_and_write_is_evaluated_again() {
    let store = Memory::new();
    // Someone removes the pairing just before the write: nothing must be applied.
    *store.on_reread.borrow_mut() = Some(Box::new(|i: &mut ItemRecord| {
        i.custom_fields
            .insert("actionPairings".into(), Value::String("[]".into()));
        i.updated_at = "2026-10-21T10:30:00-06:00".into();
    }));
    let r = run(&store, &[(MediaKind::Scene, "101".to_string())]);
    assert_eq!(r.changed, 0, "{r:?}");
    assert!(store.writes.borrow().is_empty());
}

#[test]
fn unreadable_items_are_skipped_and_counted() {
    let store = Memory::new();
    store
        .items
        .borrow_mut()
        .get_mut("101")
        .unwrap()
        .custom_fields
        .insert("roleTagAssignments".into(), Value::String("{broken".into()));
    let r = run(&store, &[(MediaKind::Scene, "101".to_string())]);
    assert_eq!((r.changed, r.skipped), (0, 1));
}

#[test]
fn the_apply_task_drains_the_journal_and_advances_the_watermark() {
    let dir = std::env::temp_dir().join(format!("se-apply-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let journal = Journal::new(&dir);
    journal.append(MediaKind::Scene, "101").unwrap();
    journal.mark_pending().unwrap();
    let store = Memory::new();
    let r = apply_journal(&store, &journal, &mut |_| {}).unwrap();
    // Only the journaled scene: there was no watermark, so no catch-up yet.
    assert_eq!((r.items, r.changed), (1, 1));
    assert!(journal.drain().unwrap().is_empty());
    assert!(journal.mark_pending().unwrap(), "the marker was cleared");
    let w = journal.watermark().unwrap();
    assert_eq!(w[&MediaKind::Scene], "2026-10-21T10:00:00-06:00");

    // Next run: the catch-up threshold is one second before the watermark, so it finds every
    // scene saved in that second or later (all 12 fixtures) and applies the 8 not yet done.
    let r = apply_journal(&store, &journal, &mut |_| {}).unwrap();
    assert_eq!((r.items, r.changed), (12, 8), "{r:?}");
    // Third run: only the scenes written since, all already up to date.
    let r = apply_journal(&store, &journal, &mut |_| {}).unwrap();
    assert_eq!(r.changed, 0, "{r:?}");
    assert!(r.items >= 1 && r.items < 12, "{r:?}");
}
