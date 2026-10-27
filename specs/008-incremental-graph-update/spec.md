# Feature Specification: Incremental Graph Update

**Feature Branch**: `master` (spec directory `008-incremental-graph-update`)

**Created**: 2026-10-26

**Status**: Draft

**Input**: User description: "finish up phase 2". The two open Phase 2 items of the roadmap:
- **Incremental build** with progress and cancellation; the store rebuilt when missing, damaged,
  or from an older format.
- **Bounded, flag-filtered discovery** (constitution Principle VII).

## Context

Spec 007 builds the semantic graph, but only as a full rebuild: every run reads the whole
library and publishes a new graph. That is fine for a first build, and fast at today's sizes
(40 s for a fully tagged library of Production size). It is wasteful when a handful of scenes
changed, and it gets slower as the library grows.

This feature adds an **update**: a run that reads only what changed since the last build or
update, changes only those items' facts, and publishes a new generation. When an update cannot
be trusted, it falls back to a full rebuild on its own:
- the store is missing, damaged, from an older format, or from another server;
- the graph was built by different mapping rules or another vocabulary version;
- there is no record of when the graph was last brought up to date.

It also closes the discovery item: it states the bounds the reads already keep, and adds the
one they still need, that an update reads in proportion to what changed.

**Out of scope: hooks.** Starting an update automatically after each save is the roadmap's
Phase 3 item "Hook-driven incremental updates". This feature delivers the update that such a
hook will start, and keeps it safe to run at any time, by hand or by a later hook.

**Sources:**
- the 003 decision record, rules 7, 8 and 13 (catch-up, deletions, apply budget), and § 3
  Freshness (merges fire only on the destination; `updated_at` is server-local text, compared to
  the second);
- spec 005's change reads (`changed_since`, `all_ids`, records by id);
- spec 006's store (`writer`, `remove_matching`, generations);
- spec 007's mapping rules and build task.

## User Scenarios & Testing *(mandatory)*

The users of this feature are:
- **The owner:** runs the update, or the full rebuild, from Stash's task list.
- **The engine's later features:** Phase 3's hooks and queries rely on a graph that is current
  and equal to what a full rebuild would give.

### User Story 1 - Update the graph with only what changed (Priority: P1)

The owner runs "Update semantic graph". The engine:
1. finds the tags, performers, scenes and images that changed since the graph was last brought
   up to date;
2. re-reads only those;
3. replaces their facts;
4. removes the facts of items that were deleted, or that no longer carry semantic data;
5. publishes one new generation.

The job shows progress, and its log says how many items were changed, added and removed.

**Why this priority**: This is the incremental build. Without it, every change to the library
costs a full rebuild.

**Independent Test**: Build a library, change it in each way the library can change, run the
update, and compare the result with a full rebuild of the changed library. They are identical.

**Acceptance Scenarios**:

1. **Given** a built graph and one scene whose role assignments changed, **When** the update
   runs, **Then** only that scene is re-read, and the new generation equals a full rebuild of
   the changed library.
2. **Given** a scene deleted from Stash, **When** the update runs, **Then** every fact of that
   scene, including its instances, pairings and timeline, is gone.
3. **Given** a scene whose semantic fields were all cleared, **When** the update runs, **Then**
   it leaves the graph, as a full rebuild would leave it out.
4. **Given** a new performer tagged with a Trait, **When** the update runs, **Then** the
   performer and its Trait link are in the graph.
5. **Given** a tag that gains the Role flag, **When** the update runs, **Then** every fact that
   depends on that flag also changes: a child Role's broader role, the Trait, Label or
   Variation links of performers carrying it, and relation rules naming it.
6. **Given** two tags merged in Stash, **When** the update runs, **Then** the merged-away tag's
   facts are gone, and the destination tag's facts are current.
7. **Given** no change since the last update, **When** the update runs, **Then** no new
   generation is published, and the log says the graph is current.

---

### User Story 2 - An update that cannot be trusted rebuilds instead (Priority: P1)

The update falls back to a full rebuild, and says why in the log, when:
- the store needs a rebuild (missing, damaged, other format, other server);
- the graph was built by other mapping rules or another vocabulary version;
- the record of the last build or update is missing or unreadable.

The owner can also run "Build semantic graph" at any time to force a full rebuild, as today.

**Why this priority**: An update applied on top of a graph built by other rules would mix two
mappings, and nobody could tell. Falling back keeps the graph equal to a full rebuild in every
case.

**Independent Test**: For each condition, prepare the store that way, run the update, and check
that it rebuilt in full and named the reason.

**Acceptance Scenarios**:

1. **Given** no graph yet, **When** the update runs, **Then** it builds the whole graph and
   reports "first build".
2. **Given** a graph built by an older engine whose mapping or vocabulary version differs,
   **When** the update runs, **Then** it rebuilds in full and names both versions.
3. **Given** a damaged store, **When** the update runs, **Then** it clears and rebuilds, as the
   full build does.
