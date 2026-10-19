# Tasks: Engine Design Spike

**Input**: Design documents from `/specs/003-engine-design-spike/`

**Prerequisites**:
- [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md) (R1–R10),
  [data-model.md](data-model.md), [quickstart.md](quickstart.md)
- Contracts: [spike-bench](contracts/spike-bench.md), [spike-plugin](contracts/spike-plugin.md),
  [decision-record](contracts/decision-record.md)

**Tests**: The plan requires tests first for the spike's own logic: statistics, snapshot
publication, the host guard, the seed manifest and the synthetic vocabulary. The measurements
themselves run live against the test server (`STASH_TEST_*` environment variables, as in 002) and
are not unit tests.

**Reuse** (found through the graphify graph of the workspace):
- The spike plugin reuses:
  - `plugin::input::parse` / `PluginInput` (`crates/plugin/src/input.rs`)
  - `plugin::envelope::{Envelope, stash_output}` (`crates/plugin/src/envelope.rs`)
  - `plugin::log::Logger` with `add_secret` (`crates/plugin/src/log.rs`)
- All Stash I/O goes through `stash_adapter::Client` (`from_server_connection`, `with_api_key`,
  `run`) in `crates/stash-adapter/src/lib.rs`.
- Flag-filtered fetches reuse `TAG_FLAGS`, `MEDIA_KEYS`, `PERFORMER_FLAGS` and the filter types in
  `crates/stash-adapter/src/fixtures.rs`.

**Organization**: Tasks are grouped by user story. US1 and US2 are P1; US3 and US4 are P2. US4
(the decision record) depends on the results of US1–US3.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US4 from spec.md

---

## Phase 1: Setup

**Purpose**: The spike crate, the separate spike plugin, and where results live. None of this is
built by the release package scripts (R9).

- [X] T001 Create `crates/spike/Cargo.toml`:
  - `publish = false`, `[lib]`, and binaries `spike-plugin` and `spike-bench`.
  - Dependencies: `plugin`, `engine`, `semantic-vocabulary`, `stash-adapter` with
    `features = ["spike"]`, `oxigraph` 0.4.11 (default features, bundled RocksDB),
    `serde`, `serde_json`, `flate2`, `base64`, `uuid`.
  - Create an empty `crates/spike/src/lib.rs` and stub `main` functions in
    `crates/spike/src/bin/spike-plugin.rs` and `crates/spike/src/bin/spike-bench.rs`.
- [X] T002 Add a `spike` cargo feature (default off) to `crates/stash-adapter/Cargo.toml`, and a
  `#[cfg(feature = "spike")] pub mod spike;` in `crates/stash-adapter/src/lib.rs` with an empty
  `crates/stash-adapter/src/spike.rs`. The shipped `plugin` crate must not enable it.
- [X] T003 [P] Write the spike plugin manifest `spike/semantic-engine-spike.yml` exactly as in
  `contracts/spike-plugin.md`:
  - `interface: raw` and `exec: [spike-plugin]`
  - `ui.assets` mapping `/` to `store/assets`
  - one hook "Record semantic saves" on the 13 Tag/Scene/Image/Performer Create/Update/Merge/
    Destroy Post triggers, with `defaultArgs: { op: recordHook }`
- [X] T004 [P] Write `scripts/spike-install.sh` (POSIX `sh`, same style as `scripts/install.sh`):
  - Run `cargo build --release -p spike --bin spike-plugin`.
  - Copy the binary and `spike/semantic-engine-spike.yml` into
    `<plugins dir>/semantic-engine-spike/`.
  - Refuse any plugins dir that is not under
    `/home/bbulder/Projects/Stash-Plugins/Test-Instance/` unless `--force` is given.
- [X] T005 [P] Create `specs/003-engine-design-spike/results/README.md`:
  - Say that `spike-bench` writes these files and that they contain no credentials.
  - Explain the file naming `<id>[-<timestamp>].json` from `contracts/spike-bench.md`.

