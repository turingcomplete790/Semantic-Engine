# Tasks: Stash Adapter, the Library Reader

**Input**: Design documents from `/specs/005-stash-adapter/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R10), data-model.md,
contracts/reader-interface.md, contracts/vocabulary-terms.md, quickstart.md, plugin-notes.md

**Tests**: included. Constitution Principle VII requires tests first, run without a live Stash
against recorded fixtures. Each test task is written to fail before the code that makes it pass.
Live checks against the test instance (`http://localhost:9998`) are opt-in (`--ignored`) or
quickstart steps.

**Organization**: tasks are grouped by user story:
- **US1 (P1):** full read of the semantic library;
- **US2 (P1):** change reads, record reads and id listings, with spec 004 rebuilt on them;
- **US3 (P2):** distinct, credential-free failures;
- **US4 (P2):** offline testing: `MemoryReader` and fixture capture.

**Standing rules for every task:**
- **Credentials:** read the test instance's API key from `../Test-Instance/creds.txt` into an
  environment variable only. Never print it or write it to a file.
- **Live access:** live access is read-only, except for the spike's `seed` and `cleanup` on the
  test instance.
- **Typed requests:** every new request is a Cynic typed operation against
  `schemas/stash-v0.31.1.graphql`. There are no hand-built query strings in crate code.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US4

---

## Phase 1: Setup

- [X] T001 Create the module skeleton:
  - `crates/stash-adapter/src/reader/mod.rs`, declaring `keys`, `field`, `measurements`,
    `records`, `queries`, `client` and `memory` (empty modules);
  - `pub mod reader;` in `crates/stash-adapter/src/lib.rs`;
  - an empty `crates/stash-adapter/src/version.rs`, registered in `lib.rs`;
  - a `[[bin]] reader-bench` entry in `crates/stash-adapter/Cargo.toml`, with a stub
    `crates/stash-adapter/src/bin/reader-bench.rs` that prints usage.

  `cargo build --workspace` passes.
- [X] T002 [P] Write recorded GraphQL response fixtures in `crates/stash-adapter/tests/fixtures/reader/`,
  with a `README.md` saying how each was obtained.
  - **Sources:** shapes are taken from read-only queries against the test instance, trimmed to a
    small library.
  - **Library contents:**
    - 6 semantic tags, one of them a `variantOfTagId` variant with no flag, with `fieldMapping`
      on a Trait;
    - 4 performers: an Archetype with `archetypeParents` and `archetypeGraphPosition`, a
      Trait-tagged performer, a performer with only `measurements`, and a performer with no
      semantic data, who is referenced by a scene but must not be read;
    - 3 scenes and 2 images, one of them with a field holding invalid JSON text, and one with a
      list where an object is expected.
  - **Responses:**
    - one id-discovery response per key query of research R3;
    - record batches by ids;
    - one "latest `updated_at`" response per kind;
    - `version` responses for `v0.31.1`, `v0.30.0` and `nonsense`.
- [X] T003 [P] Build the measurements parity set:
  1. Write `crates/stash-adapter/tests/fixtures/reader/gen-measurements.mjs`. It contains the
     plugin's `parseMeasurements` and the four `compute` functions of `DERIVED_PERFORMER_FIELDS`,
     copied verbatim from `../semantic-tagging/src/ui/matrix.js` at commit `2c04141`, with a
     header comment citing the commit and lines.
  2. The script reads `measurements-input.json` (a list of strings) and writes
     `measurements.json` (`[{measurements, bust, waist, hip, whr}]`, `null` where the plugin
     gives null).
  3. The inputs are at least 30 strings:
     - 20 distinct real values from the test instance, read with one read-only
       `findPerformers(performer_filter: {measurements: {value: "", modifier: NOT_NULL}})`;
     - the edge cases: `36DD`, `34D½-24-34`, `34DD.5-25-35`, `34D 1/2-24-34`, `32DDD-22-33`,
       `32DDDD-24-34`, `34Z-24-34`, `34-24-34`, `0D-24-34`, `natural`, an empty string,
       `34D-0-34`, `34D-24-0`, ` 34c - 26 - 36 ` and `30AA-30-30`.
  4. Run `node gen-measurements.mjs` once, and commit the inputs, the output and the script.
     Record the Node version in the README.

