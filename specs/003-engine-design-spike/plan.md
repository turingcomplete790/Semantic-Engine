# Implementation Plan: Engine Design Spike

**Branch**: `003-engine-design-spike` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-engine-design-spike/spec.md`

## Summary

Answer Principle IV's open questions with measurements, then write one decision record. A
throwaway spike crate builds two binaries:
- **`spike-plugin`**, installed in Stash as a separate plugin (`semantic-engine-spike`), never
  packaged. It opens an on-disk Oxigraph store, runs small queries, returns answers of a chosen
  size, writes under each sharing option, serves snapshot files, and records every hook it
  receives.
- **`spike-bench`**, a command-line harness. It drives that plugin through Stash's GraphQL API,
  makes the same saves the web UI, the semantic-tagging plugin and the viewer make, and writes
  each result as JSON next to the decision record.

Graph data comes from two places:
- **Real data:** semantic test data seeded onto the test server, which the cleanup command
  reverses.
- **Synthetic data:** graphs shaped like the vocabulary at the test library's size and at
  50,000, 100,000 and 250,000 scenes. The owner's Production library holds 49,683 scenes (with
  clips) on 2026-10-16 and is growing daily, so the viewer's old 50,000 design scale is the floor,
  not the ceiling.

The source reading behind the plan is in [research.md](research.md). Two findings reshape the
spike:
- **Hooks block saves:** Stash runs post-hooks synchronously, so every hook call adds to the
  client's save time.
- **Read-only alongside a writer is unsafe:** Oxigraph documents opening a store read-only while
  another process writes it as undefined behaviour.

## Technical Context

**Language/Version**: Rust stable (edition 2021), same workspace

**Primary Dependencies**: existing crates (`stash-adapter` with Cynic, `engine`,
`semantic-vocabulary`); `oxigraph` 0.4.11 with bundled RocksDB; `serde_json`; the standard library's
advisory file locks (`File::try_lock`) for the writer lock; no new network crate

**Storage**: Oxigraph stores under the spike plugin's folder (`<plugin dir>/store/`), including
published snapshot generations; hook log as JSON lines in the same folder; measurement results as
JSON in `specs/003-engine-design-spike/results/`

**Testing**: unit tests for the harness's statistics and for snapshot publication with no live
Stash; every measurement itself runs against the local test server (Stash v0.31.1,
localhost:9998), opt-in through the same `STASH_TEST_*` environment variables 002 uses

**Target Platform**: measurements on Linux x86_64 (the test server's host); build time and package
size from CI for all five release targets

**Project Type**: spike crate (two binaries) plus a written decision record

**Performance Goals**: measured against the viewer's 009 budgets: first build ≤ 5 min (SC-001),
ready ≤ 2 s from a stored copy (SC-002), Phase 1 query first results < 200 ms (SC-003), change
visible ≤ 60 s (SC-004); spike-specific: answer size at which a call exceeds 1 s (spec FR-008)

**Constraints**: no listening socket, no host but the invoking Stash, no credential logged or stored
(Principle IV); writes only to test data on the test server, reversible (spec FR-012); nothing from
the spike in release packages or the 1.0.0 contract (spec FR-013)

**Scale/Scope**:
- Libraries: the test library (1,021 scenes, 1,327 images, 6 performers, 17 tags). Synthetic
  graphs at 1,021, 50,000, 100,000 and 250,000 scenes (Production: 49,683 and growing).
- Clients: 3 saving clients × 4 entity types.
- Options: 3 sharing options, 2 snapshot delivery paths, 1 long-lived process evaluation.
- Output: 1 decision record.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Stash is the system of record | Pass | Spike stores are disposable caches; seeded test data is recorded and reversed (R8) |
| II. Plugin documents are the contract | Pass | Synthetic graphs use vocabulary 1.1.0 terms only; seeded data follows the documented custom-field keys (`stash-mapping.ttl`) |
| III. One engine, one set of answers | Pass | Produces the shared decision both clients rely on; no client-visible operations |
| IV. Reached only through Stash | Pass | The spike plugin is invoked only through Stash; the long-lived-process option is evaluated on paper first and dropped if it needs a socket (R3); session cookie and API key never logged |
| V. Rust, embedded, headless | Pass | Oxigraph embedded; spike code in its own crate, reusing the adapter for all Stash I/O |
| VI. Versioned, explicit contracts | Pass | Contract 1.0.0 unchanged; spike plugin has its own id and manifest, never published (R9) |
| VII. Fast, bounded, test-first | Pass | Every figure comes from a repeatable command; fetches stay flag-filtered (reusing `fixtures.rs` filters); statistics and snapshot logic tested first |
| Technology: Cynic | Pass | Saves and plugin calls made through typed Cynic operations in the adapter (new mutations added to the adapter, test-only feature) |
| Technology: bundled RocksDB | Pass | Same `oxigraph` default build |

No violations. Complexity Tracking not required.

**Re-check after Phase 1 design**: Pass.
- The spike plugin's manifest declares hooks only for the four semantic entity types (R4).
- Mutations exist only behind the `spike` feature of the adapter, so the shipped plugin still
  compiles queries only for Stash data (002 R10).
- Snapshot files are served from the spike plugin's own folder through Stash's plugin asset
  route (R6).

**Re-check against constitution v3.0.0 (2026-10-20)**: Pass.
- **Principles I and III (clients depend on the engine; no client copies or engines):** the
  spike's snapshot and change-file operations are measurement-only. They are never packaged
  (FR-013), and the decision record rejects client snapshots.
- **Principle IV (settled model):** the decision record adopts what v3.0.0 states: published
  generations, a single writer, and hooks for freshness with `updated_at` catch-up.

## Project Structure

### Documentation (this feature)

```text
specs/003-engine-design-spike/
├── plan.md
├── research.md
├── data-model.md                # measurement, option, hook observation, decision, seed manifest
├── quickstart.md                # how to run every measurement and write the record
├── contracts/
│   ├── spike-bench.md           # harness subcommands, arguments, result JSON, exit codes
│   ├── spike-plugin.md          # spike plugin manifest, operations, hooks, assets
│   └── decision-record.md       # required shape of decision-record.md
├── results/                     # JSON written by spike-bench (committed)
├── decision-record.md           # the deliverable (written last)
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── stash-adapter/
│   └── src/spike.rs             # NEW, feature "spike": typed mutations (sceneUpdate, bulkSceneUpdate,
│                                #   tagUpdate, tagsMerge, tagDestroy, performerUpdate, imageUpdate,
│                                #   bulkImageUpdate) and runPluginOperation
└── spike/                       # NEW, publish = false, never built by package scripts
    ├── Cargo.toml
    ├── src/lib.rs               # module list
    ├── src/store.rs             # store layout, snapshot publish/open, writer lock, generation copy
    ├── src/measure.rs           # Measurement records, statistics, no-overwrite result files
    ├── src/synth.rs             # vocabulary-shaped synthetic graphs at a given scene count
    ├── src/plugin_ops/          # spike plugin operations (query, write, buildFromStash, recordHook,
    │                            #   payload, exportSnapshot, snapshotInfo, applyEdit)
    ├── src/bench/               # harness subcommands in contracts/spike-bench.md
    ├── src/bin/spike-plugin.rs  # raw-interface plugin entry point
    └── src/bin/spike-bench.rs   # harness entry point
spike/
└── semantic-engine-spike.yml    # spike plugin manifest (separate plugin id)
scripts/
├── spike-install.sh             # builds spike-plugin and copies it with its manifest into a plugins dir
└── spike-build-cost.sh          # build time and release sizes (results/build-release.json)
```

**Structure Decision**:
- **Separate crate:** spike code lives in `crates/spike` so `scripts/package.sh` (which builds
  only `-p plugin`) never picks it up. Deleting the crate later removes the spike in one step.
- **Separate plugin:** it installs under its own id, so the real plugin, its contract and its
  `serverId` setting are untouched.
- **Adapter stays the only Stash client:** all Stash I/O still goes through `stash-adapter`, and
  its spike-only mutations sit behind a feature flag.

## Complexity Tracking

No constitution violations to justify.