---

## Phase 2: Foundational (blocking)

**Purpose**: Shared parts every story needs: measurement records, store layout, the two binaries'
skeletons, the host guard, the Stash write operations, and seeding. No story starts before this
phase is done.

- [X] T006 [P] Write tests in `crates/spike/tests/measure.rs` for the Measurement record:
  - `median`, `p95`, `min` and `max` are recomputed from `samples` on load, so they can never
    disagree with the raw data.
  - A latency figure with `runs < 100` is rejected.
  - The first call after a plugin reload is never in `samples`; it gets its own measurement with
    `variant = "first-call"`.
  - Writing never overwrites an existing `results/<id>.json`; a rerun gets a timestamp suffix.
  - `command` and `failures[].message` never contain the API key or cookie value.
- [X] T007 Implement `crates/spike/src/measure.rs` to make T006 pass:
  - The `Measurement` struct with the data-model fields, in camelCase JSON: `id`, `question`
    (enum `process|persistence|freshness|limits|snapshot|build|decision`), `command`,
    `server{stashVersion,host}`, `library{kind,scenes,images,performers,tags,triples}`, `runs`,
    `samples`, `median`, `p95`, `min`, `max`, `parts`, `failures[{run,kind,message}]`, `takenAt`
    (RFC 3339) and `engineCommit`.
  - The `Option` and `HookObservation` structs.
  - `write_result(dir, &Measurement)`, which redacts secrets.
- [X] T008 [P] Write tests in `crates/spike/tests/store.rs` for the snapshot generation layout
  (data-model "Snapshot generation"), using a temporary directory:
  - **Publish:** in this order: take `writer.lock`, write `work/`, `flush`, `backup` into
    `snapshots/<n+1>/`, atomically rename a temp file over `current`, release the lock.
  - **Read during write:** a reader opening `current` while a write runs sees the previous
    generation, complete.
  - **Lock modes:** a second writer with `lockMode=fail` gets `Busy` at once; with `wait` it
    succeeds after the first writer releases, or times out after `waitMs`.
  - **GC:** it keeps at least the newest two generations and removes older ones past the grace
    period.
  - **Damage:** a missing or damaged `current` is reported as a distinct error, not a panic.
- [X] T009 Implement `crates/spike/src/store.rs` to make T008 pass. Use `oxigraph::store::Store`
  `open`, `open_read_only`, `flush`, `backup` and `bulk_loader`, and the standard library's advisory file locks (`File::try_lock`, stable since Rust 1.89) for `writer.lock`; no locking crate is needed.
  Expose:
  - `open_writer(dir, LockMode)`
  - `publish(&writer)`
  - `open_current_read_only(dir)`
  - `open_live_read_only(dir)`, for option A (documented as undefined behaviour, R2)
  - `gc(dir, grace)`
- [X] T010 [P] Write tests in `crates/spike/tests/synth.rs`:
  - Every predicate and class the generator emits is a term of vocabulary 1.1.0. Check with
    `semantic_vocabulary::embedded()` and `Vocabulary::terms()`.
  - Item IRIs come from `semantic_vocabulary::iri::ItemIris`.
  - The same scale and seed give identical quads.
  - `--scale test` gives 1,021 scenes.
- [X] T011 Implement `crates/spike/src/synth.rs` to make T010 pass:
  - Generate vocabulary-shaped quads for a scene count: scenes, images, performers, the six
    flagged tag types plus Variations, and reified role, action and pluggable instances per
    scene.
  - Take per-scene densities from the seeded test library, recorded in the code comment.
  - Support the scales `test` (1,021), `50k`, `100k` and `250k`, or a number.
  - Stream into `bulk_loader` so 250k does not need the whole graph in memory.