---

## Phase 2: Foundational (blocks every story)

- [X] T004 Move version parsing into the adapter:
  - **In the adapter:** `parse_stash_version` and `MINIMUM_STASH_VERSION = "v0.31.1"` go into
    `crates/stash-adapter/src/version.rs`, together with `stash_supported`.
  - **In the engine:** `crates/engine/src/versions.rs` re-exports them, and keeps
    `STASH_MINIMUM_VERSION` as an alias, so existing callers compile unchanged.
  - **Tests:** move the existing parsing tests to `crates/stash-adapter/tests/version.rs`.
    `cargo test --workspace` passes.
- [X] T005 [P] Write the key table of research R1 in `crates/stash-adapter/src/reader/keys.rs`.
  - **Keys and kinds:** `TAG_FLAGS` (8 flags), `TAG_KEYS`, `PERFORMER_KEYS` and `MEDIA_KEYS`, each
    entry `(key, Kind)`, where `Kind` is `Flag`, `List`, `Object` or `Id`:
    - `categories`, `variations`, `relationRules`, `archetypeParents`, `actionPairings`,
      `roleTagPairings`, `relationAssignments` and `timeline` are `List`;
    - `labelFormula`, `variationData`, `relationEnds`, `fieldMapping`, `archetypeGraphPosition`,
      `roleTagAssignments`, `actionAssignments`, `pluggableAssignments` and
      `autoRoleAssignments` are `Object`;
    - `variantOfTagId` is `Id`.
  - **Discovery keys:**
    - tags: the 8 flags plus `variantOfTagId`;
    - performers: `isArchetype` and `archetypeParents`;
    - media: the 8 media keys.
  - **Never read:** a const `NOT_READ` lists `isGeneric`, `looks`, `archetypeRecipe`,
    `directTraitVariations`, `inheritedVariations` and `inheritedTraitVariations`, so a test can
    assert that none of them is ever selected.
- [X] T006 [P] Write failing tests in `crates/stash-adapter/tests/reader_fields.rs` for field
  parsing (data-model "Field"):
  - **Values:** JSON text that parses to the expected kind gives `Field::Value`. A JSON value
    stored directly gives `Field::Value`. Invalid JSON gives
    `Field::Unreadable { raw, reason }`, with the raw text kept. The wrong top-level kind gives
    `Unreadable`.
  - **Ids:** an `Id` given as a string or as an integer is kept as a string.
  - **Flags:** `1`, `"1"`, `true` and `"true"` are set. `0`, `"0"`, `false`, `"yes"`, `null` and
    `2` are not set, and none of them is an error (FR-014).
  - **Display data:** `archetypeGraphPosition` `{"x":1.5,"y":-2}` gives a position. A
    position with a missing `y`, a string `x`, or invalid JSON gives `None`, never an error
    (FR-002a).
- [X] T007 Implement `crates/stash-adapter/src/reader/field.rs` (`Field`, `parse_field(kind,
  value)`, `flag_set(value)`, `graph_position(value)`) until T006 passes.
  - **004 flag rule:** `flag_set` reuses 004's rule. Move the existing helper into this file and
    keep `engine::relations::defs::flag_set` delegating to it, so there is one copy.
- [X] T008 [P] Write failing tests in `crates/stash-adapter/tests/reader_measurements.rs`:
  - **Parity:** every case of `fixtures/reader/measurements.json` gives the same `bust_size`,
    `waist`, `hip` and `waist_to_hip_ratio` as the plugin, compared with an exact equality on
    the f64 values (SC-010). `null` means absent.
  - **Raw text:** `raw` always equals the input.
  - **Unparseable text:** it gives every derived value absent, and no error.
