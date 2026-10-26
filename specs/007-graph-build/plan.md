# Implementation Plan: The Semantic Graph from the Documents

**Branch**: `master` (spec directory `007-graph-build`) | **Date**: 2026-10-26 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/007-graph-build/spec.md`

## Summary

This feature builds the semantic graph. A new engine module, `engine::build`, has two layers:
- **The mapping:** it turns the reader's records (spec 005) into facts, following every active
  entry of `stash-mapping.ttl`. The rules are written out, key by key, in
  [contracts/mapping-rules.md](contracts/mapping-rules.md).
- **The build:** it writes the facts to the store (spec 006), and publishes one generation.

A new task, "Build semantic graph" (operation `buildGraph`), runs a full rebuild from Stash.

**The design points:**
- **The mapping is pure:** a function of the records and the server id, tested by fixtures with
  no Stash and no store (R1, R10).
- **The order:** stamp check, then the lock, then the read. The lock is taken before the read,
  so a second build ends at once with the new `BUSY` code (R4).
- **A fresh working store:** a new `GraphStore::rebuild_writer()` starts from an empty one. The
  bulk loader then takes every batch, and the published generations stay current until the new
  one is published (R3).
- **Stopping:** Stash kills the process (Stash 0.31.1 `raw.go`). Safety comes from 006's single
  publish, its dirty marker and the operating system's lock release. A kill test proves it (R2).
- **Bad data:** it is skipped or noted field by field, with ten note reasons. Notes never hold
  values, and at most 1,000 are logged (R8).
- **The vocabulary facts are written too,** so field rules resolve through `pluginFieldName` and
  queries can use sub-classes (R7).
- **Vocabulary 1.4.0 (MINOR):** `pairsA` and `pairsB`, `roleAppliedAutomatically`, the real
  pairing direction values, and a clarified Derived status (R6).
- **Contract 1.2.0 (MINOR):** the new task and `BUSY` (R12).

## Technical Context

**Language/Version**: Rust stable (1.99 here), edition 2021

**Primary Dependencies**:
- the workspace crates `stash-adapter` (the reader), `semantic-vocabulary` (IRIs and the embedded
  vocabulary) and `engine::graph`;
- `oxigraph` 0.4, only inside `engine::graph`;
- `serde` and `serde_json`;
- no new dependency.

**Storage**: the store of spec 006, under `{PluginDir}/graph/`. It is disposable (Principle I).
The fixtures live under `crates/engine/tests/fixtures/build/`.

**Testing**:
- `cargo test -p engine`: the fixture, coverage, test-instance, notes, determinism, run and kill
  tests (R10);
- `cargo test -p plugin`: the task handler and the contract agreement;
- `vocab-check` for vocabulary 1.4.0;
- `build-bench` for the budgets (R11).

None of the tests needs Stash or a network (FR-022).

**Target Platform**: the five release targets of 002. Measurements are taken on Linux. CI runs
the tests on Linux, macOS and Windows.

**Project Type**: Rust Stash plugin (library crates plus a thin plugin binary)

**Performance Goals**:
- a build of Production, from Stash, under 5 minutes;
- a synthetic, fully tagged library of Production size (50,000 scenes, 29,000 images), under 5
  minutes (SC-004);
- expected: about 3 million facts and about 25 s of loading, at 006's measured rate.

**Constraints**:
- peak memory under 2 GB at synthetic Production size, stated and measured (R11);
- Stash is only read; only `{PluginDir}/graph` is written;
- no credential, performer name, or field value in any log line;
- no Oxigraph type outside `engine::graph` (006 G9).

**Scale/Scope**:
- one engine module of about 5 files, one plugin handler, one bench binary;
- about 7 test files and one fixture set;
- one vocabulary MINOR, and one contract MINOR.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Checked against
constitution v3.0.0.

| Principle | Status | Notes |
|---|---|---|
| I. Stash is the system of record | Pass | The graph is rebuilt from Stash, entirely, on every build. The build only reads Stash. A store that needs a rebuild is cleared and built again (R4). |
| II. Plugin documents are the contract | Pass | Every fact follows a `stash-mapping.ttl` entry (FR-001), and the coverage test enforces it (R10). Four gaps found in mapping are corrected in the vocabulary in this feature (R6). Each is sourced from the documents, the plugin's code read as evidence, or spec 004. |
| III. One engine, one set of answers | Pass | The mapping is in the engine. Relation definitions are parsed by the same code as the `relations` operation (R5). No client receives the store. |
| IV. Reached only through Stash | Pass | A Stash task, with progress through Stash's log. There is a single writer, and readers see only published generations. Hooks are untouched, so hook cost is unchanged. |
| V. Rust, embedded, headless | Pass | Pure Rust in `engine`. Oxigraph stays behind `engine::graph`. No `unsafe`, and no new dependency. |
| VI. Versioned, explicit contracts | Pass | Contract 1.2.0 and vocabulary 1.4.0 are MINOR, additive releases. Errors are structured, with the new `BUSY` code. Notes are structured codes (R8, R12). |
| VII. Fast, bounded, test-first | Pass | Budgets are stated (SC-004, peak memory) and measured by `build-bench`. Compatibility fixtures come first. A stopped build is proven safe by a kill test (R2). |
| Technology: bundled RocksDB, no CMake | Pass | No build change. |

No violations. Complexity Tracking not required.

**Re-check after Phase 1 design**: Pass.
- **Mapping rules:** written per key, with a fact list for each, so the fixture's expected
  graph can be written from them by hand.
- **Vocabulary changes:** additive only, with no domain or range changes, so V014 holds at
  MINOR.
- **Store interface:** one additive method.
- **Main measured risk:** peak memory, because the full read is held in memory by design
  (R4). It is stated and measured, and 006 peaked at 1.7 GB for a larger graph.

## Project Structure

### Documentation (this feature)

```text
specs/007-graph-build/
├── plan.md
├── research.md                    # R1–R12
├── data-model.md                  # Mapped, TagIndex, Note, NoteReason, BuildReport, BuildError, states
├── quickstart.md                  # offline tests, synthetic budget, test instance, Production
├── contracts/
│   ├── mapping-rules.md           # every key and the facts it writes
│   ├── vocabulary-1.4.md          # pairsA/B, roleAppliedAutomatically, clarified comments
│   └── operations-1.2.md          # buildGraph task, BUSY, rebuild_writer
├── checklists/requirements.md
├── results/                       # build-bench output (measurement data)
└── tasks.md                       # /speckit-tasks
```

### Source Code (repository root)

```text
crates/engine/
├── src/
│   ├── build/
│   │   ├── mod.rs              # public surface: map_library, run, BuildOptions, BuildReport, BuildError
│   │   ├── map.rs              # records → facts: tags, performers, media, assignments, pairings, relations
│   │   ├── timeline.rs         # timeline entries, stage picks, Mise-en-scène instances, atTime
│   │   ├── labels.rs           # Label formulas and Field Mapping slots
│   │   ├── notes.rs            # Note, NoteReason, dedup and the log cap
│   │   ├── vocab.rs            # the embedded vocabulary as facts
│   │   └── run.rs              # the build order (R4), batches, progress, report
│   ├── graph/mod.rs            # + rebuild_writer() (R3)
│   ├── graph/term.rs           # + Ord on Term and Fact (R9)
│   ├── versions.rs             # CONTRACT_VERSION 1.2.0
│   ├── errors.rs               # + BUSY
│   └── bin/build-bench.rs      # synth, stash (R11)
└── tests/
    ├── fixtures/build/         # library.json, expected.nt, expected-notes.json, README.md
    ├── build_fixture.rs
    ├── build_coverage.rs
    ├── build_test_instance.rs
    ├── build_notes.rs
    ├── build_determinism.rs
    ├── build_run.rs
    └── build_kill.rs
crates/semantic-vocabulary/src/iri.rs   # + valid_id()
crates/plugin/src/build.rs              # buildGraph handler
crates/plugin/src/ops.rs                # + registry entry
plugin/semantic-engine.yml              # + task
vocabulary/                             # 1.4.0: core.ttl, stash-mapping.ttl, CHANGELOG, TERMS, released/1.4.0.json
api/operations.json, docs/api.md        # 1.2.0
specs/006-graph-store/contracts/graph-store-interface.md   # + rebuild_writer
.github/workflows/ci.yml                # the package check also rejects build-bench
ROADMAP.md                              # tick the three Phase 2 items
```

**Structure Decision**:
- **Where the code lives:** the build is in `engine`, beside `graph` and `relations`. The
  plugin crate gets only the handler.
- **The bench binary:** `build-bench` is in `engine/src/bin`, which `scripts/package.sh` never
  packages.
- **The test-instance fixture:** read in place from the adapter's fixtures, so there is one
  captured library.

## Complexity Tracking

No constitution violations to justify.