- [X] T012 Add typed Cynic operations to `crates/stash-adapter/src/spike.rs`:
  - **Mutations:** `tagCreate`, `tagUpdate`, `tagDestroy`, `tagsDestroy`, `tagsMerge`,
    `bulkTagUpdate`, `sceneUpdate`, `bulkSceneUpdate`, `imageUpdate`, `bulkImageUpdate`,
    `performerUpdate`, `bulkPerformerUpdate`, `runPluginOperation`, `setPluginsEnabled` and
    `reloadPlugins`.
  - **Queries:** `stats`, and `findScenes`/`findImages`/`findPerformers`/`findTags` with an
    `updated_at` `GREATER_THAN` filter plus an ids-only page.
  - Use only the pinned `schemas/stash-v0.31.1.graphql`.
  - Add a fake-Stash unit test in `crates/stash-adapter/tests/spike.rs`, following
    `tests/common` (`fake_stash`), that sends one mutation and checks the request body.
- [X] T013 Implement the spike plugin skeleton in `crates/spike/src/bin/spike-plugin.rs` and
  `crates/spike/src/plugin_ops.rs`:
  - Read stdin with `plugin::read_stdin` and parse with `plugin::input::parse`.
  - Add the session cookie value as a `Logger` secret.
  - Dispatch on `args.op`, wrapping results with `plugin::envelope::Envelope` and `stash_output`.
  - The store dir is `<PluginDir>/store`.
  - Unknown op returns `UNKNOWN_OPERATION` with the list of known ops.
- [X] T014 [P] Write tests in `crates/spike/tests/bench_guard.rs` for the `spike-bench`
  connection rules in `contracts/spike-bench.md`:
  - It refuses `STASH_TEST_URL` other than `http://localhost:9998` unless `--allow-host`, with
    exit code 3.
  - Write subcommands (`seed`, `cleanup`, `hooks`) are refused on any other host even with
    `--allow-host`, also exit code 3.
  - A missing env var is exit code 2.
  - The API key never appears in stdout, stderr or result files.
- [X] T015 Implement the `spike-bench` skeleton in `crates/spike/src/bin/spike-bench.rs` and
  `crates/spike/src/bench/mod.rs` to make T014 pass:
  - Subcommand parsing (std only, no CLI crate).
  - Global options `--runs`, `--scale` and `--out`, defaulting to
    `specs/003-engine-design-spike/results`.
  - The env and host guard, and exit codes 0–4.
  - A `call_op(op, args)` helper that runs `runPluginOperation(plugin_id: "semantic-engine-spike")`
    through the adapter, measures client-side wall time and parses the envelope.
- [X] T016 [P] Write tests in `crates/spike/tests/seed.rs` for the seed manifest (data-model
  "Seed manifest"):
  - The manifest is written with `state: seeding` before the first write, then `seeded`.
  - Cleanup restores every `changed[].customFieldsBefore` and deletes every `createdTags` id,
    then sets `state: cleaned`.
  - Cleanup still reverses whatever is recorded when the state is `seeding` (a crash mid-seed).
  - Cleanup refuses when `server` differs from the current URL or no manifest exists (exit 3).
  - Use a fake Stash from `crates/stash-adapter/tests/common`.
- [X] T017 Implement the `seed` and `cleanup` subcommands in `crates/spike/src/bench/seed.rs` to
  make T016 pass (R8):
  - Create 6 flagged tags per type: `plugtag`, `isAction`, `isRole`, `isTrait`, `isLabel`,
    `isMiseEnScene` and `isVariation`, set as tag custom fields.
  - Write `roleTagAssignments`, `actionAssignments` and `pluggableAssignments` custom fields on
    `--scenes N` scenes and `--images N` images, in the shapes documented in
    `vocabulary/stash-mapping.ttl` and the 002 fixtures under
    `crates/stash-adapter/tests/fixtures/test-instance/`.
  - Record `statsBefore` from `stats`.
