# Tasks: Incremental Graph Update

**Input**: Design documents from `/specs/008-incremental-graph-update/`

**Prerequisites**:
- plan.md and spec.md;
- research.md (R1–R11) and data-model.md;
- the contracts: graph-store-format-2.md, reader-additions.md, operations-1.3.md;
- quickstart.md.

**Tests**: included. Constitution Principle VII requires tests first, run without Stash. Each
test task is written to fail before the code that makes it pass. Tests use spec 007's example
server id and fresh temporary store folders.

**Organization**: tasks are grouped by user story:
- **US1 (P1):** update with only what changed;
- **US2 (P1):** fall back to a full rebuild when an update cannot be trusted;
- **US3 (P2):** updates are as safe as builds;
- **US4 (P3):** reads stay bounded.

**Standing rules for every task:**
- **Navigate with graphify first:** run `graphify query`, `explain` or `path` (the CLI next to
  `graphify-out/.graphify_python`) before broad searches. Use `grep` for exact text only.
- **Equivalence is the oracle:** an updated graph must equal a full rebuild of the same library,
  fact for fact (FR-007). When a case is unclear, the full rebuild's answer is the right one.
- **No Oxigraph outside `engine::graph`** (G9). No value or credential in any log. Stash is only
  read.
- **The firewall:** a fresh test binary may first fail as "unreachable". Rerun before debugging.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US4

---

## Phase 1: Setup

- [X] T001 Create empty `crates/engine/src/build/stamp.rs` and
  `crates/engine/src/build/update.rs`, and declare them in `crates/engine/src/build/mod.rs`.
  Create `crates/engine/tests/common/update.rs`, and declare `pub mod update;` in
  `crates/engine/tests/common/mod.rs`.

---

## Phase 2: Foundational (blocks every story)

**Reader additions (contracts/reader-additions.md):**

- [X] T002 Write `crates/stash-adapter/tests/reader_semantic.rs` first:
  - **The rule:** `SemanticTag::is_semantic`, `SemanticPerformer::is_semantic(&performer_tags)`,
    `MediaRecord::is_semantic` and `performer_tag_set`. Cover:
    - a tag with flags, with only `variantOfTagId`, and with neither;
    - a performer that is an Archetype, has parents only, has measurements only, carries a Trait
      tag, carries a non-semantic tag;
    - media with and without fields.
  - **`semantic_tags()`:** on the fake Stash (`tests/common::library_handler`, `library.json`)
    and on `MemoryReader`, it returns exactly `full_read().tags`.
  - **`performers_with_tags(ids)`:** it returns the performers carrying any of the ids, sorted,
    in chunks of 500.
  - **`full_read`'s output is unchanged** on `library.json`.
- [X] T003 Implement the rule in `crates/stash-adapter/src/reader/records.rs`, as research R3
  states it:
  - a tag: "a flag is set, or `variantOfTagId` is present";
  - a performer: "an Archetype, archetype parents or measurements, or a tag in
    `performer_tags`";
  - media: "at least one semantic field is present".

  Make `read_all` in `crates/stash-adapter/src/reader/client.rs` filter through these functions
  instead of its inline `retain` closures.
- [X] T004 Add `semantic_tags` and `performers_with_tags` to the `LibraryReader` trait in
  `crates/stash-adapter/src/reader/mod.rs`:
  - **`Client`** (`crates/stash-adapter/src/reader/client.rs`): implement them by reusing the
    tag half of `read_all` (factor it out, so the full read and `semantic_tags` share it) and
    the INCLUDES performer query in `PERFORMER_TAG_CHUNK` chunks.
  - **`MemoryReader`** (`crates/stash-adapter/src/reader/memory.rs`): implement them from its
    records.
  - **`OnceReader`** in `crates/engine/src/bin/build-bench.rs`: return `Setup` errors.

  Record the additions in `specs/005-stash-adapter/contracts/reader-interface.md`. Make T002 pass.

**Store format 2 (contracts/graph-store-format-2.md):**

