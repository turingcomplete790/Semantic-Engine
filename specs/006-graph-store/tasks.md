# Tasks: Graph Store, Oxigraph Embedded

**Input**: Design documents from `/specs/006-graph-store/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R9), data-model.md,
contracts/graph-store-interface.md, quickstart.md

**Tests**: included. Constitution Principle VII requires tests first, run without a live Stash.
Each test task is written to fail before the code that makes it pass. Every test uses a fresh
temporary store folder and a fixed server id.

**Organization**: tasks are grouped by user story:
- **US1 (P1):** write facts and publish them for readers;
- **US2 (P1):** read and query a published generation;
- **US3 (P2):** a store that repairs itself;
- **US4 (P3):** report what the store holds.

**Standing rules for every task:**
- **No Oxigraph in the interface:** no `oxigraph` type appears in a `pub` item of
  `engine::graph` (rule G9).
- **Readers:** readers never open `work/` (G1).
- **Nothing outbound:** no host is contacted, and no credential is written.
- **The spike as reference:** `crates/spike/src/store.rs` is the reference implementation. Copy
  its proven pieces (lock, checkpoint publish, generation listing, cleanup), but never depend on
  the spike crate.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US4

---

## Phase 1: Setup

- [X] T001 Create the module skeleton:
  - `crates/engine/src/graph/mod.rs`, declaring `term`, `layout`, `health`, `writer`, `reader`
    and `error` (empty files of those names);
  - `pub mod graph;` in `crates/engine/src/lib.rs`;
  - a stub `crates/engine/src/bin/graph-bench.rs` that prints usage.

  `cargo build --workspace` passes.
- [X] T002 [P] Write the shared test helpers in `crates/engine/tests/common/graph.rs`, declared
  from `crates/engine/tests/common/mod.rs`:
  - `temp_store(name) -> (PathBuf, GraphStore)`, which makes a fresh folder under
    `std::env::temp_dir()` and uses the server id `00000000-0000-4000-8000-000000000006`;
  - `iri(s)`, `lit(s)`, `typed(s, dt)` and `fact(s, p, o)` builders;
  - `sample_facts(n)`: n scenes, each with 2 performers and a title literal.

---

## Phase 2: Foundational (blocks every story)

- [X] T003 [P] Write failing tests in `crates/engine/tests/graph_terms.rs` for the data-model
  rules of Term and Fact:
  - **IRIs:** "an absolute IRI"; a relative or invalid one gives `GraphError::InvalidFact`.
  - **Blank nodes:** the label must match "`[A-Za-z0-9_-]+`".
  - **Literals:** "at most one of `datatype` and `language`"; both gives `InvalidFact`.
  - **Subject and predicate:** the subject is "`Iri` or `Blank` only", and the predicate "`Iri`
    only". Anything else gives `InvalidFact`.
  - **Round trip:** converting to the store's form and back keeps every field. Include non-ASCII
    text, a 100 KB literal, a language-tagged literal and a typed literal.
- [X] T004 Implement `Term`, `Fact`, `Pattern` and `Solutions` in
  `crates/engine/src/graph/term.rs`, with private conversion to and from Oxigraph's types. All
  conversion goes through this file. T003 then passes.
- [X] T005 [P] Implement `crates/engine/src/graph/error.rs`:
  - **`GraphError`:** the kinds `Busy`, `NotBuilt`,
    `NeedsRebuild(RebuildReason)`, `InvalidQuery { message, line: Option<u32>, column:
    Option<u32> }`, `InvalidFact { message }`, `NoServerId`, `Io { message }` and
    `Internal { message }`. Each has a `kind()` name.
  - **`RebuildReason`:** `Missing`, `Damaged { detail }`,
    `OtherFormat { found, expected }` and `OtherServer`.
  - **`Health`:** `Ready { generation }`, `NotBuilt` and `NeedsRebuild(RebuildReason)`.
  - **Formatting:** `Display` texts that never include the server ids.
- [X] T006 Implement `crates/engine/src/graph/layout.rs` (research R2):
  - **Paths:** `store.json`, `work/`, `work.dirty`, `snapshots/<n>/`, `snapshots/<n>.json`,
    `current` and `writer.lock`.
  - **The stamp:** read and write `{"format", "serverId", "createdAt"}`.
  - **`current`:** read it, and write it atomically with `current.tmp` and a rename.
  - **Generation files:** read and write `{"generation", "publishedAt", "facts", "format",
    "serverId"}`.
  - **Generations on disk:** list the generation numbers present.
  - **Times:** a UTC RFC 3339 helper.

  Constants in `mod.rs`: `STORE_FORMAT = 1`, `GENERATION_GRACE = 15 minutes` and
  `STORE_DIR = "graph"`.
- [X] T007 Implement `check()` in `crates/engine/src/graph/health.rs`, in the order of research
  R4, steps 1 to 7:
  1. `store.json` missing gives Missing, or NotBuilt for an empty, stamped-never folder.
  2. Unreadable, or not valid JSON, gives Damaged.
  3. A format other than 1 gives OtherFormat.
  4. Another `serverId` gives OtherServer.
  5. `current` missing gives NotBuilt.
  6. `current` unreadable, or naming a missing generation folder, gives Damaged.
  7. If the read-only open fails, it gives Damaged, keeping the library's message.
- [X] T008 Implement `GraphStore::new` and `GraphStore::clear_for_rebuild` in
  `crates/engine/src/graph/mod.rs`:
  - **`new`:** an empty server id makes every call give `NoServerId`.
  - **`clear_for_rebuild`:** it takes the writer lock (giving `Busy` if held). It removes
    `work/`, `work.dirty`, `current` and `snapshots/`, then writes a fresh stamp. The result is
    `NotBuilt` and can be written.
- [X] T009 Implement `GraphStore::reader` and `Reader::generation` in
  `crates/engine/src/graph/reader.rs`:
  - **Opening:** it runs `check()`, then opens `snapshots/<current>/` read-only, together with
    its `Generation`.
  - **Errors:** `NotBuilt` or `NeedsRebuild` when the check says so.
  - **For tests:** add a crate-internal `contains(&Fact) -> bool`.

**Checkpoint**: terms, errors, layout and health compile and are tested. A stamped, empty store
can be created. Nothing is published yet.

---

## Phase 3: User Story 1, write facts and publish them for readers (Priority: P1) 🎯 MVP

**Goal**: a writer with the lock changes the working store and publishes numbered generations;
readers see only published state (FR-004 to FR-009).

**Independent Test**: `cargo test -p engine --test graph_write_publish` passes.

### Tests for User Story 1 ⚠️

- [X] T010 [P] [US1] Write failing tests in `crates/engine/tests/graph_write_publish.rs`:
  - **First publish:** on an empty, cleared store, add 10 facts and publish. The `Generation` is
    number 1, with `facts == 10`. A new reader has generation 1 and contains every fact.
  - **Unpublished changes are invisible:** add more without publishing, and a new reader still
    has generation 1 without them (acceptance scenario US1-2).
  - **Busy writer:** while a `Writer` is held, `writer()` gives `Busy`, measured under 50 ms
    (SC-004).
  - **Crash before publish:** drop a writer without publishing, after adding facts. Readers still
    see generation 1. The next `writer()` starts from generation 1's facts: the dropped facts are
    gone, and `work.dirty` was handled. Also run this with a child process that adds facts and
    calls `std::process::abort()` (re-run the test binary with an environment variable, as in
    research R9). The lock must be free afterwards (FR-009).
  - **Numbering:** publish 3 more times, with one dropped writer in between. The numbers are 2,
    3 and 4, with no gaps or repeats (FR-007).
  - **All or nothing:** remove 5 facts and add 5 in one change, then publish. A reader sees all
    10 changes, never a mixture (US1-5).
  - **Pattern removal:** `remove_matching` with only the subject removes and counts that
    subject's facts. `clear` removes every fact.
  - **No-ops:** adding a fact that is present, or removing one that is absent, has no effect.

### Implementation for User Story 1

- [X] T011 [US1] Implement `Writer` in `crates/engine/src/graph/writer.rs` (research R3):
  - **Lock:** take `writer.lock` with `File::try_lock`. `WouldBlock` gives `Busy`.
  - **Recovery:** if `work.dirty` exists, replace `work/` with a copy of the current generation.
    Hard-link `.sst` files and copy the rest, as the spike's `copy_generation` does. With no
    current generation, empty `work/`.
  - **The marker:** create `work.dirty` before the first change.
  - **`add`:** Oxigraph's bulk loader when `work/` is empty, otherwise `extend`.
  - **`remove`, `remove_matching`, `clear`:** removal uses `quads_for_pattern` followed by
    `remove`.
  - **`publish(self)`:** flush; `backup` into `snapshots/<n>/`; count with `len`; write
    `snapshots/<n>.json`; switch `current` atomically; remove `work.dirty`.

  File-system errors give `Io`, with `current` untouched (the edge case "disk full").
- [X] T012 [US1] Implement `GraphStore::writer` in `crates/engine/src/graph/mod.rs`. It runs
  `check()` first and refuses `NeedsRebuild` (rule G6), then opens the `Writer`. T010 then
  passes.

**Checkpoint**: graphs can be written and published safely, which is the MVP.

---

## Phase 4: User Story 2, read and query a published generation (Priority: P1)

**Goal**: SPARQL `SELECT` over the generation a reader opened, which stays fixed for the call,
with structured errors (FR-010 to FR-013, SC-001).

**Independent Test**: `cargo test -p engine --test graph_read --test graph_concurrency --test
graph_interface` passes.

### Tests for User Story 2 ⚠️

- [X] T013 [P] [US2] Write failing tests in `crates/engine/tests/graph_read.rs`:
  - **Answers:** a `SELECT` with `ORDER BY` returns rows in that order. `variables` are in
    projection order, without `?`. An `OPTIONAL` variable that is not bound is `None`. Literals
    come back with their datatype and language.
  - **Fixed generation:** a reader with generation 5 open keeps answering from 5 after 6 is
    published, and a new reader gets 6 (US2-2).
  - **Syntax errors:** a broken query gives `InvalidQuery` with `line: Some(1)` and the correct
    column, for example `SELECT ?s WHERE { ?s ?p }`.
  - **Other forms:** `ASK`, `CONSTRUCT`, `DESCRIBE` and an update (`INSERT DATA`) each give
    `InvalidQuery`, with a message naming `SELECT`.
  - **Not built vs empty:** a stamped store with nothing published gives `NotBuilt`. A store
    published with 0 facts gives an empty `Solutions`, not an error (FR-013).
- [X] T014 [P] [US2] Write the multi-process test in `crates/engine/tests/graph_concurrency.rs`
  (research R9, SC-001):
  - **Readers:** 4 reader processes, each re-running the test binary in reader mode through an
    environment variable, read in a loop.
  - **Writer:** meanwhile, publish 50 generations. Each replaces 200 facts
    `<urn:t:block/i> <urn:t:gen> "n"` with the generation number n.
  - **Each read:** `SELECT` all of them, and assert every value equals the reader's own
    `generation().number`.
  - **Pass:** at least 5,000 reads in total, with 0 mismatches and 0 errors.
- [X] T015 [P] [US2] Write `crates/engine/tests/graph_interface.rs` (rule G9). It reads every
  file in `crates/engine/src/graph/` and asserts that no line declaring a public item mentions
  `oxigraph`. A public item is a line starting `pub ` after trimming, excluding `pub(crate)` and
  `pub(super)`.

### Implementation for User Story 2

- [X] T016 [US2] Implement `Reader::select` in `crates/engine/src/graph/reader.rs` (research R7):
  - **Parsing:** parse the query and reject every form other than `SELECT` with `InvalidQuery`.
  - **Running:** run it on the open generation, and convert the solutions through `term.rs`.
  - **Error position:** read `line` and `column` from the parser's `error at {line}:{column}:`
    text. If the text has another form, they are `None`, and the message is kept.

  T013 to T015 then pass.

**Checkpoint**: the engine can answer SELECT queries from a published generation, and SC-001
holds.

---

## Phase 5: User Story 3, a store that repairs itself (Priority: P2)

**Goal**: the four rebuild reasons, clearing, and cleanup of old generations (FR-014 to FR-017,
SC-006, SC-007).

**Independent Test**: `cargo test -p engine --test graph_health` passes.

### Tests for User Story 3 ⚠️

- [X] T017 [P] [US3] Write failing tests in `crates/engine/tests/graph_health.rs`:
  - **Missing:** a missing folder, and a folder without `store.json`.
  - **Damaged:** each of these:
    - a truncated `store.json`;
    - `current` holding `abc`;
    - `current` naming a deleted generation folder;
    - a generation folder whose RocksDB `CURRENT` file was deleted.
  - **Other format:** `format` 0 and `format` 2. Each gives `OtherFormat { found, expected: 1 }`.
  - **Other server:** another server id gives `OtherServer`.
  - **Refused:** for every case, `reader()`, `writer()` and `summary()` refuse with
    `NeedsRebuild`, giving that reason (SC-006).
  - **Clearing:** `clear_for_rebuild` turns each case into `NotBuilt`, ready for a write, and the
    next publish is generation 1.
  - **Cleanup by age:** publish 6 generations, setting the folder times of generations 1 to 4 to
    an hour ago. After the next publish, only generation 5, generation 6 (the previous one), and
    the new generation 7 remain, with their `.json` files.
  - **Cleanup by grace:** repeat with recent times. Every generation younger than
    `GENERATION_GRACE` is kept.
  - **A backlog:** with 20 old generations left over, one publish removes all of them except the
    current, the previous, and the young ones.

### Implementation for User Story 3

- [X] T018 [US3] Implement cleanup in `crates/engine/src/graph/writer.rs` (research R5). It
  runs at the end of `publish`, after `current` switches:
  - **What it keeps:** the current generation, the previous one, and those younger than
    `GENERATION_GRACE`, by the folder's modified time.
  - **What it removes:** each other generation's folder and its `.json`.
  - **A removal that fails:** it is skipped and logged in the returned `Generation`'s notes,
    never failing the publish.
- [X] T019 [US3] Make every entry point run `check()` and refuse rebuild states (rule G6), in
  `crates/engine/src/graph/mod.rs` and `crates/engine/src/graph/health.rs`:
  - **Entry points:** `reader()`, `writer()` and `summary()`.
  - **Damaged:** carries the library's message as `detail`.
  - **Messages:** never include the server ids.

  T017 then passes.

**Checkpoint**: a copied, stale or broken store is never read, and the disk does not grow
without bound.

---

## Phase 6: User Story 4, report what the store holds (Priority: P3)

**Goal**: the summary of FR-018.

**Independent Test**: `cargo test -p engine --test graph_summary` passes.

### Tests for User Story 4 ⚠️

- [X] T020 [P] [US4] Write failing tests in `crates/engine/tests/graph_summary.rs`:
  - **Published store:** after publishing 3 generations of known sizes, `summary()` reports:
    - `health: Ready`;
    - the current `Generation`, with its number, `published_at` and `facts`;
    - `format: 1`, and the `server_id`;
    - `disk_bytes` greater than 0, and equal to the folder's size;
    - `writing: false`.
  - **During a write:** while a `Writer` is held, `writing` is `true`, and `summary()` returns
    in under 50 ms.
  - **Not built:** a `NotBuilt` store reports its health, with no generation.

### Implementation for User Story 4

- [X] T021 [US4] Implement `GraphStore::summary` in `crates/engine/src/graph/mod.rs` (research
  R6):
  - **Sources:** the stamp, `current` and that generation's `.json`.
  - **Disk:** the size of the whole store folder.
  - **Writing:** try the lock without waiting, and release it at once if the try succeeds.
  - **No counting:** it never counts the facts. T020 then passes.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T022 Implement `crates/engine/src/bin/graph-bench.rs` (research R8):
  - **`build --scenes N --dir D`:** synthetic facts in the spike's shape: 2 performers per
    scene; a role, an action (with one variation) and a pluggable instance per performer; and a
    timeline entry. They are written in batches through `GraphStore` and published once.
  - **`open --runs N`:** times `reader()`.
  - **`query --runs N`:** times a whole call, `reader()` plus a fixed set of `SELECT` queries,
    including "scenes where performer A has role R and performer B has action X" over the
    synthetic vocabulary.
  - **`disk`:** the size per scene, and the size of the whole store after cleanup.
  - **`stress --readers R --seconds S`:** the T014 check at scale.

  Results are written as JSON to `specs/006-graph-store/results/graph-{command}-{scenes}.json`.
  No credential and no host are involved.
- [X] T023 Run quickstart step 2 at 1,000 and at 250,000 scenes on Linux. Record the figures
  against SC-001, SC-002, SC-003 and SC-007, with the build time for reference, in a new
  "Results" section of `specs/006-graph-store/quickstart.md`. If a budget is missed, stop and
  report it before tuning anything.
- [X] T024 [P] Extend the "Release package has no spike files" step in
  `.github/workflows/ci.yml` so it also rejects `graph-bench`. Run quickstart step 4 locally.
- [X] T025 [P] Update the documentation:
  - **`ROADMAP.md`:** tick the Phase 2 items "Oxigraph embedded behind an engine-owned
    interface" and "Store with published snapshot generations", linking
    `specs/006-graph-store/`.
  - **Module docs:** describe the layout and rules G1 to G9 in
    `crates/engine/src/graph/mod.rs`.
- [X] T026 Run the full gate:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `cargo test --workspace`, three times, to catch flakes in the concurrency test;
  - the vocabulary and script checks CI runs.

  Then update the graphify graph.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1):** none. T002 can run in parallel with T001's follow-ups.
- **Foundational (Phase 2):**
  - T003 and T005 can run in parallel.
  - T004 needs T003. T006 needs T005.
  - T007 needs T006. T008 needs T006 and T007. T009 needs T004, T007 and T008.
- **US1 (Phase 3):** needs Foundational.
- **US2 (Phase 4):** needs Foundational. Its tests need US1's writer to publish test data, so it
  is done after US1.
- **US3 (Phase 5):** needs US1, because cleanup is part of `publish`. It can run alongside US2.
- **US4 (Phase 6):** needs US1. It can run alongside US2 and US3.
- **Polish (Phase 7):** needs US1 to US4.

### Within Each User Story

- Tests are written first and fail.
- Then the implementation makes them pass. A story is complete when its independent test passes.

### Parallel Opportunities

- T003 and T005 in Foundational.
- T013, T014 and T015, which are separate test files.
- T017 and T020, test files of different stories, once US1 is done.
- T024 and T025 in Polish.

---

## Parallel Example: User Story 2

```text
Task: "T013 [US2] graph_read.rs"
Task: "T014 [US2] graph_concurrency.rs"
Task: "T015 [US2] graph_interface.rs"
```

Then T016 makes all three pass.

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup and Foundational: terms, errors, layout, health, and creating a stamped store.
2. US1: write and publish, crash-safe.
3. **Stop and validate:** `graph_write_publish` passes. The next feature's builder could already
   write a graph.

### Incremental Delivery

1. US2: queries, the multi-process guarantee (SC-001), and the interface check.
2. US3: rebuild reasons and cleanup.
3. US4: the summary.
4. Polish: the bench, the measurements at 250,000 scenes, CI, the documentation and the full
   gate.

### Notes

- Commit only when the owner asks. No push or release without approval.
- No test or bench touches Stash. The store is a local folder.
