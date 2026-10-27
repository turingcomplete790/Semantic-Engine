# Implementation Plan: Incremental Graph Update

**Branch**: `master` (spec directory `008-incremental-graph-update`) | **Date**: 2026-10-26 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/008-incremental-graph-update/spec.md`

## Summary

A new task, "Update semantic graph" (`updateGraph`), brings the published graph up to date with
what changed since the last build or update, and rebuilds in full when an update cannot be
trusted. The design rests on four decisions:

- **One named graph per item (R1, store format 2):** every tag, performer, scene and image owns
  the graph named by its IRI, so removing or replacing an item is clearing its graph. Queries see
  the union, so their answers do not change. Owners never overlap, by construction of spec 007's
  mapping.
- **A kept fact count (R2):** measured at 250,000 scenes, `Store::len` is 2.4–3.1 s of the
  publish, while flush and checkpoint take under 40 ms. The writer keeps the count, and an
  update's publish drops to about 40 ms.
- **One semantic rule (R3):** the full read's filter moves into the reader as public functions,
  so an update and a full rebuild agree on which items exist.
- **What to re-map (R4, R5):**
  - every semantic tag, read whole each time (few, and they drive other items' facts);
  - changed performers, scenes and images, from `updated_at`;
  - performers carrying a tag whose facts changed;
  - deletions, from id lists against the store's owners.

  Graphs are replaced only when their facts actually differ (R6), so a no-op save publishes
  nothing.

Each generation records a build stamp: the mapping version, the vocabulary version, and the
catch-up point (R7). Correctness is proven by equivalence: after at least 20 scripted change
sequences, the updated graph equals a full rebuild, fact for fact (R8).

The contract moves to 1.3.0. Hooks stay with Phase 3.

## Technical Context

**Language/Version**: Rust stable (1.99), edition 2021

**Primary Dependencies**:
- the workspace crates `stash-adapter` (reader additions), `engine::graph` (format 2) and
  `engine::build` (spec 007);
- `oxigraph` 0.4, only inside `engine::graph`, for named graphs and union-default-graph queries;
- no new dependency.

**Storage**: the store of spec 006, now format 2: named graphs per owner, and a `build` stamp in
each generation file. Format-1 stores rebuild on their first update.

**Testing**:
- **Engine:** the owned-graph store tests, equivalence, fallback, current, reads, kill, and owner
  disjointness;
- **Adapter:** the semantic-rule and new-method tests;
- **Plugin:** the contract tests;
- **Benches:** `build-bench update` and `graph-bench query` at 250,000 scenes.

None of the tests needs Stash.

**Target Platform**: the five release targets. Measurements on Linux; CI runs the tests on
Linux, macOS and Windows.

**Project Type**: Rust Stash plugin (library crates plus a thin plugin binary)

**Performance Goals**:

| Case | Target |
|---|---|
| 1 changed scene, 250,000 scenes, end to end | under 5 s |
| 1,000 changed scenes, 250,000 scenes | under 60 s |
| No change, Production size | under 2 s, and nothing published |
| Query p95 at 250k, on format 2 | under 200 ms (006 SC-003, kept) |

**Constraints**:
- an updated graph equals a full rebuild, fact for fact;
- Stash is only read;
- one writer;
- no credential or value in any log;
- no Oxigraph type outside `engine::graph`.

**Scale/Scope**: about 4 new files in `engine::build` (update, change set, stamp, report), store
additions in 2 files, reader additions in 3, one plugin handler, about 8 test files, and one
bench command.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Checked against
constitution v3.0.0.

| Principle | Status | Notes |
|---|---|---|
| I. Stash is the system of record | Pass | The graph stays a disposable cache. Every update is checked equal to a full rebuild from Stash. Whenever an update cannot be trusted (store, mapping, vocabulary, catch-up point, change volume), it rebuilds from Stash (R5, R7). |
| II. Plugin documents are the contract | Pass | No mapping rule changes. The update applies spec 007's rules, record by record. A mapping version guards against mixing rule sets. |
| III. One engine, one set of answers | Pass | The semantic rule is defined once, in the reader, for full reads and updates (R3). Queries see one union graph. |
| IV. Reached only through Stash | Pass | A Stash task. A single writer, published generations, readers of published generations. No hooks are added, so save cost is unchanged. Catch-up uses `updated_at` in Stash's own text, and deletions an id comparison (decision record rules 7 and 8). |
| V. Rust, embedded, headless | Pass | Named graphs and union queries are inside `engine::graph`, with no Oxigraph types outside it (G9). |
| VI. Versioned, explicit contracts | Pass | Store format 2, the reader additions, contract 1.3.0 (MINOR), and a mapping version in each generation. |
| VII. Fast, bounded, test-first | Pass | Budgets are stated and measured (R10). Update reads are bounded by changes, apart from the semantic tags (R4, stated). Tests come first: equivalence, fallback, kill. |
| Technology: bundled RocksDB, no CMake | Pass | No build change. |

No violations. Complexity Tracking not required.

**Re-check after Phase 1 design**: Pass.
- **Format 2** is a versioned store change, and old stores rebuild.
- **Owner disjointness** is a stated rule, with a test.
- **Two risks are measured, not assumed:**
  - query cost over the union graph, against 006's 200 ms p95;
  - the one-changed-scene budget, which R2's measurement makes reachable.

## Project Structure

### Documentation (this feature)

```text
specs/008-incremental-graph-update/
├── plan.md
├── research.md                       # R1–R11, with the publish measurement
├── data-model.md                     # build stamp, ownership, change set, UpdateReport
├── quickstart.md
├── contracts/
│   ├── graph-store-format-2.md       # Owned, Replaced, owned-graph methods, G10–G13
│   ├── reader-additions.md           # is_semantic, semantic_tags, performers_with_tags
│   └── operations-1.3.md             # updateGraph
├── checklists/requirements.md
├── results/                          # build-bench update, graph-bench query
└── tasks.md                          # /speckit-tasks
```

### Source Code (repository root)

```text
crates/stash-adapter/src/reader/
├── records.rs            # + is_semantic on the three record types, performer_tag_set
├── mod.rs                # + semantic_tags, performers_with_tags on the trait
├── client.rs             # + both methods; read_all filters through is_semantic
└── memory.rs             # + both methods
crates/engine/src/graph/
├── mod.rs                # STORE_FORMAT 2; Owned, Replaced re-exports
├── writer.rs             # add_owned, replace_owned, remove_owned, owners, owners_referencing,
│                         #   kept count, publish_with
├── reader.rs             # queries over the union of graphs
├── layout.rs             # Generation.build
└── term.rs               # owned quads (graph name) conversion
crates/engine/src/build/
├── run.rs                # the full build writes owned graphs and the build stamp
├── stamp.rs              # BuildStamp, MAPPING_VERSION, reading and checking
├── update.rs             # the update order of R5, fallback, UpdateReport
└── mod.rs                # + re-exports
crates/engine/src/bin/
├── build-bench.rs        # + update --change K
└── graph-bench.rs        # build writes owned graphs (format 2)
crates/engine/tests/
├── graph_owned.rs, owners_disjoint.rs
├── update_equivalence.rs, update_fallback.rs, update_current.rs, update_reads.rs, update_kill.rs
└── common/update.rs      # library mutations, CountingReader, full-rebuild comparison
crates/stash-adapter/tests/reader_semantic.rs
crates/plugin/src/build.rs     # + update_graph handler
crates/plugin/tests/ops_update.rs
plugin/semantic-engine.yml, api/operations.json, docs/api.md   # 1.3.0
specs/005-stash-adapter/contracts/reader-interface.md, specs/006-graph-store/contracts/graph-store-interface.md
ROADMAP.md                # tick the two Phase 2 items
```

**Structure Decision**: the update lives beside the build in `engine::build`, sharing its
mapping and batching. The store and reader changes are additive, in their own crates' modules.
The plugin gains one handler in its existing `build.rs`.

## Complexity Tracking

No constitution violations to justify.