- [X] T009 Implement `crates/stash-adapter/src/reader/measurements.rs` as
  `Measurements::parse(raw: &str) -> Measurements`, following research R2 exactly, until T008
  passes:
  - the pattern;
  - DD, DDD and DDDD read as E, F and G;
  - the half-cup markers `½`, `.5` and `1/2`;
  - the cup steps AA 0 to L 17;
  - the ×2.5 multiplier;
  - the waist bonus `(28 − waist) × 2` when the waist is given and under 28;
  - Waist-to-Hip Ratio as waist ÷ hip, rounded with `(x * 100.0).round() / 100.0`, only when
    both are non-zero.

  Cite `matrix.js` at `2c04141` in the module docs.
- [X] T010 Implement the record types of data-model.md in
  `crates/stash-adapter/src/reader/records.rs`:
  - `SemanticTag`, `TagFlag`, `SemanticPerformer`, `Measurements` (re-exported),
    `PerformerDisplay`, `MediaRecord`, `LibraryRead`, `ReadStart`, `RecordBatch<T>` and
    `Progress` with `share()`;
  - `Kind` (`Scene`, `Image`, `Performer`, `Tag`), with `From<MediaKind>`.

  **Moving `MediaKind`:** move it here from `relations.rs`, and re-export it from `relations` so
  004's paths still compile.

  **Rules to keep:**
  - every list is "sorted by numeric id";
  - `share()` counts discovery as the first 5%, and never decreases;
  - all types derive `Serialize` and `Deserialize`, so a `LibraryRead` round-trips through JSON.

  Add a round-trip unit test.
- [X] T011 Define the `LibraryReader` trait in `crates/stash-adapter/src/reader/mod.rs`, exactly
  as in contracts/reader-interface.md:
  - `full_read`, `tag_records`, `performer_records`, `media_records`, `changed_since`,
    `all_ids` and `latest_updated_at`;
  - the constants `RECORD_BATCH = 250` and `PERFORMER_TAG_CHUNK = 500`.

  Add `Error::Stopped` to `crates/stash-adapter/src/lib.rs`. The remaining error variants come in
  US3.

**Checkpoint**: fields, measurements and records are parsed and tested. The trait compiles. No
request is sent yet.

---

## Phase 3: User Story 1, full read of the semantic library (Priority: P1) 🎯 MVP

**Goal**: one call returns every semantic tag, performer, scene and image, each once, with
progress and stopping (FR-001 to FR-008, FR-002a, FR-002b, FR-015).

**Independent Test**: `cargo test -p stash-adapter --test reader_full --test reader_stop`
passes against the fake Stash serving the T002 fixtures.

### Tests for User Story 1 ⚠️

- [X] T012 [P] [US1] Write failing tests in `crates/stash-adapter/tests/reader_full.rs`. They use
  `tests/common::fake_stash` with a handler that answers by operation name from the T002
  fixtures. They assert:
  - **Tags:** every semantic tag, including the flagless variant found by `variantOfTagId`, and
    no other tag. Flags, parents and parsed fields are set, with `fieldMapping` parsed (FR-001).
  - **Performers:**
    - the Archetype (with `display.graph_position`), the Trait-tagged performer, and the
      measurements-only performer with derived values are present;
    - the performer who is only referenced is absent;
    - no request selects a key from `NOT_READ` (FR-002, FR-002a, FR-002b).
  - **Media:** scenes and images with performer ids, native tag ids, parsed fields and
    `updated_at`. The invalid-JSON field is `Unreadable`, and the rest of that record is intact
    (FR-003, FR-013).
  - **Start first:** the "latest `updated_at`" requests come before the first discovery request,
    and `start` holds all four kinds (FR-007).
  - **Each item once:** an id that two discovery queries both return appears once. An id whose
    record batch omits it is counted in `missing`, not returned (FR-008).
  - **Fixed request count:** it equals the discovery queries plus ⌈ids ÷ 250⌉ per kind. No
    request has a single id when more ids are pending (no N+1, FR-005).
  - **Bodies:** every discovery request filters on a custom field with `NOT_NULL`, or on tags
    with `INCLUDES`, or on `measurements` with `NOT_NULL`. None of them is unfiltered
    (FR-004).
