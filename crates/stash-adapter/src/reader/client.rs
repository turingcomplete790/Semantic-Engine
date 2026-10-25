//! The library reader over Stash's GraphQL (spec 005 research R3 to R5, R7).
//!
//! A full read takes where it starts, runs one id-only discovery query per key, then reads the
//! merged ids in batches of [`RECORD_BATCH`]. The stop flag is checked before every request and
//! progress is reported after every one, so a stop takes effect before the next request.

use super::field::{flag_set, graph_position, parse_field, Field};
use super::keys::{
    GRAPH_POSITION, MEDIA_KEYS, PERFORMER_DISCOVERY, PERFORMER_TAG_FLAGS, TAG_DISCOVERY, TAG_KEYS,
};
use super::measurements::Measurements;
use super::queries::*;
use super::records::{
    id_order, sorted_ids, Kind, LibraryRead, MediaKind, MediaRecord, PerformerDisplay, Phase,
    Progress, ReadStart, RecordBatch, SemanticPerformer, SemanticTag, TagFlag,
};
use super::{LibraryReader, PERFORMER_TAG_CHUNK, RECORD_BATCH};
use crate::fixtures::{not_null, CriterionModifier};
use crate::{Client, Error, JsonMap};
use cynic::QueryBuilder;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

fn ids(list: &[String]) -> Option<Vec<cynic::Id>> {
    Some(list.iter().map(cynic::Id::new).collect())
}