- [X] T018 Live: install the spike plugin with `scripts/spike-install.sh`, reload plugins, and
  run `spike-bench seed --scenes 500 --images 500` against the test server.
  - Check `specs/003-engine-design-spike/results/seed-manifest.json` has `state: seeded`, and that
    `stats` changed only by the created tags.

**Checkpoint**: the spike plugin answers through Stash, results are written, and the test
library holds semantic data.

---

## Phase 3: User Story 1 - Process and persistence model (Priority: P1) 🎯 MVP

**Goal**:
- Per-call cost through Stash with the stored graph reopened every call, split into its parts.
- Behaviour of sharing options A/B/C under a concurrent write.
- The upper bound on what a long-lived process could save.

**Independent Test**: Run `spike-bench call-cost` and `spike-bench concurrency` (quickstart
steps 2–3) and read `results/process-reopen-*.json` and `results/persist-*.json`.

- [X] T019 [US1] Implement the `query` op in `crates/spike/src/plugin_ops/query.rs`:
  - Open the published generation with `store::open_current_read_only` and run one of five fixed
    SPARQL `SELECT`s of the documents' Phase 1 kind (1 to 5 triple patterns), chosen by
    `queryId`.
  - Return `{ rows, timings: { storeOpen, query, serialise } }`.
  - Report a missing store as an error code, never as empty results (Principle IV).
- [X] T020 [US1] Implement the `write` op in `crates/spike/src/plugin_ops/write.rs`, with
  arguments `option` (A/B/C), `scale`, `lockMode` (wait/fail) and `waitMs`:
  - **Option A:** writes the live store with no publish.
  - **Option B:** writes `work/` then publishes a generation.
  - **Option C:** holds the writer lock for every call, reads included.
  - Return `{ triples, timings: { lock, write, flush, publish }, generation }`.
