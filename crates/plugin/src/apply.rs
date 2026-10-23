//! The "Apply relations" and "Re-apply relations" task bodies (spec 004 FR-007 to FR-011;
//! research R5 to R9). Both run as Stash background jobs, never inside a save or a client call.
//!
//! Items are read in batches of 100 with their performers' tags, evaluated, and written only when
//! their automatic relations or roles change. Just before a write the item is read again; if it
//! changed meanwhile it is evaluated again, so a concurrent edit is not overwritten with stale
//! results. A stopped run leaves each item either old or new, and a rerun completes it.

use crate::journal::{one_second_earlier, Journal};
use engine::relations::diff::plan;
use engine::relations::evaluate::{matches, Labels};
use engine::relations::Model;
use serde_json::{Map, Value};
use stash_adapter::relations::{ItemRecord, MediaKind, PerformerRecord, TagRecord, BATCH};
use stash_adapter::{Client, Error};
use std::collections::{BTreeMap, BTreeSet};

/// The Stash reads and writes the tasks need; the real implementation is the adapter's client.
pub trait Store {
    fn definitions(&self) -> Result<Vec<TagRecord>, Error>;
    fn items(&self, kind: MediaKind, ids: &[String]) -> Result<Vec<ItemRecord>, Error>;
    fn performer_tags(&self, ids: &[String]) -> Result<Vec<PerformerRecord>, Error>;
    fn changed_since(&self, kind: MediaKind, threshold: &str) -> Result<Vec<String>, Error>;
    fn latest_updated_at(&self, kind: MediaKind) -> Result<Option<String>, Error>;
    fn ids_with_relation_data(&self, kind: MediaKind) -> Result<Vec<String>, Error>;
    fn update(&self, kind: MediaKind, id: &str, partial: Map<String, Value>) -> Result<(), Error>;
}

impl Store for Client {
    fn definitions(&self) -> Result<Vec<TagRecord>, Error> {
        self.relation_definitions()
    }
    fn items(&self, kind: MediaKind, ids: &[String]) -> Result<Vec<ItemRecord>, Error> {
        self.items_by_ids(kind, ids)
    }
    fn performer_tags(&self, ids: &[String]) -> Result<Vec<PerformerRecord>, Error> {
        Client::performer_tags(self, ids)
    }
    fn changed_since(&self, kind: MediaKind, threshold: &str) -> Result<Vec<String>, Error> {
        Client::changed_since(self, kind, threshold)
    }
    fn latest_updated_at(&self, kind: MediaKind) -> Result<Option<String>, Error> {
        Client::latest_updated_at(self, kind)
    }
    fn ids_with_relation_data(&self, kind: MediaKind) -> Result<Vec<String>, Error> {
        Client::ids_with_relation_data(self, kind)
    }
    fn update(&self, kind: MediaKind, id: &str, partial: Map<String, Value>) -> Result<(), Error> {
        self.update_item_custom_fields(kind, id, partial)
    }
}

/// What a run did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Items evaluated.
    pub items: usize,
    /// Items written.
    pub changed: usize,
    /// Items not evaluated because a field could not be read.
    pub skipped: usize,
    /// Items not written because their rule chain was still changing after the pass limit.
    pub unstable: usize,
    /// Messages for the job log.
    pub notes: Vec<String>,
}

impl Report {
    fn add(&mut self, o: Report) {
        self.items += o.items;
        self.changed += o.changed;
        self.skipped += o.skipped;
        self.unstable += o.unstable;
        self.notes.extend(o.notes);
    }

    pub fn summary(&self, what: &str, seconds: f64) -> String {
        format!(
            "{what}: {} items, {} changed, {} skipped, {} unstable, {seconds:.1} s",
            self.items, self.changed, self.skipped, self.unstable
        )
    }
}

fn labels_for(store: &dyn Store, items: &[ItemRecord]) -> Result<Labels, Error> {
    let ids: BTreeSet<String> = items
        .iter()
        .flat_map(|i| i.performers.iter().cloned())
        .collect();
    let ids: Vec<String> = ids.into_iter().collect();
    Ok(store
        .performer_tags(&ids)?
        .into_iter()
        .map(|p| (p.id, p.tags.into_iter().collect()))
        .collect())
}