fn object(v: JsonMap) -> Map<String, Value> {
    match v.0 {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn id_strings<'a>(it: impl IntoIterator<Item = &'a cynic::Id>) -> Vec<String> {
    sorted_ids(it.into_iter().map(|i| i.inner().to_string()).collect())
}

/// The keys of `table` present (and not null) in `cf`, parsed.
fn fields(
    cf: &Map<String, Value>,
    table: &[(&str, super::keys::Shape)],
) -> BTreeMap<String, Field> {
    table
        .iter()
        .filter_map(|(key, shape)| match cf.get(*key) {
            None | Some(Value::Null) => None,
            Some(v) => Some((key.to_string(), parse_field(*shape, v))),
        })
        .collect()
}

fn tag_record(r: TagRow) -> SemanticTag {
    let cf = object(r.custom_fields);
    SemanticTag {
        id: r.id.inner().to_string(),
        name: r.name,
        flags: TagFlag::ALL
            .into_iter()
            .filter(|f| cf.get(f.key()).is_some_and(flag_set))
            .collect(),
        parents: id_strings(r.parents.iter().map(|p| &p.id)),
        fields: fields(&cf, &TAG_KEYS),
        updated_at: r.updated_at.text(),
    }
}

fn performer_record(r: PerformerRow) -> SemanticPerformer {
    let cf = object(r.custom_fields);
    let present = |k: &str| cf.get(k).filter(|v| !v.is_null());
    SemanticPerformer {
        id: r.id.inner().to_string(),
        tags: id_strings(r.tags.iter().map(|t| &t.id)),
        archetype: present("isArchetype").is_some_and(flag_set),
        archetype_parents: present("archetypeParents")
            .map(|v| parse_field(super::keys::Shape::List, v)),
        measurements: r
            .measurements
            .filter(|m| !m.trim().is_empty())
            .map(|m| Measurements::parse(&m)),
        display: present(GRAPH_POSITION).map(|v| PerformerDisplay {
            graph_position: graph_position(v),
        }),
        updated_at: r.updated_at.text(),
    }
}

fn media_record(
    kind: MediaKind,
    id: &cynic::Id,
    performers: Vec<String>,
    tags: Vec<String>,
    cf: JsonMap,
    updated_at: &StashTime,
) -> MediaRecord {
    MediaRecord {
        kind,
        id: id.inner().to_string(),
        performers,
        tags,
        fields: fields(&object(cf), &MEDIA_KEYS),
        updated_at: updated_at.text(),
    }
}

/// Splits `requested` into the records found and the ids that are gone.
fn batch<T>(requested: &[String], mut found: Vec<T>, id: impl Fn(&T) -> &str) -> RecordBatch<T> {
    found.sort_by_key(|r| id_order(id(r)));
    let have: BTreeSet<&str> = found.iter().map(&id).collect();
    let missing = sorted_ids(
        requested
            .iter()
            .filter(|r| !have.contains(r.as_str()))
            .cloned()
            .collect(),
    );
    RecordBatch { found, missing }
}

impl Client {
    /// Checks the server's version once per client (FR-017). An old or unparseable version is
    /// refused; `stash_version()` itself stays unchecked, so `status` can report it.
    pub(crate) fn check_version(&self) -> Result<(), Error> {
        if self.version_checked.get().is_some() {
            return Ok(());
        }
        let found = self.stash_version()?.unwrap_or_default();
        if !crate::version::stash_supported(&found) {
            return Err(Error::UnsupportedVersion {
                found: if found.is_empty() {
                    "unknown".into()
                } else {
                    found
                },
                minimum: crate::version::MINIMUM_STASH_VERSION.into(),
            });
        }
        let _ = self.version_checked.set(());
        Ok(())
    }

    fn tag_ids(&self, filter: Option<TagIdFilter>) -> Result<Vec<String>, Error> {
        let page = self.run_read(TagIds::build(TagIdVars { filter }))?;
        Ok(id_strings(page.find_tags.tags.iter().map(|t| &t.id)))
    }

    fn performer_ids(&self, filter: Option<PerformerIdFilter>) -> Result<Vec<String>, Error> {
        let page = self.run_read(PerformerIds::build(PerformerIdVars { filter }))?;
        Ok(id_strings(
            page.find_performers.performers.iter().map(|p| &p.id),
        ))
    }

    /// Ids of scenes or images; `custom_fields` and `updated_at` as given.
    pub(crate) fn media_ids(
        &self,
        kind: MediaKind,
        custom_fields: Option<Vec<crate::fixtures::CustomFieldCriterion>>,
        updated_at: Option<TimestampCriterion>,
    ) -> Result<Vec<String>, Error> {
        Ok(match kind {
            MediaKind::Scene => {
                let filter =
                    (custom_fields.is_some() || updated_at.is_some()).then_some(SceneIdFilter {
                        custom_fields,
                        updated_at,
                    });
                let page = self.run_read(SceneIds::build(SceneIdVars { filter }))?;
                id_strings(page.find_scenes.scenes.iter().map(|s| &s.id))
            }
            MediaKind::Image => {
                let filter =
                    (custom_fields.is_some() || updated_at.is_some()).then_some(ImageIdFilter {
                        custom_fields,
                        updated_at,
                    });
                let page = self.run_read(ImageIds::build(ImageIdVars { filter }))?;
                id_strings(page.find_images.images.iter().map(|i| &i.id))
            }
        })
    }

    pub(crate) fn tag_rows(&self, list: &[String]) -> Result<Vec<SemanticTag>, Error> {
        let page = self.run_read(TagsByIds::build(IdsVars { ids: ids(list) }))?;
        Ok(page.find_tags.tags.into_iter().map(tag_record).collect())
    }

    pub(crate) fn performer_rows(&self, list: &[String]) -> Result<Vec<SemanticPerformer>, Error> {
        let page = self.run_read(PerformersByIds::build(IdsVars { ids: ids(list) }))?;
        Ok(page
            .find_performers
            .performers
            .into_iter()
            .map(performer_record)
            .collect())
    }

    pub(crate) fn media_rows(
        &self,
        kind: MediaKind,
        list: &[String],
    ) -> Result<Vec<MediaRecord>, Error> {
        let vars = IdsVars { ids: ids(list) };
        Ok(match kind {
            MediaKind::Scene => self
                .run_read(ScenesByIds::build(vars))?
                .find_scenes
                .scenes
                .into_iter()
                .map(|s| {
                    let performers = id_strings(s.performers.iter().map(|p| &p.id));
                    let tags = id_strings(s.tags.iter().map(|t| &t.id));
                    media_record(
                        kind,
                        &s.id,
                        performers,
                        tags,
                        s.custom_fields,
                        &s.updated_at,
                    )
                })
                .collect(),
            MediaKind::Image => self
                .run_read(ImagesByIds::build(vars))?
                .find_images
                .images
                .into_iter()
                .map(|i| {
                    let performers = id_strings(i.performers.iter().map(|p| &p.id));
                    let tags = id_strings(i.tags.iter().map(|t| &t.id));
                    media_record(
                        kind,
                        &i.id,
                        performers,
                        tags,
                        i.custom_fields,
                        &i.updated_at,
                    )
                })
                .collect(),
        })
    }

    pub(crate) fn latest(&self, kind: Kind) -> Result<Option<String>, Error> {
        Ok(match kind {
            Kind::Tag => self
                .run_read(LatestTag::build(()))?
                .find_tags
                .tags
                .first()
                .map(|t| t.updated_at.text()),
            Kind::Performer => self
                .run_read(LatestPerformer::build(()))?
                .find_performers
                .performers
                .first()
                .map(|p| p.updated_at.text()),
            Kind::Scene => self
                .run_read(LatestScene::build(()))?
                .find_scenes
                .scenes
                .first()
                .map(|s| s.updated_at.text()),
            Kind::Image => self
                .run_read(LatestImage::build(()))?
                .find_images
                .images
                .first()
                .map(|i| i.updated_at.text()),
        })
        .map(|t| t.filter(|t| !t.is_empty()))
    }
}

/// One full read in progress: the stop flag, progress, and the request count.
struct Run<'a> {
    stop: &'a dyn Fn() -> bool,
    progress: &'a mut dyn FnMut(Progress),
    requests: usize,
    phase: Phase,
    done: usize,
    total: usize,
}