- [X] T021 [P] [US1] Implement the `buildFromStash` op in
  `crates/spike/src/plugin_ops/build.rs`:
  - Fetch with the bounded, flag-filtered queries from `stash_adapter::fixtures` ("flag is not
    null" filters only; no per-item requests).
  - Map items to vocabulary quads with `synth.rs` helpers.
  - Load and publish.
  - Return `{ items, triples, timings: { fetch, build } }`.
- [X] T022 [US1] Implement the `synth` and `build` subcommands in
  `crates/spike/src/bench/build.rs`:
  - **`synth --scale S`:** writes `build-synth-<S>`, with build time, store size on disk and
    triple count over at least 3 runs.
  - **`build --from-stash`:** writes `build-stash-test`, with fetch and build time per item.
- [X] T023 [US1] Implement the `call-cost --scale S` subcommand in
  `crates/spike/src/bench/call_cost.rs`:
  - Call `reloadPlugins` once and time the first call into `process-first-call`, which is never
    mixed into samples.
  - Then run at least 100 `query` calls cycling `queryId` 1–5.
  - Write `process-reopen-<S>` with `parts` set to `processStart` (the 002 baseline method),
    `storeOpen`, `query`, `serialise`, and client wall time.
- [X] T024 [US1] Implement the `concurrency --option A|B|C --scale S` subcommand in
  `crates/spike/src/bench/concurrency.rs`:
  - Start a long `write` (the 250k bulk load) through `runPluginOperation` on one thread, while
    4 reader threads loop `query` calls until it ends.
  - Record per read: ok, waited or failed, latency, and whether the rows came from a mixed
    generation (detected by a generation-tagged quad written last).
  - Also run two simultaneous `write` calls with `lockMode=fail` and `wait`.
  - Write `persist-<option>-<S>`.
- [X] T025 [US1] Live: run quickstart step 2 (`synth` and `call-cost` for `test`, `50k`, `100k`
  and `250k`, plus `build --from-stash`) and step 3 (`concurrency` A, B and C at `250k`) on the
  test server.
  - Commit the result JSON under `specs/003-engine-design-spike/results/`.
- [X] T026 [US1] Write the long-lived-process evaluation in
  `specs/003-engine-design-spike/results/process-long-lived.md`, following R3:
  - Why sockets and named pipes are excluded by Principle IV.
  - The file-mailbox alternative.
  - The measured `storeOpen` share at 250k as the upper bound on its saving.
  - A verdict: dropped, or proposed with the amendment (FR-005, FR-015).

**Checkpoint**: per-call cost and sharing-option behaviour are measured at every scale.

---

## Phase 4: User Story 2 - Freshness (Priority: P1)

**Goal**:
- A hook table for 4 entity types × 3 clients, with delay and added save latency.
- Bulk and merge behaviour.
- Whether `updated_at` and id comparison can catch up what hooks miss.

**Independent Test**: Run quickstart step 4 and check `results/hooks.json` against the coverage
rule.

- [X] T027 [US2] Implement the `recordHook` op in `crates/spike/src/plugin_ops/hooks.rs`:
  - Take `args.hookContext` (`id`, `type`, `input`, `inputFields`) and append one JSON line
    `{ receivedAt, type, id, inputFields, inputBytes }` to `<PluginDir>/hooks.jsonl`.
  - Do nothing else (R4: this is the floor for a hook handler).
  - Time its own run and include it in the line.
- [X] T028 [P] [US2] Write `crates/spike/src/bench/clients.rs`, the scripted saves per client:
  - **web-ui:** the web interface's single and bulk mutations.
  - **semantic-tagging:** `tagCreate`, `tagUpdate`, `tagDestroy`, `sceneUpdate`, `imageUpdate`
    and `performerUpdate`, with that plugin's custom-field payload shapes taken from
    `/home/bbulder/Projects/Stash-Plugins/semantic-tagging/`.
  - **viewer:** the same plus `bulkSceneUpdate`, `bulkImageUpdate`, `bulkPerformerUpdate`,
    `bulkTagUpdate`, `tagsMerge` and `tagsDestroy`, with payload shapes from
    `/home/bbulder/Projects/Semantic-Stash-Viewer/`.
  - Each save touches only items listed in the seed manifest, or tags it creates and records
    there.
- [X] T029 [US2] Implement the `hooks` subcommand in `crates/spike/src/bench/hooks.rs`:
  - For each `entity` × `client`, time the save with the spike plugin enabled, then disabled via
    `setPluginsEnabled`, re-enabling it after.
  - Read new `hooks.jsonl` lines from the plugin dir (`STASH_TEST_PLUGINS_DIR`) and match them to
    the save.
  - Write `HookObservation` rows to `results/hooks.json` with these fields: `entity`
    (tag|scene|image|performer), `client` (web-ui|semantic-tagging|viewer), `mode`
    (scripted|manual), `mutation`, `items`, `hooksReceived`, `delay`, `saveLatency`,
    `saveLatencyNoHook`, `updatedAtBumped`, `missed` and `outOfOrder`.
  - Enforce the coverage rule: at least one scripted row for each of the 12 `entity` × `client`
    pairs and at least one manual row per client. Exit 1 with the missing pairs listed otherwise.
- [X] T030 [US2] Extend `hooks` in `crates/spike/src/bench/hooks.rs` with `--bulk N` for 10, 100 and 1000 scenes via
  `bulkSceneUpdate`, recording the number of hook calls, total added save latency and order.
  Also cover `tagsMerge` and `tagsDestroy` rows (FR-007).
- [X] T031 [US2] Add `hooks --manual` in `crates/spike/src/bench/hooks.rs`. It waits while the owner saves one item from each real
  client, then logs the received hooks as `mode: manual` rows, with the client given by
  `--client` for each listening window (`--seconds`, default 120).
- [X] T032 [US2] Implement the `catch-up` subcommand in `crates/spike/src/bench/catch_up.rs`:
  - For each scripted save in `hooks.json`, check whether `updated_at > t` finds the item, and
    whether a custom-field-only change bumps `updated_at`. Write `freshness-updated-at`.
  - Time listing all ids per type at test-library size. Write `freshness-id-diff`, with the
    per-item cost for extrapolating deletion detection to the Production estimate.
- [X] T033 [US2] Live: run quickstart step 4's scripted part (`hooks`, `hooks --bulk 10/100/1000`,
  `catch-up`) on the test server.