- [X] T013 [P] [US1] Write failing tests in `crates/stash-adapter/tests/reader_stop.rs` (FR-006,
  SC-009). With a library of 600 scenes, where the fake server generates batches:
  - **Progress:** it is reported after every discovery query and every batch. `share()` never
    decreases, and the last call has `Phase::Done`.
  - **Stopping:** if `should_stop` turns true after batch 2, no request for batch 3 is sent, and
    the result is `Err(Error::Stopped)`.
  - **Stop before start:** a stop before the first request sends nothing.

### Implementation for User Story 1

- [X] T014 [US1] Write the typed Cynic operations in `crates/stash-adapter/src/reader/queries.rs`:
  - **Tag discovery:** ids of tags where a key is not null (`per_page: -1`).
  - **Performer discovery:** ids of performers where a key is not null; ids of performers whose
    tags include a list of tag ids (`HierarchicalMultiCriterionInput`, `INCLUDES`, chunked at
    `PERFORMER_TAG_CHUNK`); ids of performers whose `measurements` is not null
    (`StringCriterionInput`).
  - **Media discovery:** ids of scenes and images where a key is not null.
  - **Records by `ids`:** for each kind, selecting only the fields of R1:
    - tags: `id name parents{id} custom_fields updated_at`;
    - performers: `id tags{id} measurements custom_fields updated_at`;
    - scenes and images: `id performers{id} tags{id} custom_fields updated_at`.
  - **Latest `updated_at`:** for each kind (`per_page: 1, sort: "updated_at", direction: DESC`).

  Reuse `fixtures::CriterionModifier` and `CustomFieldCriterion` rather than redefining them.
- [X] T015 [US1] Implement `full_read`, `tag_records`, `performer_records` and `media_records` for
  `Client` in `crates/stash-adapter/src/reader/client.rs`, as research R3 to R5 and R7 describe:
  1. **Start:** take `start`.
  2. **Discovery:** run it, merging ids by numeric order. Run the performer-by-tags query using
     the Trait, Label and Variation tag ids found in tag discovery.
  3. **Records:** fetch them in batches of `RECORD_BATCH` through `ids`.
  4. **Parsing:** parse fields with `field.rs`, and `measurements` with `measurements.rs`. Put
     `archetypeGraphPosition` in `display`, not in fields.
  5. **Missing ids:** count them.
  6. **Stop and progress:** check `should_stop` and call `progress` after every request.

  Continue until T012 and T013 pass.
- [X] T016 [P] [US1] Add the vocabulary 1.3.0 terms from
  `specs/005-stash-adapter/contracts/vocabulary-terms.md`:
  - **`vocabulary/labels.ttl`:** `FieldMappingRule`, `hasFieldMapping`, `mapsToField`,
    `mappingPriority` and `mappingVariation`. Annotations: `sem:status sem:Active` and
    `sem:since "1.3.0"`.
  - **`vocabulary/core.ttl`:**
    - the annotation property `sem:pluginFieldName` (domain `rdf:Property`, range `xsd:string`);
    - `bustSize`, `waist`, `hip` and `waistToHipRatio`, each with domain `sem:Performer`, the
      range in the contract, `sem:scope sem:Global`, `sem:status sem:Derived`,
      `sem:pluginFieldName` (`__derived_bust`, `__derived_waist`, `__derived_hip`,
      `__derived_whr`), and the deliberate-addition `sem:source`;
    - `sem:version "1.3.0"`.
  - **`vocabulary/stash-mapping.ttl`:** `map_fieldMapping` and `map_measurements`, with the
    allowed values exactly as in the contract.

  Each term carries `sem:sourceRevision "2c04141, read 2026-10-24"`.