impl Run<'_> {
    /// Sends one request, unless asked to stop first; then reports progress.
    fn step<T>(&mut self, send: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        if (self.stop)() {
            return Err(Error::Stopped);
        }
        let out = send()?;
        self.requests += 1;
        self.done += 1;
        (self.progress)(Progress {
            phase: self.phase,
            done: self.done.min(self.total),
            total: self.total,
        });
        Ok(out)
    }

    fn enter(&mut self, phase: Phase, total: usize) {
        self.phase = phase;
        self.done = 0;
        self.total = total;
    }
}

/// Discovery queries whose number is known before the read: 4 starts, the tag keys, the
/// performer keys plus `measurements`, and the media keys of both kinds.
const FIXED_DISCOVERY: usize =
    4 + TAG_DISCOVERY.len() + PERFORMER_DISCOVERY.len() + 1 + 2 * MEDIA_KEYS.len();

const MEDIA: [MediaKind; 2] = [MediaKind::Scene, MediaKind::Image];

impl Client {
    fn read_all(&self, run: &mut Run) -> Result<LibraryRead, Error> {
        run.enter(Phase::Discover, FIXED_DISCOVERY);

        // Where the read starts (FR-007).
        let mut start = ReadStart::new();
        for kind in Kind::ALL {
            if let Some(t) = run.step(|| self.latest(kind))? {
                start.insert(kind, t);
            }
        }

        // Discovery: one "key is not null" query per key (research R3).
        let mut tag_ids = Vec::new();
        let mut performer_tag_ids = Vec::new();
        for key in TAG_DISCOVERY {
            let found = run.step(|| {
                self.tag_ids(Some(TagIdFilter {
                    custom_fields: Some(not_null(key)),
                    updated_at: None,
                }))
            })?;
            if PERFORMER_TAG_FLAGS.contains(&key) {
                performer_tag_ids.extend(found.iter().cloned());
            }
            tag_ids.extend(found);
        }
        let tag_ids = sorted_ids(tag_ids);
        let performer_tag_ids = sorted_ids(performer_tag_ids);
        run.total += performer_tag_ids.len().div_ceil(PERFORMER_TAG_CHUNK);

        let mut performer_ids = Vec::new();
        for key in PERFORMER_DISCOVERY {
            performer_ids.extend(run.step(|| {
                self.performer_ids(Some(PerformerIdFilter {
                    custom_fields: Some(not_null(key)),
                    ..Default::default()
                }))
            })?);
        }
        performer_ids.extend(run.step(|| {
            self.performer_ids(Some(PerformerIdFilter {
                measurements: Some(StringCriterion {
                    value: String::new(),
                    modifier: CriterionModifier::NotNull,
                }),
                ..Default::default()
            }))
        })?);
        for chunk in performer_tag_ids.chunks(PERFORMER_TAG_CHUNK) {
            performer_ids.extend(run.step(|| {
                self.performer_ids(Some(PerformerIdFilter {
                    tags: Some(TagsCriterion {
                        value: ids(chunk),
                        modifier: CriterionModifier::Includes,
                    }),
                    ..Default::default()
                }))
            })?);
        }
        let performer_ids = sorted_ids(performer_ids);

        let mut media_ids: BTreeMap<MediaKind, Vec<String>> = BTreeMap::new();
        for kind in MEDIA {
            let mut found = Vec::new();
            for (key, _) in MEDIA_KEYS {
                found.extend(run.step(|| self.media_ids(kind, Some(not_null(key)), None))?);
            }
            media_ids.insert(kind, sorted_ids(found));
        }

        // Records, by id, in batches (research R4).
        let batches = |n: usize| n.div_ceil(RECORD_BATCH);
        run.enter(
            Phase::Records,
            batches(tag_ids.len())
                + batches(performer_ids.len())
                + media_ids.values().map(|v| batches(v.len())).sum::<usize>(),
        );
        let mut read = LibraryRead {
            start,
            ..LibraryRead::default()
        };
        let mut missing = |kind: Kind, n: usize| {
            if n > 0 {
                *read.missing.entry(kind).or_insert(0) += n;
            }
        };

        let mut tags = Vec::new();
        for chunk in tag_ids.chunks(RECORD_BATCH) {
            let b = batch(chunk, run.step(|| self.tag_rows(chunk))?, |t| &t.id);
            missing(Kind::Tag, b.missing.len());
            tags.extend(b.found);
        }
        tags.retain(|t| !t.flags.is_empty() || t.fields.contains_key("variantOfTagId"));
        let semantic_performer_tags: BTreeSet<&str> = tags
            .iter()
            .filter(|t| {
                t.flags
                    .iter()
                    .any(|f| PERFORMER_TAG_FLAGS.contains(&f.key()))
            })
            .map(|t| t.id.as_str())
            .collect();

        let mut performers = Vec::new();
        for chunk in performer_ids.chunks(RECORD_BATCH) {
            let b = batch(chunk, run.step(|| self.performer_rows(chunk))?, |p| &p.id);
            missing(Kind::Performer, b.missing.len());
            performers.extend(b.found);
        }
        performers.retain(|p| {
            p.archetype
                || p.archetype_parents.is_some()
                || p.measurements.is_some()
                || p.tags
                    .iter()
                    .any(|t| semantic_performer_tags.contains(t.as_str()))
        });

        let mut media: BTreeMap<MediaKind, Vec<MediaRecord>> = BTreeMap::new();
        for (kind, list) in &media_ids {
            let mut found = Vec::new();
            for chunk in list.chunks(RECORD_BATCH) {
                let b = batch(chunk, run.step(|| self.media_rows(*kind, chunk))?, |m| {
                    &m.id
                });
                missing((*kind).into(), b.missing.len());
                found.extend(b.found);
            }
            found.retain(|m| !m.fields.is_empty());
            media.insert(*kind, found);
        }

        read.tags = tags;
        read.performers = performers;
        read.scenes = media.remove(&MediaKind::Scene).unwrap_or_default();
        read.images = media.remove(&MediaKind::Image).unwrap_or_default();
        read.requests = run.requests;
        run.enter(Phase::Done, 0);
        (run.progress)(Progress {
            phase: Phase::Done,
            done: 0,
            total: 0,
        });
        Ok(read)
    }