4. **Given** the record of the last update is unreadable, **When** the update runs, **Then** it
   rebuilds in full and says so.

---

### User Story 3 - Updates are as safe as builds (Priority: P2)

An update follows the same rules as a full build:
- it publishes nothing when stopped or when it fails;
- it ends at once when another writer holds the store;
- it ends at once without a server id;
- it never writes to Stash;
- bad fields in re-read items are noted, as in a full build.

**Why this priority**: The update changes a published graph in place, piece by piece. A
stopped or failed update must never leave a mix of old and new facts where readers can see it.

**Independent Test**: Kill updates at varied points and check that the published generation is
always the previous whole one, and that the next update still reaches the same graph as a full
rebuild.

**Acceptance Scenarios**:

1. **Given** an update stopped part-way, **When** readers query, **Then** they see the previous
   generation, and the next update applies every change, including those the stopped one had
   begun.
2. **Given** an update running, **When** a build or another update starts as an operation,
   **Then** it ends at once with `BUSY`.
3. **Given** Stash fails during an update, **When** it ends, **Then** nothing is published, the
   failure is named, and the next update catches up from the same point.

---

### User Story 4 - Reads stay bounded (Priority: P3)

The engine's reads from Stash stay bounded:
- **discovery** is filtered by the plugin's keys and flags, so its cost follows the number of
  semantic items, not the library's size;
- **an update** reads in proportion to what changed: one change query per kind, the changed
  records, and one id list per kind for deletions;
- **requests** go out in batches, whatever the library's size.

The roadmap's discovery item is recorded as met, with the measured figures.

**Why this priority**: Principle VII requires bounded work. The reads already keep most of
these bounds; this story states them, measures them, and adds the update's.

**Independent Test**: Count the requests an update sends for 0, 1 and 1,000 changed items on a
library of Production size, and check that they grow with the changes only, apart from the id
lists.

**Acceptance Scenarios**:

1. **Given** a library of Production size and no changes, **When** the update runs, **Then** it
   sends only the change queries and the id lists, and no record reads.
2. **Given** 1,000 changed scenes, **When** the update runs, **Then** the record reads cover
   those scenes and the items their facts depend on, in batches.

---

### Edge Cases

- **A change during an update:** an item saved while an update runs is caught by the next
  update. The point an update catches up from is taken before its first read, as the full
  build's is.
- **Many saves in the same second:** Stash compares `updated_at` to the second, so the update
  reads from one second before the last point. Items re-read twice give the same facts.
- **A changed item no longer semantic:** a scene whose semantic fields were all cleared is
  still in Stash, but leaves the graph.
- **A deleted tag still referenced:** scene data that names a deleted tag keeps its facts, as a
  full rebuild keeps them. The tag's own facts go, and the build notes it as a missing tag.
- **Facts that depend on other items:**
  - a tag's flags decide its children's broader links, performers' Trait, Label and
    Variation links, and whether relation rules naming it hold;
  - the update re-maps the items whose facts depend on a changed tag, as well as the tag.
- **Merged tags:** a merge fires no change on the merged-away tag. The id comparison finds it
  gone.
- **A huge change:** when most of the library changed, an update may take longer than a full
  rebuild. It falls back to a full rebuild when the changed items exceed a stated share of the
  library.
- **Clock and time zone:** the catch-up point is stored exactly as Stash wrote it, never
  converted. The 003 spike found that a UTC threshold matched none of 1,000 changed scenes.
- **An update after a stopped full build:** the stopped build published nothing, so the record
  of the last good point is the previous one, and the update starts from there.

## Requirements *(mandatory)*

### Functional Requirements

**The update**

- **FR-001**: The plugin MUST offer a Stash task, "Update semantic graph". It brings the
  published graph up to date with the library, reading only what changed since the graph was
  last brought up to date, and publishes one new generation.
- **FR-002**: The update MUST find changes per kind (tags, performers, scenes, images) from
  Stash's `updated_at`. It reads from one second before the last catch-up point, in Stash's own
  time text (003 decision record rule 7).
- **FR-003**: The update MUST find deletions per kind by comparing Stash's ids with the items in
  the graph (rule 8). This includes tags merged away.
- **FR-004**: For each changed item, the update MUST remove that item's facts and write its new
  facts, so the result is what the mapping rules of spec 007 give for its current data.
- **FR-005**: An item that was deleted, or no longer carries semantic data, MUST lose all its
  facts. That includes the facts of its instances, pairings, timeline, rules and slots.
- **FR-006**: When a tag changes or is removed, the update MUST also re-map the items whose
  facts depend on that tag's flags:
  - its child tags (broader links);
  - the performers that carry it (Trait, Label and Variation links);
  - the relations whose ends or rules name it (spec 004 drops a rule whose condition is not a
    Label or Role).
- **FR-007**: After any sequence of changes, the graph an update publishes MUST equal, fact for
  fact, the graph a full rebuild of the same library gives.
- **FR-008**: When nothing changed, the update MUST publish no new generation, and MUST say so
  in the log.
