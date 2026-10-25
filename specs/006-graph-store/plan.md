# Implementation Plan: Graph Store, Oxigraph Embedded

**Branch**: `master` (spec directory `006-graph-store`) | **Date**: 2026-10-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/006-graph-store/spec.md`

## Summary

This feature adds the engine's graph store: Oxigraph embedded, on the bundled RocksDB, behind a
module the engine owns (`engine::graph`, contracts/graph-store-interface.md). The store follows
the model the spike proved and constitution Principle IV fixed:

- **One writer:** a single writer holds `writer.lock`, and writes a working store.
- **Publishing:** it publishes numbered generations as RocksDB checkpoints and switches
  `current` with an atomic rename.
- **Readers:** they open only the current generation, read-only, and keep it for the whole call.

The spike's store code (`crates/spike/src/store.rs`) is the starting point. This feature adds:
- **Engine types:** facts, terms and solutions, so no Oxigraph type leaks to callers (research
  R1).
- **A stamp:** format version and server id, checked before every read, with four distinct
  rebuild reasons (R2, R4).
- **A dirty marker:** a crash before publish resets the working store to the last published
  state (R3).
- **Cleanup:** keeps the current generation, the previous one, and those younger than 15
  minutes, and tolerates Windows file locks (R5).
- **A fact count recorded at publish:** the summary stays cheap (R6).
- **SELECT queries with structured errors:** the error includes the line and column where
  available (R7).

A development binary, `graph-bench`, measures the budgets at 250,000 scenes (R8). A
multi-process test checks the no-partial-read guarantee (R9).

The graph's content (vocabulary mapping), incremental builds, and client query operations are
the next features. The operations contract stays 1.1.0.

## Technical Context

**Language/Version**: Rust stable (1.99 here; 1.89 or newer for `File::try_lock`), edition 2021

**Primary Dependencies**:
- `oxigraph` 0.4 (already in `engine`; 0.4.11 in `Cargo.lock`), with its bundled RocksDB
  (constitution, Technology & Platform Constraints);
- `serde` and `serde_json`, already used, for the stamp and the generation files;
- no new dependency. Times are formatted with a small helper, as the plugin's journal does.

**Storage**: `{PluginDir}/graph/` (research R2): `store.json`, `work/`, `work.dirty`,
`snapshots/<n>/`, `snapshots/<n>.json`, `current`, `writer.lock`. All of it is disposable
(Principle I).

**Testing**:
- `cargo test -p engine`, with temporary store folders;
- a multi-process concurrency test (R9);
- `graph-bench` for the 250,000-scene budgets;
- no Stash and no network.

**Target Platform**: the five release targets of 002. Measurements are taken on Linux; CI runs
the tests on Linux, macOS and Windows.

**Project Type**: Rust Stash plugin (library crates plus a thin plugin binary)

**Performance Goals**:
- opening a generation: under 15 ms median (SC-002);
- a query call at 250,000 scenes: under 200 ms at p95 (SC-003);
- a second writer learns it is busy in under 50 ms (SC-004).

**Constraints**:
- readers never open `work/`; a published generation's files never change;
- disk use: at most 3 times one generation after cleanup (SC-007);
- no host contacted; no credential written;
- no `oxigraph` type in the public interface.

**Scale/Scope**:
- libraries up to 250,000 scenes, at about 8.3 KB per scene per generation;
- one module of about 5 files, one bench binary, 6 test files.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Checked against
constitution v3.0.0.

| Principle | Status | Notes |
|---|---|---|
| I. Stash is the system of record | Pass | The store is a cache under the plugin's folder, rebuilt when missing, damaged or of another format, and refused when stamped for another server (R2, R4). |
| II. Plugin documents are the contract | Pass (not touched) | No semantic data is read or written in this feature. The mapping is the next feature. |
| III. One engine, one set of answers | Pass | Queries run only in the engine; nothing hands a client the store or a generation (decision record rule 10). |
| IV. Reached only through Stash | Pass | The spike's model exactly: a single writer, published generations, readers of published generations only (rules 1–3). No listener; no host contacted. Hooks are untouched, so hook cost is unchanged. |
| V. Rust, embedded, headless | Pass | Oxigraph is embedded behind the engine-owned module `engine::graph`, with no Oxigraph types in its interface (R1, G9). RocksDB's C++ is inside Oxigraph's bundled build; the engine adds no `unsafe` and no FFI of its own. |
| VI. Versioned, explicit contracts | Pass | The store format carries a version (`STORE_FORMAT`). Errors are structured, query errors with a position (R7). The operations contract is unchanged. |
| VII. Fast, bounded, test-first | Pass | Budgets are stated (SC-002 to SC-004, SC-007) and measured by `graph-bench`. Tests come first and run with no Stash. Building cannot be stopped mid-publish, but a stopped writer simply never publishes (R3). |
| Technology: bundled RocksDB, no CMake | Pass | It uses the RocksDB Oxigraph already bundles, with no change to the build. |

No violations. Complexity Tracking not required.

**Re-check after Phase 1 design**: Pass.
- **Interface:** internal, with no Oxigraph types.
- **Format:** the store format is versioned.
- **Stamp:** it lives in a plain file outside the RDF store, so a damaged store and a foreign one
  can be told apart (R2).
- **One measured risk:** the 250,000-scene open and query budgets. The spike met both with the
  same layout, and `graph-bench` checks them again.

## Project Structure

### Documentation (this feature)

```text
specs/006-graph-store/
├── plan.md
├── research.md                       # R1–R9
├── data-model.md                     # Term, Fact, Pattern, Generation, stamp, Health, Solutions, Summary, GraphError
├── quickstart.md                     # offline tests, 250k budgets, package check
├── contracts/
│   └── graph-store-interface.md      # GraphStore, Reader, Writer; rules G1–G9; constants
├── checklists/requirements.md
├── results/                          # graph-bench output (measurement data)
└── tasks.md                          # /speckit-tasks
```

### Source Code (repository root)

```text
crates/engine/
├── src/
│   ├── graph/
│   │   ├── mod.rs              # GraphStore, constants, re-exports (the only public surface)
│   │   ├── term.rs             # Term, Fact, Pattern, Solutions; conversion to and from Oxigraph
│   │   ├── layout.rs           # paths, the stamp, current, generation files, atomic writes
│   │   ├── health.rs           # check(): the R4 order of checks
│   │   ├── writer.rs           # lock, dirty marker and reset, add, remove, clear, publish, cleanup
│   │   ├── reader.rs           # open the current generation; select with structured errors
│   │   └── error.rs            # GraphError, Health and rebuild reasons
│   ├── bin/graph-bench.rs      # build, open, query, disk, stress (R8)
│   └── lib.rs                  # pub mod graph
└── tests/
    ├── graph_write_publish.rs
    ├── graph_read.rs
    ├── graph_health.rs
    ├── graph_summary.rs
    ├── graph_concurrency.rs    # multi-process (R9)
    └── graph_interface.rs      # G9: no oxigraph in public signatures
.github/workflows/ci.yml        # the package check also rejects graph-bench
ROADMAP.md                      # tick the two Phase 2 items
```

**Structure Decision**:
- **Where the store lives:** in `engine`, which already depends on Oxigraph. The plugin binary
  will reach it only through the engine.
- **The spike:** `crates/spike` stays as history. Its store code is the reference, not a
  dependency.
- **Bench binary:** `graph-bench` lives in `engine/src/bin`, which `scripts/package.sh` never
  packages (it builds only `-p plugin`).

## Complexity Tracking

No constitution violations to justify.