    fn by_ids<T>(
        &self,
        list: &[String],
        fetch: impl Fn(&[String]) -> Result<Vec<T>, Error>,
        id: impl Fn(&T) -> &str + Copy,
    ) -> Result<RecordBatch<T>, Error> {
        self.check_version()?;
        let list = sorted_ids(list.to_vec());
        let mut out = RecordBatch {
            found: Vec::new(),
            missing: Vec::new(),
        };
        for chunk in list.chunks(RECORD_BATCH) {
            let b = batch(chunk, fetch(chunk)?, id);
            out.found.extend(b.found);
            out.missing.extend(b.missing);
        }
        Ok(out)
    }
}

impl LibraryReader for Client {
    fn full_read(
        &self,
        should_stop: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<LibraryRead, Error> {
        if should_stop() {
            return Err(Error::Stopped);
        }
        self.check_version()?;
        let mut run = Run {
            stop: should_stop,
            progress,
            requests: 0,
            phase: Phase::Discover,
            done: 0,
            total: 0,
        };
        self.read_all(&mut run)
    }

    fn tag_records(&self, list: &[String]) -> Result<RecordBatch<SemanticTag>, Error> {
        self.by_ids(list, |c| self.tag_rows(c), |t| &t.id)
    }

    fn performer_records(&self, list: &[String]) -> Result<RecordBatch<SemanticPerformer>, Error> {
        self.by_ids(list, |c| self.performer_rows(c), |p| &p.id)
    }

    fn media_records(
        &self,
        kind: MediaKind,
        list: &[String],
    ) -> Result<RecordBatch<MediaRecord>, Error> {
        self.by_ids(list, |c| self.media_rows(kind, c), |m| &m.id)
    }

    fn changed_since(&self, kind: Kind, threshold: &str) -> Result<Vec<String>, Error> {
        self.check_version()?;
        let after = Some(TimestampCriterion::after(threshold));
        match kind {
            Kind::Tag => self.tag_ids(Some(TagIdFilter {
                custom_fields: None,
                updated_at: after,
            })),
            Kind::Performer => self.performer_ids(Some(PerformerIdFilter {
                updated_at: after,
                ..Default::default()
            })),
            Kind::Scene => self.media_ids(MediaKind::Scene, None, after),
            Kind::Image => self.media_ids(MediaKind::Image, None, after),
        }
    }

    fn all_ids(&self, kind: Kind) -> Result<Vec<String>, Error> {
        self.check_version()?;
        match kind {
            Kind::Tag => self.tag_ids(None),
            Kind::Performer => self.performer_ids(None),
            Kind::Scene => self.media_ids(MediaKind::Scene, None, None),
            Kind::Image => self.media_ids(MediaKind::Image, None, None),
        }
    }

    fn latest_updated_at(&self, kind: Kind) -> Result<Option<String>, Error> {
        self.check_version()?;
        self.latest(kind)
    }
}
