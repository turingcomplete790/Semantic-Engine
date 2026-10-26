# Tasks: The Semantic Graph from the Documents

**Input**: Design documents from `/specs/007-graph-build/`

**Prerequisites**:
- plan.md and spec.md;
- research.md (R1–R12) and data-model.md;
- the contracts: mapping-rules.md, vocabulary-1.4.md, operations-1.2.md;
- quickstart.md.

**Tests**: included. Constitution Principle VII requires compatibility fixtures and tests first,
run without Stash (FR-022). Each test task is written to fail before the code that makes it pass.
Tests use the example server id `00000000-0000-4000-8000-000000000001` and fresh temporary store
folders.

**Organization**: tasks are grouped by user story:
- **US1 (P1):** every documented fact is in the graph;
- **US2 (P1):** build and publish the graph from Stash;
- **US3 (P2):** bad data never breaks the build;
- **US4 (P2):** the graph matches the documents, checked by fixtures.

**Standing rules for every task:**
- **The rules are the contract:** every fact follows `contracts/mapping-rules.md`. A fact not
  listed there is not written. If a task finds a case the contract does not cover, update the
  contract first, then the code.
- **No values in notes or logs:** a note holds kind, id, key, entry and reason only (research
  R8). No credential and no name ever reaches a log line.
- **No Oxigraph outside `engine::graph`:** the builder uses `Fact` and `Term` only (006 G9).
- **Stash is only read:** the only folder written is `{PluginDir}/graph`, or a temporary folder
  in tests.
- **The firewall:** a fresh test binary may first fail as "unreachable" because the firewall
  asks about it. Rerun before debugging.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US4

---

## Phase 1: Setup

- [X] T001 Create the module skeleton:
  - `crates/engine/src/build/mod.rs`, declaring `map`, `timeline`, `labels`, `notes`, `vocab`
    and `run`, each as an empty file of that name;
  - `pub mod build;` in `crates/engine/src/lib.rs`;
  - a stub `crates/engine/src/bin/build-bench.rs` that prints usage.
- [X] T002 [P] Create `crates/engine/tests/fixtures/build/README.md`. It explains the fixture
  set (data-model § Fixtures) and the bless variable `BUILD_FIXTURE_BLESS=1` (research R10),
  and has a table to fill in: mapping entry or note reason → the record that exercises it.

---

## Phase 2: Foundational (blocks every story)

**Vocabulary 1.4.0 (contracts/vocabulary-1.4.md):**

- [X] T003 Add `sem:pairsA`, `sem:pairsB` (both `rdfs:subPropertyOf sem:pairs`, domain
  `sem:Pairing`, range `sem:TagAssignment`) and `sem:roleAppliedAutomatically` (domain
  `sem:RoleInstance`, range `xsd:boolean`) to `vocabulary/core.ttl`. Give them the labels,
  sources, scope SceneScoped, status Active and since "1.4.0", as the contract gives them.
  - Replace the comments of `sem:pairs`, `sem:pairingDirection` and the status `sem:Derived`
    with the contract's text.
  - Extend the comments of `sem:bustSize`, `sem:waist`, `sem:hip`, `sem:waistToHipRatio`
    ("Stored by the builder…") and `sem:inStage` ("Not stored.").
  - Bump `sem:version` to "1.4.0".
- [X] T004 Update `vocabulary/stash-mapping.ttl`. Add the `rdfs:comment` lines to
  `sem:map_actionPairings`, `sem:map_roleTagPairings` and `sem:map_autoRoleAssignments`, as the
  contract gives them. `sem:map_roleTagPairings` keeps its existing comment, joined into one
  sentence pair.
- [X] T005 Add the 1.4.0 entry to `vocabulary/CHANGELOG.md`, with additions and clarifications,
  each with its reason from research R6. Then:
  - run `cargo run -p semantic-vocabulary --bin vocab-check -- --write-terms --write-snapshot`,
    which writes `vocabulary/TERMS.md` and `vocabulary/released/1.4.0.json`;
  - run `vocab-check` until it passes;
  - update `crates/semantic-vocabulary/tests/fixtures/expected-terms.txt` if its test lists
    terms.