- [X] T017 [P] [US1] Write failing tests in `crates/semantic-vocabulary/tests/iri_scheme.rs` for
  `ItemIris::field_mapping(tag, slot)`:
  - `{tag}/fieldMapping:base`;
  - `{tag}/fieldMapping:variation:{n}`, where `n` is the slot's index in the sorted variation
    names.

  Then implement it in `crates/semantic-vocabulary/src/iri.rs`.
- [X] T018 [US1] Release vocabulary 1.3.0:
  1. **Changelog:** add `## 1.3.0 (2026-10-24)` to `vocabulary/CHANGELOG.md`, with these
     sections:
     - additions (`FieldMappingRule`);
     - deliberate additions under Principle II: the derived measurements, with the formula and
       constants, citing `matrix.js` at `2c04141`;
     - a new "Display data, passed through" list holding `archetypeGraphPosition`, removed from
       "Excluded on purpose";
     - `archetypeRecipe` stays excluded.
  2. **Regenerate:** run `cargo run -p semantic-vocabulary --bin vocab-check -- --write-terms
     --write-snapshot`, which writes `vocabulary/TERMS.md` and `vocabulary/released/1.3.0.json`.
  3. **Check:** `vocab-check` passes every rule from V001 to V014, and
     `cargo test -p semantic-vocabulary` passes, including `release_compat.rs`.
  4. **Pinned version:** update any test that pins the vocabulary version to accept 1.3.0.

**Checkpoint**: a full read works against the fixtures, and vocabulary 1.3.0 describes every key
read. US1 is usable by the next Phase 2 item.

---

## Phase 4: User Story 2, change reads, record reads and id listings (Priority: P1)

**Goal**: the catch-up and deletion-check reads for all four kinds (FR-009 to FR-012), with
spec 004 rebuilt on the reader (FR-021).

**Independent Test**: `cargo test -p stash-adapter --test reader_changes` passes, and every
spec 004 test passes unchanged: `cargo test -p stash-adapter --test relations` and
`cargo test -p plugin`.

### Tests for User Story 2 ⚠️

- [X] T019 [P] [US2] Write failing tests in `crates/stash-adapter/tests/reader_changes.rs`:
  - **`changed_since`:** for each of the four kinds, the request carries `updated_at`
    `GREATER_THAN` with the threshold text exactly as given, and never `ids`. The answer is
    sorted and de-duplicated (FR-009).
  - **Records:** `media_records` with a deleted id lists it in `missing`, and returns the rest in
    the full-read shape (FR-010).
  - **`all_ids(Tag)`:** a query with no filter, so it returns all tags, not only semantic ones.
    `all_ids` for the other kinds selects `id` only (FR-011).
  - **`latest_updated_at`:** works for `Performer` and `Tag` as well as `Scene` and `Image`
    (FR-012).

### Implementation for User Story 2

- [X] T020 [US2] Add the typed operations to `crates/stash-adapter/src/reader/queries.rs`: ids
  updated after a threshold, for each kind, and every id for each kind (`per_page: -1`, `id`
  only).
- [X] T021 [US2] Implement `changed_since`, `all_ids` and `latest_updated_at` for `Client` in
  `crates/stash-adapter/src/reader/client.rs`, until T019 passes.
- [X] T022 [US2] Rebuild spec 004's reads on the reader in `crates/stash-adapter/src/relations.rs`,
  following the table in contracts/reader-interface.md, "Spec 004 adapters":
  - `relation_definitions`: semantic-tag discovery and `tag_records`, filtered to
    `DEFINITION_FLAGS`, giving the same `TagRecord` fields in the same order;
  - `items_by_ids`;
  - `performer_tags`: a by-id read with no semantic membership rule, returning all tag ids;
  - `changed_since`;
  - `latest_updated_at`;
  - `ids_with_relation_data`.

  Signatures stay the same, and the writes are untouched. Remove the queries in `relations.rs`
  that are now unused. `cargo test -p stash-adapter --test relations`, `cargo test -p engine`
  and `cargo test -p plugin` pass without editing any 004 test (SC-008). Re-record a 004
  fixture only if a request body changed, and say so in its README.