- [X] T005 Write `crates/engine/tests/graph_owned.rs` first:
  - `add_owned` of two owners, then `owners()`;
  - `replace_owned` with identical facts gives `Unchanged` and leaves the store clean (no
    `work.dirty`);
  - `replace_owned` with different facts gives `Changed { removed, added }`;
  - `remove_owned` returns the graph's size, and 0 for an unknown owner;
  - `owners_referencing(iri)`;
  - after each sequence, the published generation's `facts` equals a `SELECT (COUNT(*) …)`
    over the reader (G10);
  - `publish_with(json)` writes `build` into `snapshots/<n>.json`, and `Generation.build` reads
    it back;
  - a store stamped format 1 gives `NeedsRebuild(OtherFormat { found: 1, expected: 2 })`;
  - `select` sees facts in owner graphs and the default graph alike (union).
- [X] T006 Implement format 2 in `crates/engine/src/graph/`:
  - `STORE_FORMAT = 2` (`mod.rs`); `Owned` and `Replaced` (`mod.rs` or `term.rs`); owned-quad
    conversion with a graph name (`term.rs`);
  - in `writer.rs`:
    - `add_owned` (the bulk loader when the store is empty or the batch holds at least 10,000
      facts, as `add`);
    - `replace_owned`, which compares the graph's facts with the new ones and writes only on a
      difference, without touching `work.dirty` when unchanged;
    - `remove_owned`, `owners`, `owners_referencing`, `publish_with`;
    - **the kept count:** `Option<u64>`. It is 0 for `rebuild_writer`, the current generation's
      count for `writer`, and `None` after any default-graph `add`, `remove`, `remove_matching`
      or `clear`. `publish` uses `Store::len` only when it is `None`.
  - **`reader.rs`:** evaluate every query with the default graph as the union of all graphs.
  - **`layout.rs`:** `Generation.build: Option<Value>`, with `#[serde(default,
    skip_serializing_if = "Option::is_none")]`.

  Record the additions and G10–G13 in
  `specs/006-graph-store/contracts/graph-store-interface.md`. Make T005 and every existing graph
  test pass. Fix tests that hard-code format 1.
- [X] T007 Make the full build write owned graphs and the build stamp:
  - **`crates/engine/src/build/stamp.rs`:**
    - `MAPPING_VERSION: u32 = 1`;
    - `BuildStamp { mapping_version, vocabulary_version, catch_up: ReadStart, how }`, as JSON in
      the camelCase of research R7;
    - `BuildStamp::read(&Generation) -> Option<BuildStamp>`, where an unreadable or partial
      stamp gives `None`.
  - **`crates/engine/src/build/run.rs`:**
    - batch `Owned { owner: item IRI, facts }` per record, and the vocabulary as owner
      `urn:semantic-stash:vocab:`;
    - publish with `publish_with(stamp)`, where `catch_up` is `read.start` and `how` is
      `"build"`.

  Every spec 007 test must still pass unchanged. Add to `build_run.rs` a check that a build's
  generation carries a stamp with `mappingVersion` 1, and the fixture's `start`.
- [X] T008 [P] Write `crates/engine/tests/owners_disjoint.rs`: map the fixture library and
  `synth::library(200, 80)` record by record, and assert no fact is produced by two records
  (research R1). Also pin `MAPPING_VERSION` to a hash of the fixture's `expected.nt`: a changed
  expected graph without a bumped version fails, naming both.
- [X] T009 [P] Make `graph-bench build` in `crates/engine/src/bin/graph-bench.rs` write one owner
  graph per scene, image, performer and tag (`add_owned`), so its queries measure format 2.

**Update test support:**

- [X] T010 Write `crates/engine/tests/common/update.rs`:
  - **`Library`:** wraps a `LibraryRead` with a logical clock. Each mutation sets the item's
    `updated_at` to the next second in Stash's text form.
  - **Mutations:** `edit_scene_field`, `clear_scene_fields`, `delete(kind, id)`, `add_scene`,
    `add_performer`, `set_performer_tags`, `set_tag_flag`, `clear_tag_flag`, `edit_tag_field`,
    `merge_tags(source, destination)` (deletes the source, edits the destination), `add_tag`.
  - **`reader()`:** a `MemoryReader` built from the current state, with `with_ids` for items
    that exist but are not semantic.
  - **`CountingReader`:** wraps any `LibraryReader`, and counts each method call and the ids
    passed.
  - **`all_facts(&GraphStore) -> BTreeSet<Fact>`:** every fact of the published generation,
    through `SELECT ?s ?p ?o`.
  - **`rebuilt_facts(&Library) -> BTreeSet<Fact>`:** a full build of the same state into a
    fresh store, read the same way.