- [X] T006 [P] Add `pub fn valid_id(id: &str) -> bool` to `crates/semantic-vocabulary/src/iri.rs`.
  It is the check `check_id` applies ("non-empty and contain only letters, digits, '-' or
  '_'"), and `check_id` uses it. Add unit tests for `""`, `"a b"`, `"a/b"`, `"se-pair-1"` and
  `"r_1"`.

**Store and term support (spec 006 additions):**

- [X] T007 [P] Derive `PartialOrd, Ord` on `Term` and `Fact` in `crates/engine/src/graph/term.rs`
  (research R9). Add `Fact::to_ntriples(&self) -> String`: one N-Triples line, with IRIs in
  `<…>`, literals with N-Triples escaping and `^^<datatype>` or `@lang`, `xsd:string` written
  plain, and blank nodes as `_:x`. Unit-test escaping of `"`, `\` and newlines.
- [X] T008 Write the test first, in `crates/engine/tests/graph_write_publish.rs`:
  `rebuild_writer` on a store with generation 1 starts with 0 facts; generation 1 stays current
  and readable until `publish()`; then generation 2 holds only the new facts. A second
  `rebuild_writer` while one is held gives `Busy`, and a store that needs a rebuild gives
  `NeedsRebuild`.
  - Then implement `GraphStore::rebuild_writer()` in `crates/engine/src/graph/mod.rs` and
    `crates/engine/src/graph/writer.rs` (research R3), in this order:
    1. the same checks as `writer()`;
    2. the lock;
    3. create `work.dirty`;
    4. remove `work/`;
    5. open an empty store at `work/`.
  - Add the method to `specs/006-graph-store/contracts/graph-store-interface.md`.

**Shared build types:**

- [X] T009 [P] Implement `crates/engine/src/build/notes.rs` (data-model § Note, § NoteReason):
  - **`Note`:** `{ kind: Kind, id: String, key: String, entry: Option<String>, reason:
    NoteReason }`.
  - **`NoteReason`:** `Unreadable`, `BadShape`, `InvalidId`, `UnknownKind`, `Retired`,
    `DanglingRef`, `TagMissing`, `TagWrongType`, `PerformerNotInMedia`, `NotRecordedRole`.
    It serializes camelCase and has `is_skip()`, which is true for the first six.
  - **`Notes`:** a collector that keeps notes in order found, and drops any note equal in
    (kind, id, key, entry, reason) to one already held.
  - **`Note::log_line()`:** for example `note: scene 12 roleTagAssignments [7]: tagWrongType`.
    It never includes a value.
  - Add unit tests.
- [X] T010 [P] Implement `crates/engine/src/build/vocab.rs`: `vocabulary_facts() -> Vec<Fact>`.
  It returns every triple of `core.ttl`, `labels.ttl` and `timeline.ttl` from
  `semantic_vocabulary::embedded()`, never `stash-mapping.ttl`, converted from `oxrdf` to
  `Term` (research R7). Unit-test that it holds
  `sem:bustSize sem:pluginFieldName "__derived_bust"` and
  `sem:pairsA rdfs:subPropertyOf sem:pairs`, and no subject `sem:map_*`.
- [X] T011 [P] Create the test helper `crates/engine/tests/common/build.rs`:
  - `load_library(path) -> LibraryRead`;
  - `iris() -> ItemIris` for the example server id;
  - `lines(&BTreeSet<Fact>) -> Vec<String>`: sorted N-Triples, excluding facts whose subject is
    in the `urn:semantic-stash:vocab:` namespace;
  - `diff(expected, actual) -> (missing, extra)`;
  - `assert_graph(expected_path, mapped)`. It writes the file instead when
    `BUILD_FIXTURE_BLESS=1`, and otherwise fails with every missing line and every extra line
    listed.

**Checkpoint:** vocabulary 1.4.0 passes `vocab-check`; `cargo test -p engine` passes.

---

## Phase 3: User Story 1 - Every documented fact is in the graph (Priority: P1) 🎯 MVP

**Goal:** `map_library` turns a library's records into exactly the facts of
`contracts/mapping-rules.md`.

**Independent Test:** map the hand-written fixture library. Its facts equal `expected.nt`, with
none missing and none extra.

### Tests for User Story 1

- [X] T012 [US1] Write `crates/engine/tests/fixtures/build/library.json`: a hand-written
  `LibraryRead` (spec 005 JSON) with only well-formed data. It must use every active mapping
  entry and every row of `contracts/mapping-rules.md` at least once. At least:
  - **Tags:**
    - one tag of each of the 8 flags;
    - a tag with two flags;
    - a Role under a Role, and a Relation under a Relation;
    - a parent that is not among the tags;
    - categories, variations, a common variant (no flags);
    - a Label with all four rule kinds, including `variationMatch` `__any__` and
      `__derived_bust`, and a threshold of 0;
    - a Variation with `variationData`;
    - Field Mapping on a Trait with base and two variation slots;
    - a Relation with ends and two rules (label and role conditions, actions).
  - **Performers:**
    - with Trait, Label and Variation tags and a non-semantic tag;
    - an Archetype with two parents;
    - one with measurements `34D-26-35`, and one with `32B` (waist and hip absent).
  - **Media:**
    - a scene with role, action (with and without variations), pluggable and automatic role
      assignments;
    - action and role pairings, one action pairing without a direction and one `give`;
    - relation assignments, automatic with a rule and manual;
    - a timeline with two Stages (the same Mise-en-scène tag with two variations, and a second
      tag), slots of all four ref kinds, with seconds and a `markerId`;
    - an image with a timeline slot without seconds;
    - two scenes where performer 7 is Dom in one and Sub in the other (US1-2);
    - a repeated role entry (FR-010).
  - **Also:** fill in the README table from T002.
- [X] T013 [US1] Write `crates/engine/tests/fixtures/build/expected.nt` by hand, from the rules,
  one line per fact, sorted, without the vocabulary facts. Do not bless it. Review each record's
  lines against `contracts/mapping-rules.md`, and note in the README any rule the fixture made
  you clarify.
- [X] T014 [P] [US1] Write `crates/engine/tests/build_fixture.rs`: map `library.json` and
  `assert_graph("expected.nt", …)` (SC-001). Add focused tests for the acceptance scenarios
  US1-1 to US1-7, each checking its facts by name:
  - the role instance's media, performer and tag;
  - no scene-crossing pair, by a SPARQL query on a published store built from the facts;
  - action variations;
  - Role type, parent and broader role;
  - the threshold and rules by id, with `__derived_bust` resolving to `sem:bustSize` by a query
    through `sem:pluginFieldName`;
  - the performer's Trait, Label, four measurements and Archetype parents;
  - every item IRI holding the server id.
- [X] T015 [P] [US1] Write `crates/engine/tests/build_determinism.rs`: map the fixture twice,
  and once with every record list reversed and every JSON map's entries in reverse order.
  All three fact sets must be equal (SC-007).

### Implementation for User Story 1

- [X] T016 [US1] In `crates/engine/src/build/map.rs`, implement:
  - **`TagIndex::from_tags`:** each tag id to its flags.
  - **The id helpers:**
    - `stash_id(&Value) -> Option<String>`: a string or JSON integer of decimal digits;
    - `plugin_id(&Value) -> Option<String>`: a string passing `iri::valid_id`.
  - **`Mapped { facts: BTreeSet<Fact>, notes: Notes }`**, with small fact-building helpers for
    `rdf:type`, IRI objects and typed literals (`xsd:decimal`, `xsd:integer`,
    `xsd:nonNegativeInteger`, `xsd:boolean`, plain strings).
  - **`map_tag`:** every row of mapping-rules § Tags except `labelFormula` and `fieldMapping`.
    `relationEnds` and `relationRules` are parsed through `engine::relations::defs`. Expose or
    reuse a parse function there rather than writing a second parser; if `defs` needs a
    `TagRecord`, convert the `SemanticTag`.
- [X] T017 [P] [US1] In `crates/engine/src/build/labels.rs`, implement:
  - `map_label_formula(tag, value, …)`: mapping-rules § Label formulas, with the threshold
    "the stored number when greater than 0, otherwise 100";
  - `map_field_mapping(tag, value, …)`: § Field Mapping, with the variation index taken from the
    sorted names of `variations`.
- [X] T018 [US1] In `crates/engine/src/build/map.rs`, implement `map_performer`
  (mapping-rules § Performers). Use the `Measurements` fields `bust_size`, `waist`, `hip` and
  `waist_to_hip_ratio`, and write each only when it is `Some`. Never read `display`.
- [X] T019 [US1] In `crates/engine/src/build/map.rs`, implement `map_media`:
  - **The item:** its type and `hasPerformer`.
  - **Assignments:** `roleTagAssignments`, `actionAssignments` (variations merged across
    repeated entries), `pluggableAssignments` and `autoRoleAssignments`.
  - **Pairings:** both pairing keys, with `pairsA`, `pairsB` and the direction default.
  - **Relations:** `relationAssignments`.
  - **The instance set:** keep the record's instances as a set of (kind, performer, tag), so
    pairings and the timeline can tell which instances exist.
- [X] T020 [P] [US1] In `crates/engine/src/build/timeline.rs`, implement `map_timeline`
  (mapping-rules § Timeline). It writes:
  - the Timeline node;
  - entries with `position` and `entryId`;
  - Stages with distinct picks per tag;
  - slots with `places`, `slotSeconds` and `linkedMarker`;
  - `atTime` on placed instances;
  - one Mise-en-scène instance per distinct picked tag, with every distinct variation and
    `fromPick` links.

  It never writes `inStage`.
- [X] T021 [US1] In `crates/engine/src/build/mod.rs`, expose:
  - `map_record` for each record kind;
  - `map_library(read: &LibraryRead, iris: &ItemIris) -> Mapped`. It builds the `TagIndex`,
    then maps tags, performers, scenes and images, in that order.

  Make T014 and T015 pass.

**Checkpoint:** `build_fixture` and `build_determinism` pass. The MVP mapping is complete.

---

## Phase 4: User Story 2 - Build and publish the graph from Stash (Priority: P1)

**Goal:** the "Build semantic graph" task reads, maps, writes and publishes one generation, with
progress, safe stopping, and the order of research R4.

**Independent Test:**
- with a `MemoryReader`, a build publishes a generation with the expected fact count;
- a killed build leaves the previous generation current;
- on the test instance, the task runs from Stash.

### Tests for User Story 2

- [X] T022 [P] [US2] Write `crates/engine/tests/build_run.rs`, with a `MemoryReader` over the
  fixture and a temporary store:
  - **First build:** generation 1, and the report's counts equal the fixture's (US2-1).
  - **Rebuild:** after removing a scene from the records, the deleted scene's facts are gone,
    and the generation holds exactly the new graph (US2-2).
  - **Damaged store:** with a corrupted stamp, it is cleared, and the report says `damaged`
    (US2-4).
  - **Busy:** a held writer gives `BuildError::Busy`, with no change (US2-5).
  - **No server id:** an empty server id gives `NoServerId` before any read (US2-6).
  - **Reader failure:** `MemoryReader::fail_with(Unreachable)` gives `Stash(_)`, and the
    previous generation stays current (US2-7).
  - **Progress:** progress values never decrease, start at or below 0.30 and end at 1.0, with
    at most 101 calls.
  - **Empty library:** it publishes an empty-but-built generation, distinct from "not built"
    (edge case).
  - **Vocabulary:** the published generation holds the vocabulary facts.
- [X] T023 [P] [US2] Write `crates/engine/tests/build_kill.rs` (research R2). The test re-runs
  its own binary with an environment variable that selects build mode. The child builds a large
  synthetic library (about 200,000 facts, so writing takes long enough) into the shared store.
  - **The kills:** the parent kills the child after a varied delay, 20 times. At least 3 of the
    kills must land during `publish()`: the child logs phase markers to stderr, and the parent
    waits for the "publish" marker before those kills.
  - **After each kill:** `current` names the generation published before the loop; a reader
    answers a count query from it; and the next uninterrupted build succeeds as generation
    n + 1.
- [X] T024 [P] [US2] Write `crates/plugin/tests/ops_build.rs`, with the existing fake Stash
  (`tests/common`) serving the reader fixture library:
  - `buildGraph` returns `ok` with the result shape of contracts/operations-1.2.md;
  - the stderr log has progress lines, the summary line and no API key;
  - with no `serverId` setting, it gives `SERVER_ID_NOT_GENERATED`, with the message naming
    "Generate server identifier";
  - with the store lock held by the test, it gives `BUSY`;
  - with the fake answering 401, it gives `INTERNAL`, with the message naming the refusal.

### Implementation for User Story 2

- [X] T025 [US2] Implement `crates/engine/src/build/run.rs`:
  - **The entry point:** `run(reader: &dyn LibraryReader, store: &GraphStore, server_id: &str,
    options: &BuildOptions, progress: &mut dyn FnMut(f64), log: &mut dyn FnMut(Level, &str))
    -> Result<BuildReport, BuildError>`. `Level` is a small engine enum: info and warning.
  - **The order of research R4:** server id, then health, then `clear_for_rebuild` (logging
    the reason), then `rebuild_writer`, then `full_read` (with `should_stop` always false),
    then mapping record by record, `add` in batches of `batch_facts` (50,000), then
    `vocabulary_facts`, then `publish`.
  - **Progress shares:** read 0–30% (`Progress::share`), map and write 30–90%, then 90–100%.
    Report only when the whole percent changes.
  - **The report:** fill `BuildReport` (data-model) with the phase times.
  - **Errors:** map them to `BuildError`.
  - Make T022 and T023 pass.
- [X] T026 [P] [US2] Add `ErrorCode::Busy` (`"BUSY"`) to `crates/engine/src/errors.rs`, and set
  `CONTRACT_VERSION` to `"1.2.0"` in `crates/engine/src/versions.rs`.
- [X] T027 [US2] Implement `crates/plugin/src/build.rs`, with `build_graph(input, logger)`:
  - read `serverId` through the settings store, as `ops.rs` does;
  - create a `stash_adapter::Client` from the server connection;
  - open `GraphStore::new({PluginDir}/graph, serverId)`;
  - call `engine::build::run`, sending progress as `Level::Progress` lines and log lines as
    info or warning;
  - return the envelope of contracts/operations-1.2.md.

  Map `BuildError`: `NoServerId` → `SERVER_ID_NOT_GENERATED`, with the message "no server
  identifier yet: run the "Generate server identifier" task first". `Stash(e)` maps through
  `relations::stash_error`. Register `("buildGraph", crate::build::build_graph)` in
  `crates/plugin/src/ops.rs`, and add `pub mod build;` to `crates/plugin/src/lib.rs`.
- [X] T028 [US2] Add the "Build semantic graph" task to `plugin/semantic-engine.yml`, with the
  contract's text. Add `buildGraph`, its result, its errors and `BUSY` to `api/operations.json`
  and `docs/api.md` at contract 1.2.0, and fix whatever the plugin's `api_agreement` test then
  reports. Make T024 pass.

**Checkpoint:** a build runs end to end offline, and the plugin answers `buildGraph`.

---

## Phase 5: User Story 3 - Bad data never breaks the build (Priority: P2)

**Goal:** every bad field or entry is skipped or noted on its own. The rest is mapped, and the
log is capped.

**Independent Test:** map a fixture with one bad field of each kind. Each is noted once, the
other fields of the same records are mapped, and nothing of the bad field reaches the graph.

### Tests for User Story 3

- [X] T029 [US3] Add bad records to `crates/engine/tests/fixtures/build/library.json`: at least
  one per note reason of research R8. Each sits in a record that also has good fields. Include:
  - unreadable `roleTagAssignments` (US3-1);
  - an assignment with performer id `"x7"`, and a tag id `"7 a"` (US3-2);
  - a slot with `ref.kind` `"camera-angle"` (US3-3), and a retired `mise-en-scene` ref;
  - a role assignment naming an Action tag, and one naming a tag not in the library (US3-4);
  - a pairing naming a missing instance;
  - an entry with no `id`;
  - a rule with no `id`;
  - an automatic role not in `roleTagAssignments`;
  - an assignment for a performer not on the scene;
  - a pairing direction `"sideways"`;
  - a pairing id `"a/b"` (refused by the IRI scheme);
  - a performer whose fields are all unreadable but who is on a scene (edge case);
  - a retired key, `isGeneric`, if the fixture's reader form can carry it.

  Add the good facts these records still give to `expected.nt` by hand, and fill in the
  README table.
- [X] T030 [US3] Write `crates/engine/tests/fixtures/build/expected-notes.json`, the full
  expected note list in camelCase. Write `crates/engine/tests/build_notes.rs`:
  - the notes equal the file exactly, each once (SC-006);
  - no note's log line contains any value from the fixture (scan for every tag name and
    every string value in the bad fields);
  - `build_fixture` still passes, which shows the rest of each record is mapped.

### Implementation for User Story 3

- [X] T031 [US3] Complete the note checks in `crates/engine/src/build/map.rs`,
  `crates/engine/src/build/labels.rs` and `crates/engine/src/build/timeline.rs`, so that every
  case of `contracts/mapping-rules.md` gives its note reason. Skip only the entry or key the
  note is about, never the record. Make T029 and T030 pass.
- [X] T032 [US3] In `crates/engine/src/build/run.rs`, log notes as warnings:
  - the first `note_log_cap` (1,000) notes, one line each;
  - then `notes: N more not written`;
  - then the summary line of research R8, with skips and mismatches counted by reason.

  Add a `build_run` test with `note_log_cap: 2` checking the cap line and the counts.

**Checkpoint:** bad data is noted, capped and never fatal.

---

## Phase 6: User Story 4 - The graph matches the documents, checked by fixtures (Priority: P2)

**Goal:** fixtures prove that every active mapping entry is exercised, and that the real test
library maps completely.

**Independent Test:** the fixture tests pass offline. Changing one rule on purpose makes them
fail, naming the fact.

### Tests for User Story 4

- [X] T033 [P] [US4] Write `crates/engine/tests/build_coverage.rs`. For each `sem:Mapping` with
  `sem:status sem:Active` in `semantic_vocabulary::embedded()`, its `sem:produces` term must
  appear in the mapped fixture graph, as a predicate or as the object of `rdf:type`. Also check
  every term named in "Also produces" in the entry's `rdfs:comment`. A failure lists the entry
  names (FR-020, SC-002).
- [X] T034 [P] [US4] Write `crates/engine/tests/build_test_instance.rs`. It maps
  `crates/stash-adapter/tests/fixtures/test-instance/library-read.json` and checks (FR-021,
  SC-003):
  - **Counts:** the counts of `sem:Scene`, `sem:Image` and `sem:Performer` subjects equal the
    record counts, and tag subjects equal the tag count.
  - **Complete instances:** every `RoleInstance`, `ActionInstance` and `PluggableInstance` has
    exactly one `inMedia`, one `forPerformer` and one `ofTag`.
  - **Defined terms:** every fact with an item-IRI subject uses a predicate, and an `rdf:type`
    object, that `Vocabulary::is_defined` accepts, or `rdf:type` or `rdfs:label`.
  - **Known facts:** scene 2's role instance for performer 7 and tag 19; the action instance's
    variation "making out"; the Mise-en-scène instance for tag 24 with "sunset" and "night";
    the Label 26 rules r1, r2 and r3.
  - **Notes:** the test pins them exactly. The captured library turned out to have two real
    problems, both on image 15 (a role for performer 8, who is not on the image, and a slot
    placing an action instance the image does not have), so "none of the skip kind" was
    replaced by "exactly these two".
- [X] T035 [US4] Check the failure path by hand, and record the result in the fixture README:
  1. temporarily drop `sem:hasVariation` from action instances in `map.rs`;
  2. confirm `build_fixture` fails, listing the missing lines;
  3. temporarily skip `variantOfTagId`, and confirm `build_coverage` fails, naming
     `sem:map_variantOfTagId`;
  4. revert both.

**Checkpoint:** all four stories pass offline.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T036 Implement `crates/engine/src/bin/build-bench.rs` (research R11):
  - **`synth --scenes N --images M --dir D [--out FILE]`:** generates a fully tagged
    `LibraryRead` in the test instance's shape. Every media item has role, action and pluggable
    assignments for 2 performers, one action and one role pairing, and a timeline with a Stage
    and 3 slots. Performers number one per 50 scenes, with Traits, Labels and measurements, and
    the tags of every flag. It builds through a `MemoryReader`.
  - **`stash --dir D [--out FILE]`:** reads `STASH_URL` and `STASH_API_KEY` from the
    environment and never prints them.
  - **Output:** both print a JSON report with phase times, counts and facts, and peak memory
    from `VmHWM` in `/proc/self/status` on Linux, or `null` elsewhere.
- [X] T037 [P] Add `--sparql FILE` to `graph-bench query` in
  `crates/engine/src/bin/graph-bench.rs`. It runs one `SELECT` against `--dir` and prints the
  rows as JSON (quickstart § 3, SC-008).
- [X] T038 [P] Extend the package check in `.github/workflows/ci.yml` to reject `build-bench`,
  and run the quickstart § 5 check locally.
- [X] T039 Run `build-bench synth --scenes 50000 --images 29000` in release mode, on a
  disk-backed folder. Write `specs/007-graph-build/results/build-synth.json`, and check it
  against "under 5 minutes" and "peak memory under 2 GB" (SC-004).
- [X] T040 Install the plugin on the test instance (`localhost:9998` only, with credentials from
  `Test-Instance/creds.txt` in environment variables only). Run quickstart § 3:
  1. the task from Stash;
  2. a second build while one runs (`BUSY`);
  3. Stop during a build: `current` unchanged;
  4. the SC-008 query, compared with the plugin's data read by `querySQL`.

  Record the outcomes and the summary line in `specs/007-graph-build/quickstart.md` § Results.
- [X] T041 Ask the owner to run `build-bench stash` against Production, read-only, and save the
  report as `specs/007-graph-build/results/build-production.json`. Record it against "under 5
  minutes".
- [X] T042 [P] Tick the three Phase 2 items in `ROADMAP.md`: "Vocabulary from the documents",
  "Data contract read exactly as documented" and "Compatibility fixtures".
- [X] T043 Add the results table and verdicts to `specs/007-graph-build/quickstart.md`. Run
  `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` and `vocab-check`. Fix what they report.
- [ ] T044 Update the graphify graph (`graphify-out/`) with the new code and docs. Code done
  2026-10-26 (`graphify update .`: 3,345 nodes); the semantic pass over the changed docs (specs
  006 and 007, docs/api.md, vocabulary) is left for the commit cycle.

---

## Dependencies & Execution Order

### Phase Dependencies

| Phase | Depends on |
|---|---|
| Setup (1) | nothing |
| Foundational (2) | Setup. It blocks every story: the vocabulary terms, `valid_id`, `Ord`/N-Triples, `rebuild_writer`, notes, vocabulary facts and the test helper |
| US1 (3) | Foundational |
| US2 (4) | Foundational and US1's `map_record`/`map_library` (T021). T022–T024 can be written alongside US1 |
| US3 (5) | US1 (it adds checks to the same mapping files). T032 also needs US2's `run.rs` |
| US4 (6) | US1. T034 is independent of US2 and US3 |
| Polish (7) | All stories. T040 and T041 need US2 |

### Within Each Story

- Tests are written first and fail.
- Within US1: `map.rs` tasks (T016, T018, T019) run in order. `labels.rs` (T017) and
  `timeline.rs` (T020) are separate files.
- The fixture library (T012) comes before `expected.nt` (T013), which comes before the tests
  that read it.

### Parallel Opportunities

| Phase | Tasks that can run in parallel |
|---|---|
| Phase 2 | T006, T007, T009, T010, T011, after the vocabulary tasks T003–T005 |
| US1 | T014 and T015 once the fixture exists; T017 and T020 beside T016–T019 |
| US2 | T022, T023 and T024; T026 beside T025 |
| US4 | T033 and T034 |
| Polish | T037, T038, T042 |

## Parallel Example: User Story 1

```text
After T012 and T013:
  T014 build_fixture.rs   T015 build_determinism.rs
While T016 → T018 → T019 run in map.rs:
  T017 labels.rs          T020 timeline.rs
```

## Implementation Strategy

### MVP First (User Story 1)

1. Phases 1 and 2: vocabulary 1.4.0, store and term support, notes, vocabulary facts.
2. Phase 3: the fixture, its expected graph, and the mapping.
3. **Stop and validate:** `build_fixture` equals `expected.nt`. The graph is defined and
   correct, even before any task runs it.

### Incremental Delivery

1. **US1:** the graph, mapped and checked offline.
2. **US2:** the build task, publishing from Stash, with kill safety.
3. **US3:** bad data noted and capped.
4. **US4:** coverage of every mapping entry, and the real test library.
5. **Polish:** budgets at Production size, the test instance from Stash, and the owner's
   Production run.