**Checkpoint**: every read the engine needs goes through `reader`, and 004 behaves as before.

---

## Phase 5: User Story 3, distinct, credential-free failures (Priority: P2)

**Goal**: six distinct failure kinds, a version check, and retries only for transient
failures (FR-016 to FR-019, SC-006, SC-007).

**Independent Test**: `cargo test -p stash-adapter --test reader_failures` passes.

### Tests for User Story 3 ⚠️

- [X] T023 [P] [US3] Write failing tests in `crates/stash-adapter/tests/reader_failures.rs`.
  Use `Client::with_api_key` with the secret `SECRET-KEY-005`, and a short request timeout
  (T024's `with_timeouts`). Assert:
  - **Unreachable:** a closed port gives `Unreachable`.
  - **Timed out:** a server that never answers gives `TimedOut`.
  - **Not authorized:** HTTP 401 and 403 give `NotAuthorized`.
  - **Retried failures:** HTTP 503 three times gives `Unreachable` after exactly 3 requests.
    503 then 200 succeeds after 2 requests.
  - **Unsupported version:** `version` `v0.30.0` gives `UnsupportedVersion { found: "v0.30.0",
    minimum: "v0.31.1" }`. `nonsense` gives `UnsupportedVersion` too.
  - **Version checked once:** the version request is sent once per `Client`, however many reader
    calls follow.
  - **Errors from Stash:** a GraphQL `errors` array gives `Graphql`.
  - **Unexpected response:** a body missing `data`, or one that does not match the operation,
    gives `UnexpectedResponse`.
  - **No retries elsewhere:** `NotAuthorized`, `Graphql` and `UnexpectedResponse` are sent once.
    `update_item_custom_fields` against a 503 is sent once, because writes are never retried.
  - **No credentials:** `SECRET-KEY-005` appears in no `Display` or `Debug` output of any of
    these errors (SC-007).
  - **`stash_version()`:** it still returns `v0.30.0` without failing, for `status`.

### Implementation for User Story 3

- [X] T024 [US3] Rework the error handling in `crates/stash-adapter/src/lib.rs`:
  - **Variants:** add `TimedOut`, `NotAuthorized`, `UnsupportedVersion { found, minimum }` and
    `UnexpectedResponse`, and `Error::is_transient()`.
  - **Mapping in `run`:**
    - reqwest connect errors give `Unreachable`;
    - timeouts give `TimedOut`;
    - 401 and 403 give `NotAuthorized`;
    - 502, 503 and 504 give `Unreachable`;
    - other non-2xx statuses and decode failures give `UnexpectedResponse`;
    - an `errors` array gives `Graphql`.
  - **Retries:** add `run_read`, which retries transient errors at most twice, after 250 ms and
    then 1 s. Every reader query and 004 read uses it. Writes keep `run`.
  - **Timeouts:** add `Client::with_timeouts(connect, request)` for tests and the bench.
  - **Credentials:** no message is built from headers.
- [X] T025 [US3] Add the version check to `crates/stash-adapter/src/reader/client.rs`: a
  `OnceLock` on `Client`, filled on the first `LibraryReader` call. It uses
  `version::stash_supported`, and gives `UnsupportedVersion` for an old or unparseable version.
  `Client::stash_version()` stays unchecked. T023 then passes.
- [X] T026 [US3] Update the error mapping in `crates/plugin/src/relations.rs`:
  - `stash_error` maps `Unreachable` and `TimedOut` to `STASH_UNREACHABLE`;
  - `UnsupportedVersion` maps to `STASH_TOO_OLD`;
  - everything else maps to `INTERNAL`, with the kind in the message.

  `store_error` in `crates/plugin/src/ops.rs` treats `TimedOut` as unreachable. Add cases for
  each mapping to `crates/plugin/tests/ops_relations.rs`. `api/operations.json` is unchanged,
  and the agreement test passes.

**Checkpoint**: every failure is distinct and safe to log. The contract is still 1.1.0.

---

## Phase 6: User Story 4, offline testing (Priority: P2)

**Goal**: callers test against a reader with no network, and fixtures are captured read-only
(FR-023 to FR-025).

**Independent Test**: `cargo test -p stash-adapter --test reader_memory --test fixture_capture`
passes, and quickstart step 3 leaves the test instance's counts unchanged.

### Tests for User Story 4 ⚠️

- [X] T027 [P] [US4] Write failing tests in `crates/stash-adapter/tests/reader_memory.rs`. A
  `MemoryReader` keeps rules B1 to B9 of contracts/reader-interface.md:
  - a complete read, or `Stopped`;
  - progress and stopping between batches of `RECORD_BATCH`;
  - sorted and de-duplicated lists;
  - `missing` for unknown ids;
  - `changed_since` compares `updated_at` text;
  - `with_ids` extends `all_ids`;
  - `fail_with` makes every call fail with that error;
  - `from_file` loads `fixtures/reader/library-read.json`.

### Implementation for User Story 4

- [X] T028 [US4] Implement `MemoryReader` in `crates/stash-adapter/src/reader/memory.rs`, until
  T027 passes.
- [X] T029 [US4] Rebuild `crates/stash-adapter/src/fixtures.rs` and
  `crates/stash-adapter/src/bin/fixture-capture.rs` on the reader:
  - **Outputs:** capture writes `library-read.json` (a `LibraryRead`), plus the raw request and
    response pairs into `--out`, for the adapter tests.
  - **Read-only:** it calls only `LibraryReader` methods.
  - **Credentials:** it scrubs nothing because nothing secret is recorded. Assert in
    `crates/stash-adapter/tests/fixture_capture.rs` that no output contains the API key it was
    given.

  Keep the existing `tests/fixtures/test-instance/*.json` files readable, or regenerate them in
  T030.
- [X] T030 [US4] Capture fresh fixtures from the test instance, as in quickstart step 3:
  1. Record the library counts.
  2. Run `fixture-capture` into `crates/stash-adapter/tests/fixtures/test-instance/`.
  3. Confirm the counts are unchanged, and that `grep -r` for the key finds nothing.
  4. Update the README there with the date and counts.
  5. Add a test to `reader_memory.rs` that loads the captured `library-read.json` and checks its
     counts.

**Checkpoint**: the graph builder (the next Phase 2 item) can be written and tested against
`MemoryReader`.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T031 Implement `crates/stash-adapter/src/bin/reader-bench.rs` as in quickstart step 4:
  - **Subcommands:** `full`, `ids` and `change`, with `--runs N`.
  - **Environment:** `STASH_URL`, by default `http://localhost:9998`, and `STASH_API_KEY`.
  - **Read-only:** it sends only `LibraryReader` requests.
  - **Output:** it writes JSON to `specs/005-stash-adapter/results/`, recording the median and
    p95 times, ms per item, the request count, the item counts per kind, and the peak memory
    from `/proc/self/status` `VmHWM` on Linux.
  - **`change`:** waits for one save made by hand, then times `changed_since`.
- [X] T032 Run quickstart step 4 on the test instance:
  1. `spike-bench seed`.
  2. `reader-bench full`, `ids` and `change`.
  3. `spike-bench cleanup`.

  Record the figures against SC-002 (projected), SC-003, SC-004 and SC-005 in quickstart.md,
  under a new "Results" section. If the projected SC-002 figure is over 2 minutes, tune
  `RECORD_BATCH` and re-run before going on.
- [X] T033 Run quickstart step 6 (`cargo test -p plugin --test live_relations -- --ignored
  --test-threads=1`). All 7 live checks must pass.
- [X] T034 [P] Update the documentation:
  - **`ROADMAP.md`:** tick the Phase 2 "Stash adapter crate" item, linking
    `specs/005-stash-adapter/`, and add to Phase 5 "Label evaluation": derived measurements are
    available, and the "performers matching a criterion" read is still to come.
  - **`crates/stash-adapter/src/lib.rs` module docs:** list `reader` as the read path.
  - **`docs/api.md`:** add one sentence saying contract 1.1.0 is unchanged by 005.
- [X] T035 [P] Check `specs/005-stash-adapter/plugin-notes.md` against what was built. Make sure
  the constants and the table match `measurements.rs`, and the `fieldMapping` shape matches
  `keys.rs`.
- [X] T036 Run the full gate:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `cargo test --workspace` with networking unavailable (SC-008);
  - the vocabulary and script checks CI runs.

  Then update the graphify graph for the new and changed files (`graphify-out/`).
- [X] T037 Owner, read-only: run quickstart step 5 (`reader-bench full --runs 3` against
  Production) for the real SC-002 figure, and record it in quickstart.md "Results". This task
  stays open until the owner runs it, and it does not block the tasks above.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1):** none. T002 and T003 can run in parallel after T001.