**Checkpoint:** spec 007's tests pass on format 2; the store and reader additions are tested.

---

## Phase 3: User Story 1 - Update the graph with only what changed (Priority: P1) 🎯 MVP

**Goal:** `engine::build::update` re-maps only what changed, and publishes a graph equal to a
full rebuild.

**Independent Test:** after each scripted change sequence, the updated graph equals a full
rebuild of the changed library.

### Tests for User Story 1

- [X] T011 [US1] Write `crates/engine/tests/update_equivalence.rs`. Run at least 20 sequences
  from research R8, each starting from a build of the fixture library. Run `update` after every
  step, and assert `all_facts == rebuilt_facts`. Name each sequence and print the first
  differing facts on failure. Include:
  - US1-1 to US1-6;
  - a tag gaining Trait while a performer carries it;
  - a parent Role losing its flag;
  - a relation's Label condition losing its flag;
  - a performer that stops being semantic;
  - a merge;
  - a scene added, then deleted, in one sequence;
  - three changes before one update.
- [X] T012 [P] [US1] Write `crates/engine/tests/update_current.rs`:
  - with no change, `update` gives `Mode::Current`, publishes nothing, and logs "the graph is
    current" (US1-7, FR-008);
  - a save that bumps `updated_at` but changes no fact gives the same;
  - a real change after that is still applied.

### Implementation for User Story 1

- [X] T013 [US1] Implement `UpdateReport`, `Mode` and `RebuildCause` in
  `crates/engine/src/build/update.rs`, as data-model § UpdateReport says, serialized as
  contracts/operations-1.3.md shows. Add `owner_kind(iri) -> Option<Kind>`.
- [X] T014 [US1] Implement `update(reader, store, server_id, options, progress, log) ->
  Result<UpdateReport, BuildError>` in `crates/engine/src/build/update.rs`, in research R5's
  order:
  1. take the new catch-up point;
  2. read every semantic tag; remove the graphs of tags gone from it; `replace_owned` every tag
     (spec 007 `map_tag` with a `Ctx` from the current tags);
  3. collect the tags whose graphs changed or were removed;
  4. performers: `changed_since(Performer, one_second_earlier(old point))`, plus
     `performers_with_tags(changed tags)`, plus `owners_referencing(tag IRI)` filtered to
     performer owners. Read them by id; `replace_owned` the semantic ones and `remove_owned` the
     rest and the missing; remove the owners not in `all_ids(Performer)`;
  5. scenes and images: likewise, without dependents;
  6. publish with `publish_with(stamp)` (`how: "update"`) only if a graph changed. Otherwise
     drop the writer and return `Current`.

  Report progress (reads up to 40%, re-mapping up to 90%, then publish). Log notes and the
  summary line as `run` does. Map the old catch-up point from the current generation's stamp.
  Make T011 and T012 pass.
- [X] T015 [US1] Expose `update` and its types in `crates/engine/src/build/mod.rs`.

**Checkpoint:** updates equal full rebuilds across every sequence.

---

## Phase 4: User Story 2 - An update that cannot be trusted rebuilds instead (Priority: P1)

**Goal:** every FR-010 condition gives a full rebuild, with its reason.

**Independent Test:** prepare the store in each condition, run `update`, and check that it
rebuilt and named the reason.

### Tests for User Story 2

- [X] T016 [US2] Write `crates/engine/tests/update_fallback.rs`. Each case below gives
  `Mode::Rebuild` with its cause, and the resulting graph equals a full rebuild:

  | Store prepared as | Cause |
  |---|---|
  | no store | `FirstBuild` |
  | damaged stamp | `StoreNeedsRebuild` with `cleared: "damaged"` |
  | format-1 stamp | `StoreNeedsRebuild` (`otherFormat`) |
  | a generation whose `build.mappingVersion` is 0 | `MappingVersion { from: 0, to: 1 }` |
  | `build.vocabularyVersion` "1.3.0" | `VocabularyVersion` |
  | a generation published without `build` | `NoCatchUpPoint` |
  | more than a third of the items changed | `TooManyChanges` |

  Rewrite the generation file in the test to set up each version case.

