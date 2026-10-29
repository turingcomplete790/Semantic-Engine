# Implementation Plan: Query Operations

**Branch**: `master` (spec directory `009-query-operations`) | **Date**: 2026-10-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/009-query-operations/spec.md`

## Summary

Clients query the semantic graph through a new `query` operation. They write the documents'
SPARQL-inspired `SELECT` language, level 1.

**How a query runs:**
1. The engine parses it with its own small parser, so every error carries a position (R1).
2. It resolves names: vocabulary terms, tags by name, and items by id (R2).
3. It rewrites the documents' short form so scene-scoped facts join only within their scene
   (R3), and expands recursive Role and Relation criteria through spec 004's hierarchy (R5).
4. It prints SPARQL for the existing read-only reader.

**Answers** are distinct, ordered and typed (R4). Pages are stateless: each continuation carries
its generation and offset, with a check value (R6). A 10 s limit ends runaway queries (R7).

**Freshness:** the save hook is widened to tags, performers, scenes and images. It records a
pending marker and starts "Update semantic graph" once per burst (R8). `status` reports the
graph from the generation file alone (R9).

The contract moves to 1.4.0.

## Technical Context

**Language/Version**: Rust stable, the existing Cargo workspace (edition per workspace).

**Primary Dependencies**:
- Oxigraph 0.4 (existing), behind `engine::graph`.
- `semantic-vocabulary` (existing), for term checks.
- `stash-adapter` (existing), for `run_plugin_task` from the hook.
- No new crate: the continuation uses hex and FNV-1a, as spec 008's stamp does. The plugin
  names `libc` directly, for `_exit` (research R7); it was already in the tree, through `cc`.

**Storage**: the spec 006 store (format 2, unchanged) and its kept generations. The plugin folder
gains `graph.pending`, `graph.applying` and `query.key`.

**Testing**: `cargo test` with the compatibility fixture (spec 007), new scope and paging
fixtures, the synthetic library, and the plugin's envelope tests. The test instance is used for
hooks, freshness and equivalence only.

**Target Platform**: the Stash plugin on Linux (primary), Windows and macOS, as before.

**Project Type**: a Rust library crate (`engine`) and the thin plugin executable (`plugin`).

**Performance Goals**:
- Query p95 under 200 ms end to end at 250,000 scenes, plain and recursive (rule 12; 004
  SC-007).
- A 1 MiB page under 200 ms.
- The hook adds at most 7 ms per saved item (rule 11).
- A save shows in answers within 30 s.

**Constraints**:
- Pages of at most 1 MiB; no response over 2 MiB (rule 9).
- Queries read only a published generation (rules 1–3).
- No client gets the graph (rule 10).
- `query` never contacts Stash.

**Scale/Scope**:
- Production: 49,683 scenes, 29,057 images, 1,220 semantic records.
- Budgets at 250,000 scenes and 325,000 images (24.2 million facts in spec 008's synthetic
  store).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. Stash is the system of record | `query` and `status` only read the engine's store. The hook writes only marker files. The update task stays read-only toward Stash (spec 008). The new files are disposable. | pass |
| II. The documents are the contract | The language is the documents' Phase 1, plus comparisons (§ Query Features). The short form's scope rule is the Vocabulary corrections' "evaluator must enforce through how it joins". Sections and commit `3aab0be` are cited in the spec. | pass |
| III. One engine, one set of answers | One query implementation. Recursion reuses spec 004's `Hierarchy`, so `relations`, auto-apply and queries agree on "narrower" (R5). No client logic. No graph handed out (rule 10). | pass |
| IV. Reached only through Stash | `runPluginOperation` and hooks only. Readers open published generations read-only, including older kept ones for paging (G1, G2). Hooks record and start a job (rules 4, 5, 6 as superseded by 004 R6). Hook cost stated and measured (SC-006). Ending the process on the time limit leaves no lock: readers hold none. | pass |
| V. Rust, embedded, headless | **Deviation, justified below (R12):** level-1 queries are evaluated by the engine on its own query index, derived from each Oxigraph generation, not by Oxigraph's SPARQL evaluator. Oxigraph stays the store. A second `unsafe` call, the index's memory map (`memmap2::Mmap::map`), isolated in `engine::qindex::QIndex::open`. All logic is in `engine::query`. The plugin adds the operation shell, the timer thread and the hook dispatch. No Oxigraph type leaves `engine::graph` (G9): the query module prints SPARQL text for `Reader::select`. One `unsafe` FFI call, `libc::_exit`, isolated in `plugin::query::stop_at_time_limit` and covered by `ops_query`'s time-limit test (research R7, added in implementation). | pass, justified below |
| VI. Versioned, explicit contracts | Contract 1.4.0, additive. Language level 1 documented. Every error is structured with a position. Paging and limits documented. `contract` argument and `CONTRACT_MISMATCH` (R10). | pass |
| VII. Fast, bounded, test-first | Budgets stated with repeatable commands (R11). Paging bounds every answer. The time limit bounds evaluation. Tests come first per story, against fixtures, with no live Stash. | pass |

**Post-design re-check:** pass. The design adds:
- **two reader entry points** (`reader_at`, `describe`), both read-only;
- **a stamp field** (`items`), additive and ignored by older code;
- **one widened hook.**

None of these changes the store format, or a principle's meaning.

## Project Structure

### Documentation (this feature)

```text
specs/009-query-operations/
├── spec.md
├── plan.md                    # this file
├── research.md                # R1–R11
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── query-language.md      # language level 1
│   └── operations-1.4.md      # query, status additions, the hook
├── checklists/requirements.md
├── results/                   # measurements, filled by implementation
└── tasks.md                   # /speckit-tasks
```

### Source Code (repository root)

```text
crates/engine/src/
├── query/                     # new
│   ├── mod.rs                 # run(): parse → resolve → rewrite → print → page
│   ├── lex.rs                 # tokens with positions; SPARQL keywords for refusals
│   ├── parse.rs               # Ast
│   ├── names.rs               # built-in prefixes, vocabulary checks, tag and item lookups
│   ├── rewrite.rs             # short form and scope, recursive VALUES, hidden variables
│   ├── print.rs               # SPARQL text with DISTINCT and VALUES; the engine orders
│   ├── page.rs                # Page, Value, Continuation, PAGE_BYTES
│   └── error.rs               # QueryError and codes
├── graph/
│   ├── mod.rs                 # GraphStore::reader_at, GraphStore::describe
│   └── layout.rs              # opening a kept generation by number
├── relations/hierarchy.rs     # Hierarchy::from_links
├── build/run.rs, update.rs, stamp.rs   # stamp `items`
├── errors.rs                  # new error codes
└── versions.rs                # CONTRACT_VERSION 1.4.0

