---

description: "Task list for the Engine Plugin Foundation feature"
---

# Tasks: Engine Plugin Foundation

**Input**: Design documents from `/specs/002-engine-plugin-foundation/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. Constitution Principle VII requires tests before behaviour; every test task
precedes the code it covers and MUST fail first. Tests use recorded fixtures or a fake HTTP
endpoint, never a live Stash, except the opt-in integration tests gated on `STASH_TEST_URL`.

**Organization**: Grouped by user story (US1 to US5 from spec.md). Paths follow plan.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1 to US5

Key references: Stash raw interface and envelope (research R1, contracts/operations.md);
callback connection (R2); Cynic and the pinned schema (R3); server identifier storage (R4); log
format (R5); bundled RocksDB version (R6); packages (R7); install scripts (R8,
contracts/install-scripts.md); manifest (contracts/manifest.md).

---

## Phase 1: Setup

**Purpose**: New crates, pinned schema, and shared files

- [X] T001 Add workspace members `crates/stash-adapter`, `crates/engine`, `crates/plugin` with `Cargo.toml` stubs and empty `src/lib.rs` / `src/main.rs` (binary name `semantic-engine` for `crates/plugin`); keep `crates/semantic-vocabulary` unchanged
- [X] T002 Create `schemas/stash-v0.31.1.graphql` by concatenating `graphql/schema/schema.graphql` and every `graphql/schema/types/*.graphql` from `~/Projects/Stash-Plugins/Stash-Source/stash-0.31.1` in sorted file order, with a header comment naming the source version and date; add `scripts/update-schema.sh` that regenerates it from a given source tree
- [X] T003 [P] Add dependencies per plan Technical Context: `crates/stash-adapter/Cargo.toml` (`cynic` with `http-reqwest-blocking`, `reqwest` with `blocking`, `json`, `rustls-tls`, default features off; build-dependency `cynic-codegen`; `serde`, `serde_json`, `thiserror`), `crates/engine/Cargo.toml` (`oxigraph = "0.4"` default features, `uuid` with `v4`, `semantic-vocabulary`, `stash-adapter`, `serde_json`, `thiserror`), `crates/plugin/Cargo.toml` (`engine`, `stash-adapter`, `serde`, `serde_json`)
- [X] T004 [P] Create `plugin/semantic-engine.yml` exactly as in `contracts/manifest.md` (version filled from the engine version by `scripts/package.sh` later)
- [X] T005 [P] Create `api/operations.json` listing contract version `1.0.0`, operations `ping`, `status`, `getServerId`, task `generateServerId` ("Generate server identifier"), and error codes `INVALID_INPUT`, `UNKNOWN_OPERATION`, `STASH_UNREACHABLE`, `STASH_TOO_OLD`, `SERVER_ID_NOT_GENERATED`, `SERVER_ID_INVALID`, `INTERNAL`, each with "since" `1.0.0`

---

## Phase 2: Foundational (blocking)

**Purpose**: Plugin I/O envelope, logging, adapter client, versions

**CRITICAL**: No user story work begins until this phase completes.

- [X] T006 [P] Write failing test `crates/plugin/tests/envelope.rs`: parsing the stdin shape in data-model "Plugin input" (Go-cased `server_connection` fields); a missing or non-JSON stdin produces an envelope with `ok: false`, `error.code` `INVALID_INPUT`, and top-level `error: null`; any envelope has `ok`, `op`, `contractVersion`
- [X] T007 [P] Write failing test `crates/plugin/tests/log.rs`: each level writes `0x01`, its character (`t`, `d`, `i`, `w`, `e`, `p`), `0x02`, then the message; a message containing the session cookie value is rejected or redacted
- [X] T008 [P] Write failing test `crates/stash-adapter/tests/connection.rs`: the endpoint is `{Scheme}://{Host}:{Port}/graphql`, `Host` `0.0.0.0` or `::` becomes `localhost`, the session cookie is sent as a `Cookie` header, and no other host is contacted (use a local fake HTTP server bound to 127.0.0.1)
- [X] T009 Implement `crates/plugin/src/main.rs` and `crates/plugin/src/envelope.rs`: read all stdin, parse input, dispatch on `args.op`, always print `{"error": null, "output": <envelope>}` to stdout and exit 0 when an envelope exists; make T006 pass
- [X] T010 [P] Implement `crates/plugin/src/log.rs` with the Stash level prefixes and cookie redaction; make T007 pass
- [X] T011 Implement `crates/stash-adapter/build.rs` registering `schemas/stash-v0.31.1.graphql` with `cynic-codegen` as schema `stash`, and `crates/stash-adapter/src/lib.rs` with `Client::from_server_connection` (R2) and scalar declarations for `Map`, `Any`, `PluginConfigMap` as JSON values; make T008 pass
- [X] T012 [P] Write failing test `crates/engine/tests/versions.rs`: `engine::versions()` returns non-empty `engineVersion`, `contractVersion` `1.0.0`, `vocabularyVersion` equal to the embedded vocabulary's `sem:version`, `oxigraphVersion`, `rocksdbVersion` matching `^\d+\.\d+\.\d+$`, `stashMinimumVersion` `v0.31.1`
- [X] T013 Implement `crates/engine/build.rs` (read `ROCKSDB_MAJOR`, `ROCKSDB_MINOR`, `ROCKSDB_PATCH` from the bundled `oxrocksdb-sys` source's `rocksdb/include/rocksdb/version.h`, found via `cargo metadata` of the dependency or the `DEP_ROCKSDB_*` environment, and fail the build with a clear message if not found) and `crates/engine/src/versions.rs`; make T012 pass
- [X] T014 [P] Add a test `crates/engine/tests/store_opens.rs` that opens an Oxigraph store in a temporary directory and closes it, proving the bundled RocksDB links and works (FR-021)

**Checkpoint**: the binary reads Stash input, writes envelopes and logs, and can reach a Stash
endpoint; versions are known.

---

## Phase 3: User Story 1 - Install the plugin and see it respond (Priority: P1) MVP

**Goal**: An owner installs with the platform script, reloads plugins, and `status` answers.

**Independent Test**: `sh scripts/install.sh --from-file <package> --plugins-dir <dir>`, reload
plugins in Stash, call `runPluginOperation(plugin_id: "semantic-engine", args: {op: "status"})`,
receive the versions (quickstart steps 2 and 3).

### Tests for US1

- [X] T015 [P] [US1] Write failing test `crates/plugin/tests/ops_basic.rs`: `ping` returns `{"pong": true}` with no Stash call; unknown `op` returns `UNKNOWN_OPERATION` with `details.known` listing all operation names; missing `op` returns `UNKNOWN_OPERATION`
- [X] T016 [P] [US1] Write failing test `crates/plugin/tests/ops_status.rs`: with a fake Stash returning `version { version }` = `v0.31.1`, `status` returns all fields of data-model "Versions reported by status", `serverId` null when unset, and `rocksdb` `{version, available: true}`; with Stash unreachable it returns `STASH_UNREACHABLE` with local versions in `details`; with Stash `v0.30.0` it returns `STASH_TOO_OLD` (FR-017)
- [X] T017 [P] [US1] Write failing test `scripts/tests/install_sh_test.sh` (POSIX sh, run under `dash`): `--help` exits 0; `--from-file` with a test package and matching `.sha256` into a temp plugins dir exits 0 and creates `semantic-engine/semantic-engine.yml`; a wrong checksum exits 5 and leaves the dir unchanged; an unwritable dir exits 4; a fake `uname` reporting `FreeBSD` exits 3 naming the platform and build-from-source steps; re-running replaces files and leaves an unrelated file in the plugins dir untouched
- [X] T018 [P] [US1] Write failing test `scripts/tests/install_ps1_test.ps1` covering the same cases for `scripts/install.ps1` (skips the platform case off Windows)

### Implementation for US1

- [X] T019 [US1] Implement `crates/plugin/src/ops.rs` registry with `ping` and `status` (query `version { version }` through a Cynic operation in `crates/stash-adapter/src/queries.rs`, compare against `v0.31.1`); make T015 and T016 pass
- [X] T020 [US1] Write `scripts/package.sh` (POSIX sh): build `--release`, assemble `semantic-engine/` with `semantic-engine.yml` (version substituted), the executable, `vocabulary/*`, and `VERSIONS.json` from `semantic-engine status`-equivalent build data, then write `dist/semantic-engine-<version>-<target>.tar.gz` (zip on Windows) and its `.sha256`
- [X] T021 [US1] Write `scripts/install.sh` per `contracts/install-scripts.md` using only the allowed POSIX constructs and utilities: platform detection to the targets in research R7, plugins folder discovery in research R8 order, download plus SHA-256 verification (or `--from-file`), unpack to a temp dir, replace `<plugins>/semantic-engine/`, summary and exit codes; make T017 pass
- [X] T022 [US1] Write `scripts/install.ps1` with the same behaviour and exit codes, PowerShell 5.1 compatible; make T018 pass
- [X] T023 [US1] Opt-in integration test `crates/plugin/tests/integration_install.rs` (runs only when `STASH_TEST_URL` and `STASH_TEST_PLUGINS_DIR` are set): package, install with `scripts/install.sh --from-file`, call `reloadPlugins`, call `status` through `runPluginOperation`, assert `ok: true`; record the round-trip time to stdout (SC-005 baseline)

**Checkpoint**: US1 complete; the plugin installs on Unix and Windows and answers status.

---

## Phase 4: User Story 2 - One identifier per server (Priority: P1)

**Goal**: A task creates the server UUID once; every client reads the same value.

**Independent Test**: run the task twice, read `configuration { plugins(include:
["semantic-engine"]) }`, same UUID both times (quickstart step 4).

### Tests for US2

- [X] T024 [P] [US2] Write failing test `crates/stash-adapter/tests/plugin_config.rs`: reading settings uses `configuration { plugins(include: ["semantic-engine"]) }`; writing uses `configurePlugin` with the **merged** map (existing keys preserved, since `configurePlugin` replaces the whole map, research R4)
- [X] T025 [P] [US2] Write failing test `crates/engine/tests/server_id.rs`: absent → generates a value matching `^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`, writes it, re-reads, reports `generated`; present and valid → unchanged, reports `existing`; present and invalid → `SERVER_ID_INVALID`, no write; the read path never generates
- [X] T026 [P] [US2] Write failing test `crates/plugin/tests/ops_server_id.rs`: `getServerId` returns `{"serverId"}` or `SERVER_ID_NOT_GENERATED` / `SERVER_ID_INVALID`; task `generateServerId` logs `serverId=<uuid> (generated)` then `(existing)` on a second run; `status` includes `serverId` once generated

### Implementation for US2

- [X] T027 [US2] Implement `crates/stash-adapter/src/plugin_config.rs` (Cynic query and mutation for plugin settings, read-merge-write); make T024 pass
- [X] T028 [US2] Implement `crates/engine/src/server_id.rs` per data-model "Server identifier" lifecycle; make T025 pass
- [X] T029 [US2] Register `getServerId` and task `generateServerId` in `crates/plugin/src/ops.rs` and add `serverId` to `status`; make T026 pass
- [X] T030 [US2] Opt-in integration test `crates/plugin/tests/integration_server_id.rs` (gated on `STASH_TEST_URL`): run the task twice via `runPluginTask`, wait for the job, read the configuration twice through two separate client connections, assert one identical UUID

**Checkpoint**: US2 complete; item IRIs (feature 001 FR-014) can now be built.

---

## Phase 5: User Story 3 - A written API document clients can build against (Priority: P2)

**Goal**: One document states every operation, shape, error, and version, and cannot drift.

**Independent Test**: the agreement test passes; a reader implements a `status` call from
`docs/api.md` alone.

### Tests for US3

- [X] T031 [P] [US3] Write failing test `crates/plugin/tests/api_agreement.rs`: the set of operation and task names in the plugin registry, in `api/operations.json`, and in `docs/api.md` operation headings are equal; every error code the code can return is in `api/operations.json` and `docs/api.md`; the contract version is the same in all three (FR-012)

### Implementation for US3

- [X] T032 [US3] Write `docs/api.md` from `contracts/operations.md`: how to call (`runPluginOperation`, `runPluginTask`), the envelope, one section per operation and task with request, response, errors, and "since", the error code table, compatibility rules, how to read `serverId` without the engine, and vocabulary terms referenced by IRI (`urn:semantic-stash:vocab:`); make T031 pass
- [X] T033 [US3] Add a "Changes" section to `docs/api.md` with the 1.0.0 entry and a note that `api/operations.json` and the code change in the same revision

---

## Phase 6: User Story 4 - Repeatable builds with downloadable binaries (Priority: P2)

**Goal**: Every change is checked; a tag produces packages for every supported target.

**Independent Test**: a deliberately broken change in each checked area fails CI; a tag
produces packages and `SHA256SUMS` that install per US1.

- [ ] T034 [US4] Create `.github/workflows/ci.yml` replacing `vocabulary.yml`: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `vocab-check`, `scripts/validate-ttl.sh`, `shellcheck -s sh scripts/*.sh`, `dash scripts/tests/install_sh_test.sh`, `pwsh scripts/tests/install_ps1_test.ps1`; runners get a C++17 compiler and clang (bundled RocksDB); delete `.github/workflows/vocabulary.yml`
- [ ] T035 [US4] Create `.github/workflows/release.yml` triggered on `v*` tags: matrix over `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (oldest supported Ubuntu runner), `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`; run `scripts/package.sh` (PowerShell equivalent on Windows); upload packages and a combined `SHA256SUMS` to the release
- [ ] T036 [P] [US4] Add `scripts/package.ps1` mirroring `scripts/package.sh` for Windows (zip and `.sha256`)
- [ ] T037 [US4] Document in `README.md` the release artifacts, the supported targets, and the SHA-256 verification the install scripts perform; replace the "Planned" install section with the real commands

---

## Phase 7: User Story 5 - Capture test fixtures without changing the library (Priority: P3)

**Goal**: Read-only capture of the semantic data for later features' tests.

**Independent Test**: run against the test instance, files written, no credentials in them,
library counts unchanged (quickstart step 6).

### Tests for US5

- [ ] T038 [P] [US5] Write failing test `crates/stash-adapter/tests/fixture_capture.rs` against a fake Stash: the tool issues only queries (assert no request body contains `mutation`), writes one file per query plus `index.json` with counts and empty sections, and no file contains the API key, cookie, or header values (FR-015, SC-006)

### Implementation for US5

- [ ] T039 [US5] Add Cynic query operations in `crates/stash-adapter/src/fixtures.rs` for flagged tags (`custom_fields` "flag is not null" filters per Principle VII), performers with archetype flags, and scenes and images with semantic `custom_fields`, selecting only the fields the vocabulary mapping needs
- [ ] T040 [US5] Implement `crates/stash-adapter/src/bin/fixture-capture.rs`: reads `STASH_URL` and `STASH_API_KEY` from the environment, `--out DIR`, runs T039 queries, scrubs, writes files and `index.json`; make T038 pass
- [ ] T041 [US5] Run the tool against the test instance (`localhost:9998`) into `crates/stash-adapter/tests/fixtures/test-instance/`, confirm library counts before and after match, and commit the scrubbed fixtures

---

## Phase 8: Polish and cross-cutting

- [ ] T042 [P] Add `STASH_TOO_OLD`, `STASH_UNREACHABLE`, and log-redaction checks to `crates/plugin/tests/security.rs`: no stdout, stderr, or file written by the plugin contains the session cookie value; the plugin opens no listening socket (FR-016)
- [ ] T043 [P] Update `ROADMAP.md` Phase 0: tick the workspace, server identity task, manifest, install scripts, CI, fixture tooling, and API document items, linking `specs/002-engine-plugin-foundation/`
- [ ] T044 Run every step of `specs/002-engine-plugin-foundation/quickstart.md` against the test instance and fix drift; record the SC-005 status timing in `specs/002-engine-plugin-foundation/quickstart.md`
- [ ] T045 [P] SC-003 check: give `docs/api.md` to a reader who has not seen the engine and record whether a first-attempt `status` call works, in `specs/002-engine-plugin-foundation/checklists/requirements.md` notes

---

## Dependencies and execution order

- Phase 1 → Phase 2 → stories. Phase 2 blocks everything.
- US1 (P1) first: it delivers the installable plugin and the packaging the other stories use.
- US2 (P1) depends on Phase 2 only (adapter and plugin I/O); it can run in parallel with US1's
  script work.
- US3 depends on US1 and US2 (it documents their operations).
- US4 depends on US1 (`scripts/package.sh`, install script tests).
- US5 depends on Phase 2 only (adapter).
- Polish depends on all stories.

### Parallel opportunities

- T003, T004, T005; T006, T007, T008, T012; T010 with T011 after their tests; T014.
- US1 tests T015 to T018 together; then T019 alongside T021 and T022 (different files).
- US2 tests T024 to T026 together.
- US5 (T038 to T041) alongside US3 once Phase 2 is done.
- Polish T042, T043, T045.

### Implementation strategy

1. Phases 1 and 2.
2. US1 as the MVP: installable plugin answering `status`, with both install scripts.
3. US2 (server identifier), then US3 (API document), US4 (CI and release), US5 (fixtures).
4. Polish; commit after each phase.