- **FR-009**: The update MUST take its new catch-up point before its first read, and record it
  only with the generation it publishes.

**Falling back to a full rebuild**

- **FR-010**: The update MUST rebuild in full, and log why, when:
  - the store needs a rebuild;
  - the published generation was built with a different mapping version or vocabulary version
    than the running engine;
  - the catch-up point is missing or unreadable;
  - the changed items exceed a stated share of the library.
- **FR-011**: Every published generation MUST record its catch-up point per kind, the mapping
  version and the vocabulary version it was built with.
- **FR-012**: "Build semantic graph" MUST stay available as a forced full rebuild, and MUST
  record the same catch-up point, so updates can follow it.

**Safety (as spec 007)**

- **FR-013**: A stopped or failed update MUST publish nothing. The previous generation stays
  current, and the next update starts from the previous catch-up point.
- **FR-014**: The update MUST end at once with `BUSY` when another writer holds the store, and
  with `SERVER_ID_NOT_GENERATED` when there is no server id.
- **FR-015**: The update MUST report progress to Stash, and MUST note bad fields of re-read
  items as spec 007 does, within the same log cap.
- **FR-016**: The update MUST only read Stash.

**Reports and contract**

- **FR-017**: When it ends, the update MUST log one summary line. The line gives:
  - whether it updated or rebuilt, and why it rebuilt;
  - the items changed, added and removed, per kind;
  - the facts removed and added, and the generation published;
  - the notes by reason, and the time taken.
- **FR-018**: The operations contract MUST record the new task and its result, as an additive
  change.

**Bounded reads (Principle VII)**

- **FR-019**: An update's reads MUST be:
  - one change query per kind;
  - the changed records and their dependents, read by id in batches;
  - one id list per kind.

  No update may run discovery over the whole library unless it falls back to a full rebuild.
- **FR-020**: The bounds of the full read's discovery MUST be stated and measured: requests per
  semantic item, batch sizes, and time at Production size. The roadmap's discovery item records
  them.

**Budgets (Principle VII)**

- **FR-021**: The update MUST state and measure its time by a repeatable command, at 250,000
  scenes, for 0, 1, 100 and 1,000 changed items. Publishing is part of the update, and its time
  is measured on its own.

### Key Entities

- **Catch-up point**: per kind, the newest `updated_at` Stash held when a build or update
  began, in Stash's own text. It is stored with the generation it led to.
- **Build stamp**: what a generation was made by: the mapping version, the vocabulary version,
  and the catch-up point. A mismatch with the running engine forces a full rebuild.
- **Change set**: for one update, per kind, the items changed, added and removed, and the
  dependents to re-map.
- **Update report**: what an update did. It holds:
  - whether it updated or rebuilt, and why;
  - the change set's counts, and the facts removed and added;
  - the generation, the notes and the time.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After each of at least 20 scripted change sequences, the updated graph equals a
  full rebuild of the same library, fact for fact. The sequences cover adds, edits, deletions,
  cleared fields, flag changes and merges, for every kind.
- **SC-002**: On a library of 250,000 scenes, an update applying 1 changed scene completes in
  under 5 seconds end to end, and one applying 1,000 changed scenes in under 60 seconds.
- **SC-003**: An update with no changes publishes nothing, and completes in under 2 seconds at
  Production size.
- **SC-004**: In 100% of trials, an update stopped at a varied point publishes nothing, and the
  next update reaches the same graph as a full rebuild.
- **SC-005**: In 100% of the fallback conditions of FR-010, the update rebuilds in full and
  names the reason.
- **SC-006**: An update with no changes sends no record reads. The number of record reads
  grows with the changed items and their dependents only.
- **SC-007**: On the test instance, after the owner edits a scene, a tag and a performer in
  Stash, running "Update semantic graph" gives the same graph as "Build semantic graph".

## Assumptions

- **Hooks are Phase 3.** This feature adds no save hook. The update is a task the owner, or a
  later hook, runs. The relations hook of spec 004 is unchanged.
- **One writer.** Updates and builds share the store's single writer lock (spec 006).
  Stash's job queue runs tasks one at a time, so in practice only operation calls can meet
  `BUSY` (spec 007 results).
- **Mapping version.** The engine gains a mapping version, bumped whenever the mapping rules of
  spec 007 change what any record gives. It is recorded with each generation, and a change
  forces a rebuild.
- **The share for falling back** is set by the plan from measurements. The default, until
  measured, is a third of the library's semantic items.
- **Contract:** one new task, so the contract moves to 1.3.0, a MINOR, additive change.
- **Publishing at 250k:** 006 measured about 3.8 s per publish at 250,000 scenes, mostly from
  counting every fact. SC-002's 5-second budget for one changed scene needs that cost cut. The
  plan states how; for example, the count kept from the previous generation plus the change.
- **Discovery is already flag-filtered.** Spec 005's reader finds semantic items by their keys
  and flags, and reads records in batches. Its Production read took 201 ms for 1,220 semantic
  records. This feature measures and records those bounds rather than changing them.