### Implementation for User Story 2

- [X] T017 [US2] In `crates/engine/src/build/update.rs`, check the fallback conditions before
  writing. On any of them, call `run` (the full build), and wrap its `BuildReport` as
  `Mode::Rebuild` with the cause.
  - **The share limit:** `UPDATE_SHARE_LIMIT` (one third), applied to
    `(changed + dependents) / owners`, over performers, scenes and images.
  - **`firstBuild` and `storeNeedsRebuild`:** come from `GraphStore::check()`.

  Make T016 pass.

**Checkpoint:** no update ever runs on a graph it cannot trust.

---

## Phase 5: User Story 3 - Updates are as safe as builds (Priority: P2)

**Goal:** stopped, failed and concurrent updates publish nothing and lose nothing.

**Independent Test:** kill updates at varied points. The store stays whole, and the next
update matches a full rebuild.

### Tests for User Story 3

- [X] T018 [P] [US3] Write `crates/engine/tests/update_kill.rs`, following `build_kill.rs`:
  - the child builds `synth::library(900, 300)`, changes 300 scenes, and updates;
  - the parent kills it 15 times at varied points, at least 3 after the "update: publishing"
    log line;
  - after each kill, the store is a whole generation (the count query equals `facts`), and a
    fresh in-process update reaches `rebuilt_facts`.

  Add an "update: publishing" log line before publishing in `update.rs`.
- [X] T019 [P] [US3] Add to `update_current.rs`, or a new `update_failures.rs`:
  - a held writer gives `BuildError::Busy`;
  - an empty server id gives `NoServerId`;
  - a reader failing with `Unreachable` publishes nothing, and the next update from a working
    reader applies every change (FR-013);
  - bad fields in a re-read scene give spec 007's notes.

### Implementation for User Story 3

- [X] T020 [US3] Wire `updateGraph` in `crates/plugin/src/build.rs`, with `update_graph`
  mirroring `build_graph`: settings, client, store, progress and log through the shared logger,
  and the envelope. Register it in `crates/plugin/src/ops.rs`.
- [X] T021 [US3] Add the "Update semantic graph" task to `plugin/semantic-engine.yml`, and
  `updateGraph` to `api/operations.json` and `docs/api.md`, at contract 1.3.0
  (`CONTRACT_VERSION` in `crates/engine/src/versions.rs`, and `crates/engine/tests/errors.rs`).
  Write `crates/plugin/tests/ops_update.rs`:
  - a first `updateGraph` rebuilds (`firstBuild`);
  - a second, with no change, is `current`;
  - a held lock gives `BUSY`;
  - no server id gives `SERVER_ID_NOT_GENERATED`;
  - the log has the summary line and no cookie.

  Make the plugin's `api_agreement` pass. Make T018–T021 pass.

**Checkpoint:** the update runs from Stash with the same guarantees as the build.

---

## Phase 6: User Story 4 - Reads stay bounded (Priority: P3)

**Goal:** an update's reads follow the changes; discovery's bounds are stated and measured.

**Independent Test:** the counting reader shows no record reads without changes, and record
reads growing only with changes and dependents.

### Tests for User Story 4

- [X] T022 [US4] Write `crates/engine/tests/update_reads.rs`, with `CountingReader` over
  `synth::library(2000, 500)`:
  - **0 changes:** no `*_records` calls; one `changed_since` and one `all_ids` per kind
    (performers, scenes, images); one `semantic_tags`.
  - **1 and 100 changed scenes:** `media_records` asks for exactly those ids.
  - **A tag flag change:** `performer_records` asks for the changed performers plus the tag's
    carriers only.
  - **Never:** `full_read` is never called unless `Mode::Rebuild`.

### Implementation for User Story 4

- [X] T023 [US4] (Nothing to fix: T022 passed on the first run.) Fix anything T022 finds in `crates/engine/src/build/update.rs`, for example
  re-reading items twice or reading dependents that are already in the changed set.