- [X] T034 [US2] Live, owner: the owner saves one item from Stash's web UI, the semantic-tagging
  plugin's UI and the viewer while `hooks --manual` runs. Commit `results/hooks.json`.
  - Done 2026-10-18: 4 web-UI rows (performer created from the scene editor, scene save, tag
    added to a scene, tag edit) and 1 semantic-tagging row (tag custom-field update). The viewer
    sends no saves yet, so it has no manual row; rerun its round once it writes to Stash.

**Checkpoint**: the hook table is complete (SC-003), and the hook cost added to saves is known.

---

## Phase 5: User Story 3 - Limits clients will hit (Priority: P2)

**Goal**: The largest answer one call can return, a proposed page size, and the two snapshot
delivery paths compared.

**Independent Test**: Run quickstart step 5 and read `results/limits-payload.json` and
`results/snapshot-*.json`.

- [X] T035 [P] [US3] Implement the `payload` op in `crates/spike/src/plugin_ops/payload.rs`. It
  returns `{ bindings: [...] }` of about `bytes` size, using realistic binding objects (IRI plus
  label pairs).
- [X] T036 [US3] Implement the `payload` subcommand in `crates/spike/src/bench/payload.rs`:
  - Double from 1 KiB until a call fails or its median exceeds 1 s, with at least 100 runs per
    size up to the point where latency passes 200 ms.
  - Record the curve, the failure point and its message, and the proposed page size: the largest
    size whose p95 is under 200 ms (R7).
  - Write `limits-payload`.
- [X] T037 [P] [US3] Implement the `exportSnapshot` and `snapshotInfo` ops in
  `crates/spike/src/plugin_ops/snapshot.rs`:
  - Export the current generation as gzip-compressed N-Quads.
  - With `to: output`, return `{ manifest, data }` with `data` base64-encoded.
  - With `to: asset`, write `store/assets/snapshot.nq.gz` and `store/assets/manifest.json`
    (generation, built at, vocabulary and engine versions), then return `{ manifest, path }`.
  - `snapshotInfo` returns the manifest.
- [X] T038 [US3] Implement the `snapshot --path op|asset --scale S` subcommand in
  `crates/spike/src/bench/snapshot.rs`:
  - Time the export and the fetch, either via `runPluginOperation` or by HTTP GET
    `/plugin/semantic-engine-spike/assets/snapshot.nq.gz` with the API key.
  - Record sizes.
  - Check that the asset GET without credentials is refused (R6).
  - Check that a client can tell whether its snapshot is current by comparing generations.
  - Write `snapshot-<path>-<S>`.
- [X] T039 [US3] Live: run quickstart step 5 (`payload`, and `snapshot` for both paths at `test`
  and `250k`) on the test server.
  - Done 2026-10-19. `payload` scales at about 75 ms per MiB. The proposed page size is 2 MiB
    (17,231 bindings, p95 192 ms). 16 MiB passed the 1 s stop with no failed call.
  - `snapshot` at 250k (9.79M quads, 74 MB gzip) works on both paths with no failures. The
    plugin's dump and gzip take 186–208 s. Delivery is small: 105 ms to GET the asset, 90 ms to
    decode the op path's 98.5 MB base64. At `test` both paths take about 0.52 s.
  - The asset GET without a login gets 401. `snapshotInfo` answers in about 5 ms (median), and
    clients could tell their generation was current.

**Checkpoint**: the paging and snapshot rules for Phase 3 can be stated from measurements.

---

## Phase 6: User Story 4 - Go/no-go decision (Priority: P2)