- **Foundational (Phase 2):** needs Setup.
  - T004, T005, T006 and T008 can run in parallel.
  - T007 needs T005 and T006, and T009 needs T008.
  - T010 needs T007 and T009, and T011 needs T010.
- **US1 (Phase 3):** needs Foundational.
- **US2 (Phase 4):** needs T014 and T015 (the shared queries and the client). T022 also needs
  T021.
- **US3 (Phase 5):** needs Foundational. It can run alongside US1 and US2, except that T025
  edits `client.rs` after T015 and T021.
- **US4 (Phase 6):** T027 and T028 need only Foundational. T029 and T030 need US1.
- **Polish (Phase 7):** needs US1 to US4. T037 is the owner's, and blocks nothing.

### User Story Dependencies

- **US1:** the MVP. Independent after Foundational.
- **US2:** uses US1's queries and client. Its tests are independent.
- **US3:** independent in behaviour. It shares `lib.rs` and `client.rs` with US1 and US2.
- **US4:** `MemoryReader` is independent. Capture needs US1.

### Within Each User Story

- Tests are written first and fail.
- Then come the queries, then the client methods, then the callers (004 or the plugin).
- In US1, the vocabulary tasks (T016 to T018) run alongside the code tasks.

### Parallel Opportunities

- T002 and T003 (fixtures).
- T004, T005, T006 and T008 (foundational files and tests).
- T012 and T013, then T016 and T017 alongside T014 and T015.
- T019, T023 and T027, which are test files of different stories.
- T034 and T035 (documentation).

---

## Parallel Example: User Story 1

```text
Task: "T012 [US1] reader_full.rs tests"
Task: "T013 [US1] reader_stop.rs tests"
Task: "T016 [US1] vocabulary 1.3.0 terms"
Task: "T017 [US1] ItemIris::field_mapping"
```

Then T014, then T015, in sequence (both touch the reader), and T018 after T016 and T017.

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup and Foundational: fields, measurements, records and the trait.
2. US1: the full read, and vocabulary 1.3.0.
3. **Stop and validate:** `reader_full` and `reader_stop` pass, and `vocab-check` passes. The
   graph builder can start from here.

### Incremental Delivery

1. US2: change reads, and 004 rebuilt on the reader. 004's tests must stay green.
2. US3: failure kinds, retries and the version check.
3. US4: `MemoryReader` and fixture capture.
4. Polish: the bench, the measured figures, the documentation and the full gate.
5. The owner runs the Production figure (T037).

### Notes

- Commit only when the owner asks. No push or release without approval.
- Scan every file for credentials before a commit. The repository is public.