**Checkpoint:** bounded reads proven offline.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T024 Add `update --scenes N --images M --change K --dir DIR [--out FILE]` to
  `crates/engine/src/bin/build-bench.rs` (research R10):
  1. build the synthetic library (with a `MemoryReader`, since an update needs the change
     reads);
  2. apply K changes: a third edited, a third deleted, a third new scenes;
  3. time one update.

  Report the phases, the mode, the counts and the peak memory.
- [X] T025 Run `build-bench update --scenes 250000 --images 325000` with K = 0, 1, 100 and 1000
  on a disk-backed folder. Write `specs/008-incremental-graph-update/results/update-*.json`.
  Check against SC-002 and SC-003, and set `UPDATE_SHARE_LIMIT` from where an update stops being
  cheaper than a rebuild, if the third is far off.
- [X] T026 [P] Run `graph-bench build --scenes 250000` (format 2) and `graph-bench query --runs
  50`. Write `specs/008-incremental-graph-update/results/graph-query-250000.json`, and check the
  p95 is under 200 ms (006 SC-003 kept).
- [X] T027 Install on the test instance (`localhost:9998` only, with credentials in environment
  variables only), and run quickstart § 4:
  1. the first update rebuilds (`otherFormat`);
  2. edit a scene, a tag and a performer through the GraphQL API (`localhost:9998` only);
  3. update, and compare every fact with a "Build semantic graph" made into a copied plugin
     folder (or the same store after the comparison);
  4. update again: `current`.

  Restore the edited fields afterwards. Record the outcomes in
  `specs/008-incremental-graph-update/quickstart.md` § Results.
- [X] T028 Record the discovery bounds (FR-020) in `quickstart.md` § Results:
  - the request counts of `build-bench stash` on the test instance;
  - the owner's Production figures from 007 (1,220 records, read 201 ms).

  Ask the owner for a Production `build-bench stash` rerun if a request count is needed there.
- [X] T029 [P] Tick the two Phase 2 items in `ROADMAP.md` ("Bounded, flag-filtered discovery"
  and "Incremental build…") with their measured figures, and note that hook-triggered updates
  stay in Phase 3.
- [X] T030 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` and `vocab-check`, and the package check of 007 quickstart § 5.
  Update the graphify graph: `graphify update .` for code, then the semantic pass for the
  changed docs, linking the 008 doc nodes to the code nodes they describe.

---

## Dependencies & Execution Order

### Phase Dependencies

| Phase | Depends on |
|---|---|
| Setup (1) | nothing |
| Foundational (2) | Setup. It blocks every story: reader additions (T002–T004), store format 2 (T005–T006), the build on owned graphs (T007), test support (T010) |
| US1 (3) | Foundational |
| US2 (4) | US1's `update` (T014) |
| US3 (5) | US1. T020–T021 also need US2, so a first update rebuilds |
| US4 (6) | US1 |
| Polish (7) | all stories |

### Within Each Story

- Tests come first and fail.
- `update.rs` tasks (T013, T014, T017, T023) run in order, since they share one file.

### Parallel Opportunities

| Phase | Tasks that can run in parallel |
|---|---|
| Phase 2 | the reader tasks (T002–T004) and the store tasks (T005–T006); then T008 and T009 beside T007 |
| US1 | T012 beside T011 |
| US3 | T018 and T019 |
| Polish | T026 and T029 beside the others |

## Parallel Example: Foundational

```text
Reader:  T002 → T003 → T004
Store:   T005 → T006 → T007 → (T008, T009)
Then:    T010
```

## Implementation Strategy

### MVP First (User Story 1)

1. Phases 1 and 2: format 2, the reader's semantic rule, the build on owned graphs.
2. Phase 3: `update`, proven equal to a full rebuild.
3. **Stop and validate:** `update_equivalence` passes. The update is correct before it is wired
   to Stash.

### Incremental Delivery

1. **US1:** a correct update.
2. **US2:** trustworthy fallbacks.
3. **US3:** kill safety, and the task in Stash.
4. **US4:** bounded reads proven.
5. **Polish:** budgets at 250k, the test instance, the roadmap.