**Goal**: One decision record that answers all seven Phase 1 roadmap items, compares them with the
viewer's 009 budgets, and ends with an outcome (FR-014).

**Independent Test**: Read `decision-record.md` alone and check it against
`contracts/decision-record.md`.

- [ ] T040 [P] [US4] Write `scripts/spike-build-cost.sh` (POSIX `sh`):
  - Read the v0.1.0 release asset sizes with `gh release view v0.1.0 --json assets`, and the
    latest green CI run's per-job durations with `gh run view --json jobs`.
  - Time one local `cargo clean -p plugin && cargo build --release -p plugin`, 3 runs.
  - Write `specs/003-engine-design-spike/results/build-release.json` in the Measurement shape,
    with `question: build` (FR-010, R10).
- [ ] T041 [US4] Implement the `report` subcommand in `crates/spike/src/bench/report.rs`:
  - Load the newest result per id (recomputing `median` and `p95` from `samples`) and print a
    Markdown summary grouped by the seven questions.
  - Show the difference from the previous run of the same id, and flag any median more than 25%
    away (SC-002).
- [ ] T042 [US4] Live: re-run `spike-bench call-cost --scale test` and check with `report` that the
  median is within 25% of the recorded one (SC-002). Then run `spike-bench cleanup` and check
  `seed-manifest.json` reaches `state: cleaned` and `stats` equals `statsBefore` (SC-005).
- [ ] T043 [US4] Write `specs/003-engine-design-spike/decision-record.md` in the exact structure
  of `contracts/decision-record.md`:
  - **The seven decisions**, each with:
    - an options table (columns name, key figures, constitution, verdict);
    - evidence links to `results/<id>.json`;
    - the `spike-bench` commands to reproduce it.
  - **Section 3** includes the 12-row hook table plus the bulk rows.
  - **The "For client maintainers" section** gives the page size, freshness delay and cost added
    per save.
  - **"Rules for Phase 2 and Phase 3"** covers persistence, concurrency, freshness, paging and
    the hook budget.
  - **"Estimates"** scales per-item figures beyond 250k, each marked as an estimate.
  - **"Constitution amendments proposed"** is filled per FR-015.
  - **The outcome line** is `proceed`, `proceed-with-changes` or `fall-back`, and matches
    section 7.
- [ ] T044 [US4] Owner review of `decision-record.md` for SC-006: the owner can say, for each
  client, whether to depend on the engine, the page size and the freshness delay. Record the review
  date in the record's header.

**Checkpoint**: the spike's deliverable is complete. Phase 2 can cite it.

---

## Phase 7: Polish & cross-cutting

- [ ] T045 [P] Add a CI step to `.github/workflows/ci.yml` that runs `scripts/package.sh` for the
  host target and fails if the archive contains `spike-plugin` or `semantic-engine-spike.yml`
  (FR-013).