/// Evaluates one item and writes its change, re-reading it first (see the module docs).
fn apply_one(
    store: &dyn Store,
    model: &Model,
    labels: &Labels,
    item: &ItemRecord,
    r: &mut Report,
) -> Result<(), Error> {
    let mut current = item.clone();
    // At most one re-evaluation after a concurrent edit; a second edit waits for the next run.
    for attempt in 0..2 {
        let eval = match matches(&current, model, labels) {
            Ok(e) => e,
            Err(e) => {
                r.skipped += 1;
                r.notes.push(format!(
                    "{} {}: skipped, {e}",
                    current.kind.as_str(),
                    current.id
                ));
                return Ok(());
            }
        };
        if !eval.stable {
            r.unstable += 1;
            r.notes.push(format!(
                "{} {}: rule chain still changing; not written",
                current.kind.as_str(),
                current.id
            ));
            return Ok(());
        }
        let patch = match plan(&current, &eval, model) {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(()),
            Err(e) => {
                r.skipped += 1;
                r.notes.push(format!(
                    "{} {}: skipped, {e}",
                    current.kind.as_str(),
                    current.id
                ));
                return Ok(());
            }
        };
        let fresh = store.items(current.kind, std::slice::from_ref(&current.id))?;
        let Some(fresh) = fresh.into_iter().next() else {
            return Ok(()); // deleted meanwhile
        };
        if fresh.updated_at != current.updated_at {
            if attempt == 0 {
                current = fresh;
                continue;
            }
            r.notes.push(format!(
                "{} {}: changed during the run; left for the next run",
                current.kind.as_str(),
                current.id
            ));
            return Ok(());
        }
        store.update(current.kind, &current.id, patch.partial())?;
        r.changed += 1;
        return Ok(());
    }
    Ok(())
}

/// Evaluates and writes the given items, in batches of 100, calling `progress` with the share
/// done after each batch.
pub fn apply_items(
    store: &dyn Store,
    items: &[(MediaKind, String)],
    progress: &mut dyn FnMut(f64),
) -> Result<Report, Error> {
    let model = Model::from_tags(&store.definitions()?);
    apply_with(store, &model, items, progress)
}

fn apply_with(
    store: &dyn Store,
    model: &Model,
    items: &[(MediaKind, String)],
    progress: &mut dyn FnMut(f64),
) -> Result<Report, Error> {
    let mut by_kind: BTreeMap<MediaKind, Vec<String>> = BTreeMap::new();
    for (k, id) in items {
        by_kind.entry(*k).or_default().push(id.clone());
    }
    let total = items.len().max(1) as f64;
    let mut done = 0usize;
    let mut r = Report::default();
    for (kind, ids) in by_kind {
        for chunk in ids.chunks(BATCH) {
            let batch = store.items(kind, chunk)?;
            let labels = labels_for(store, &batch)?;
            for item in &batch {
                r.items += 1;
                apply_one(store, model, &labels, item, &mut r)?;
            }
            done += chunk.len();
            progress(done as f64 / total);
        }
    }
    Ok(r)
}

const KINDS: [MediaKind; 2] = [MediaKind::Scene, MediaKind::Image];

/// The "Apply relations" task: drains the journal, catches up items changed since the last
/// successful run (from the `updated_at` watermark), applies them, and loops while hooks keep
/// adding entries.
pub fn apply_journal(
    store: &dyn Store,
    journal: &Journal,
    progress: &mut dyn FnMut(f64),
) -> Result<Report, Error> {
    let io = |e: std::io::Error| Error::Setup(format!("journal: {e}"));
    let model = Model::from_tags(&store.definitions()?);
    let mut total = Report::default();
    loop {
        let mut marks = BTreeMap::new();
        for k in KINDS {
            if let Some(t) = store.latest_updated_at(k)? {
                marks.insert(k, t);
            }
        }
        let mut items = journal.drain().map_err(io)?;
        let mut seen: BTreeSet<(MediaKind, String)> = items.iter().cloned().collect();
        for (k, w) in journal.watermark().map_err(io)? {
            let Some(threshold) = one_second_earlier(&w) else {
                total.notes.push(format!(
                    "watermark {w:?} is not a Stash time; catch-up skipped"
                ));
                continue;
            };
            for id in store.changed_since(k, &threshold)? {
                if seen.insert((k, id.clone())) {
                    items.push((k, id));
                }
            }
        }
        total.add(apply_with(store, &model, &items, progress)?);
        journal.set_watermark(&marks).map_err(io)?;
        if !journal.finish().map_err(io)? {
            return Ok(total);
        }
    }
}

/// The "Re-apply relations" task: every item of `kinds` that can hold relations, or only `ids`.
pub fn reapply(
    store: &dyn Store,
    kinds: &[MediaKind],
    ids: Option<&[String]>,
    progress: &mut dyn FnMut(f64),
) -> Result<Report, Error> {
    let mut items = Vec::new();
    for k in kinds {
        let list = match ids {
            Some(ids) => ids.to_vec(),
            None => store.ids_with_relation_data(*k)?,
        };
        items.extend(list.into_iter().map(|id| (*k, id)));
    }
    apply_items(store, &items, progress)
}