crates/plugin/src/
├── query.rs                   # new: the `query` operation, the time limit, CONTRACT_MISMATCH
├── ops.rs                     # registers `query`; `status` gains `graph`
├── relations.rs               # record_change dispatches by hook type
├── journal.rs                 # graph markers: pending, applying, restore-as-stale
├── build.rs                   # update task takes and clears the graph marker
└── bin/query-bench.rs         # new: run (250k budget) and fresh (test instance)

crates/engine/tests/
├── query_parse.rs, query_names.rs, query_fixture.rs, query_scope.rs,
├── query_recursive.rs, query_paging.rs, hierarchy_links.rs
└── fixtures/query/            # known-answer queries and the scope fixture

crates/plugin/tests/
├── ops_query.rs               # new
└── ops_status.rs, ops_record_change.rs, journal.rs, api_agreement.rs   # extended

plugin/semantic-engine.yml     # the hook entry (contracts/operations-1.4.md)
api/operations.json, docs/api.md
ROADMAP.md                     # Phase 3 items ticked when done
```

**Structure Decision:**
- **The query semantics are domain logic,** so they go in the `engine` library, next to
  `graph`, `build` and `relations` (Principle V).
- **The plugin crate only adapts envelopes,** owns the process-level time limit, and dispatches
  hooks.
- **`query-bench` lives in the plugin crate,** because it drives the plugin binary end to end.

## Delivery order

1. **US1 and US2 together** (P1): parse, names, rewrite, print, single-page answers, errors, and
   the `query` operation. Scope is part of the rewrite, so it ships with the first query.
2. **US3** (P1): the widened hook, the markers, and the update task's use of them. This is
   independent of queries, and can go in parallel with step 1.
3. **US4** (P2): `from_links` and recursive constants.
4. **US5** (P2): continuations, `reader_at`, the byte bound, `PAGE_EXPIRED` and `PAGE_INVALID`.
5. **US6** (P3): `describe`, stamp `items`, and the `status` additions.
6. **Polish:**
   - the 250k budget, the hook cost, and the freshness measurement;
   - test-instance equivalence;
   - contract documents and the roadmap.

## Complexity Tracking

One deviation from Principle V (research R12): Oxigraph remains the graph store, but level-1
queries are evaluated on an engine-owned index. The constitution names Oxigraph for "the graph
and SPARQL evaluation"; an amendment is the owner's decision.

| Choice | Why | Simpler alternative rejected because |
|---|---|---|
| Own parser instead of Oxigraph's | positions for every error kind; precise refusals (R1) | the SPARQL algebra has no positions, and pass-through makes all of SPARQL contract |
| Ending the process on timeout | Oxigraph 0.4 cannot cancel a query (R7) | polling between solutions cannot stop a long join |
| `libc::_exit` (one `unsafe` call) to end it | `std::process::exit` runs RocksDB's C++ static destructors while the query thread is inside RocksDB, and the process crashes with exit 139 after its output (found in T018). `libc` was already compiled in the tree, through `cc` | `std::process::abort` ends with a signal, which Stash reports as a failed plugin; leaving the thread running keeps the call waiting past the limit |
| An engine-owned query index, and its own evaluator (R12) | at 250k, Oxigraph costs 40–640 µs per lookup and plans the short form's joins badly: Role queries ran past 10 s, a two-performer join took 142 s for its first 8,000 bindings | compaction at publish gave 2–3×; engine-side join order over Oxigraph's lookups keeps the 40 µs lookup |
| A memory map (`memmap2`, one `unsafe` call) | the index is read in place; reading 1.4 GB per call, or a system call per binary-search probe, costs more than the budget | `pread` per probe costs about 1 µs, 25 per lookup |