- [ ] T046 [P] Run `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  and `cargo test --workspace` so the spike crate builds and its tests pass in CI.
- [ ] T047 Tick the seven Phase 1 items in `ROADMAP.md` with a one-line answer each, linking
  `specs/003-engine-design-spike/decision-record.md`.
  - If the outcome is `proceed-with-changes` or `fall-back`, also edit the Phase 2–4 items the
    record changes.
- [ ] T048 Remove the spike plugin from the test server
  (`<STASH_TEST_PLUGINS_DIR>/semantic-engine-spike/`) and reload plugins.
  - Check `api/operations.json` and `docs/api.md` are unchanged from contract 1.0.0
    (`git diff --exit-code api docs`).
- [ ] T049 Fill in the results of each step in `specs/003-engine-design-spike/quickstart.md`, as
  002's quickstart did.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup** (T001–T005): no dependencies. T003–T005 can run in parallel after T001 and T002.
- **Foundational** (T006–T018): needs Setup. Tests come before their implementation: T006→T007,
  T008→T009, T010→T011, T014→T015, T016→T017. T012 is needed by T015 and T017. T018 (live seed)
  needs T013, T015 and T017.
- **US1** (T019–T026): needs Foundational. T019 and T020 come before T023 and T024; T021 before
  the `build` part of T022; T025 needs T022–T024; T026 needs T025.
- **US2** (T027–T034): needs Foundational, and is independent of US1. T027 and T028 come before
  T029, then T030 and T031 follow; T032 needs T029; T033 needs T029, T030 and T032; T034 needs
  T031.
- **US3** (T035–T039): needs Foundational. Its `snapshot` path at `250k` reuses US1's synthetic
  store (T022). T035→T036; T037→T038; T039 needs T036 and T038.
- **US4** (T040–T044): needs the live results of US1, US2 and US3 (T025, T026, T033, T034,
  T039). T040 can start any time.
- **Polish** (T045–T049): T045 and T046 any time after Setup; T047–T049 after T043.

### User story dependencies

```text
Setup → Foundational ─┬─ US1 (P1) ──────┐
                      ├─ US2 (P1) ──────┼─→ US4 (P2) → Polish
                      └─ US3 (P2, uses US1's 250k store for one row) ─┘
```

### Parallel opportunities

- **Setup:** T003, T004 and T005.
- **Foundational tests:** T006, T008, T010, T014 and T016, all in different files.
- **Across stories:** US1 and US2 can proceed in parallel after Foundational, because their files
  are disjoint (`plugin_ops/query.rs`, `write.rs`, `build.rs` and `bench/build.rs`,
  `call_cost.rs`, `concurrency.rs` vs `plugin_ops/hooks.rs` and `bench/clients.rs`, `hooks.rs`,
  `catch_up.rs`).
- **Within US1:** T021 is parallel with T019 and T020.
- **Within US3:** T035 and T037 can run in parallel.
- **Other:** T040, T045 and T046 can run any time.
- **Not parallel:** live runs (T018, T025, T033, T034, T039 and T042) share one test server, so
  they run one at a time to keep the timings clean.

### Parallel example: Foundational tests

```text
Task: "T006 measure tests in crates/spike/tests/measure.rs"
Task: "T008 store tests in crates/spike/tests/store.rs"
Task: "T010 synth tests in crates/spike/tests/synth.rs"
Task: "T014 host-guard tests in crates/spike/tests/bench_guard.rs"
Task: "T016 seed manifest tests in crates/spike/tests/seed.rs"
```

### Parallel example: US1 and US2 after Foundational

```text
Task: "T019 query op in crates/spike/src/plugin_ops/query.rs"
Task: "T021 buildFromStash op in crates/spike/src/plugin_ops/build.rs"
Task: "T027 recordHook op in crates/spike/src/plugin_ops/hooks.rs"
Task: "T028 scripted client saves in crates/spike/src/bench/clients.rs"
```

---

## Implementation Strategy

### MVP first (User Story 1)

1. Setup and Foundational: the spike plugin answers through Stash and the test library is seeded.
2. US1: per-call cost at four scales and the sharing options.
3. **Stop and check**: if reopening a 250k snapshot per call already misses the 200 ms budget by a
   wide margin, write a short interim note before going further. The go/no-go may already be
   clear, and US2 and US3 can then be cut down to what the fallback needs.

### Incremental delivery

1. US1 answers persistence, concurrency and the process model.
2. US2 answers freshness and the hook cost per save, in parallel with US1 if wanted.
3. US3 answers paging and snapshot delivery.
4. US4 writes the record, cleans up, and gets the owner's review.
5. Polish: CI packaging guard, roadmap ticks, and removal from the test server.

### Notes

- Writes go only to the test server and only through `spike-bench`. Every write is first recorded
  in `seed-manifest.json` (FR-012, SC-005).
- Credentials come from `Test-Instance/creds.txt` into environment variables only. Never echo,
  log, or commit them.
- Commit result JSON with the code that produced it, so `engineCommit` points at a real revision.
