# Tasks: Query Operations

**Input**: Design documents from `/specs/009-query-operations/`

**Prerequisites**:
- plan.md and spec.md;
- research.md (R1–R11) and data-model.md;
- the contracts: query-language.md, operations-1.4.md;
- quickstart.md.

**Tests**: included. Constitution Principle VII requires tests first, run without Stash. Each
test task is written to fail before the code that makes it pass. Engine tests build their graph
from the spec 007 compatibility fixture (`tests/common/build.rs`: `fixture_library`, `SERVER`)
into fresh temporary store folders.

**Organization**: tasks are grouped by user story:
- **US1 (P1):** ask the graph a question;
- **US2 (P1):** scene-scoped facts only join within one scene;
- **US3 (P1):** the graph follows saves on its own;
- **US4 (P2):** recursive Role and Relation criteria;
- **US5 (P2):** large answers come in pages;
- **US6 (P3):** ask the engine how it is.

**Standing rules for every task:**
- **Navigate with graphify first:** run `graphify query`, `explain` or `path` before broad
  searches. Use `grep` for exact text only. `crates/engine/src/build/` is not in the graph's
  file detection; read it directly.
- **No Oxigraph outside `engine::graph`** (G9). `engine::query` builds SPARQL text and calls
  `Reader::select`.
- **`query` and `status` never write to Stash, and `query` never contacts it.** No credential or
  field value in any log.
- **Positions:** every query error that has a place in the text carries 1-based `line` and
  `column` (FR-027).
- **The firewall:** a fresh test binary may first fail as "unreachable". Rerun before
  debugging.
- **Writes only to the test instance** at `localhost:9998`. Credentials go into environment
  variables from `../Test-Instance/creds.txt` and are never printed.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US6

---

## Phase 1: Setup

- [X] T001 Create the module `crates/engine/src/query/` with empty `mod.rs`, `lex.rs`,
  `parse.rs`, `names.rs`, `rewrite.rs`, `print.rs`, `page.rs` and `error.rs`, and declare
  `pub mod query;` in `crates/engine/src/lib.rs`. Create `crates/engine/tests/common/query.rs`
  (declared in `tests/common/mod.rs`) and the folder `crates/engine/tests/fixtures/query/`.
  Create `crates/plugin/src/query.rs` (declared in `crates/plugin/src/lib.rs`), and add
  `[[bin]] name = "query-bench" path = "src/bin/query-bench.rs"` to `crates/plugin/Cargo.toml`
  with a `main` that prints usage.

---

## Phase 2: Foundational (blocks every story)

- [X] T002 Add the new error codes to `ErrorCode` and `ErrorCode::ALL` in
  `crates/engine/src/errors.rs`: `QUERY_SYNTAX`, `QUERY_UNSUPPORTED`, `QUERY_UNKNOWN`,
  `QUERY_SCOPE_AMBIGUOUS`, `QUERY_NOT_RECURSIVE`, `QUERY_TIMEOUT`, `RESULT_TOO_LARGE`,
  `PAGE_EXPIRED`, `PAGE_INVALID`, `GRAPH_NOT_BUILT`, `GRAPH_NEEDS_REBUILD` and
  `CONTRACT_MISMATCH`, each with a doc line "(since 1.4.0)". Set `CONTRACT_VERSION` to `"1.4.0"`
  in `crates/engine/src/versions.rs`. Update the version pin in `crates/engine/tests/errors.rs`.
- [X] T003 [P] Define `QueryError` in `crates/engine/src/query/error.rs`, per data-model
  "QueryError":
  - kinds `Syntax`, `Unsupported { construct }`, `Unknown { name, why }`,
    `ScopeAmbiguous { performer }`, `NotRecursive { name }`, `TooLarge`,
    `PageExpired { generation }`, `PageInvalid`, `NotBuilt`, `NeedsRebuild { reason }`,
    `Internal`;
  - each with a message and an optional `Pos { line, column }`;
  - `code()` maps a kind to its `ErrorCode`, and `details()` gives the JSON `details` of
    contracts/operations-1.4.md;
  - `From<GraphError>`: a store needing a rebuild becomes `NeedsRebuild` with reason `damaged`,
    `otherFormat` or `otherServer`; no generation becomes `NotBuilt`; the rest becomes `Internal`.
- [X] T004 [P] Write the shared test helpers in `crates/engine/tests/common/query.rs`:
  - `fixture_store(name)`: builds the compatibility fixture into a fresh store and returns it;
  - `answer(store, text)`: runs `engine::query::run` with no page options, and returns the
    variables and rows as sorted `Vec<BTreeMap<String, String>>`, values in N-Triples form;
  - `expect_error(store, text, code, line, column)`.

**Checkpoint:** the workspace builds, and the existing tests pass with the contract at 1.4.0 (the
agreement test fails until T043 updates the documents; mark it expected).

---

## Phase 3: User Story 1 + User Story 2 — Ask a question, scoped by scene (P1) 🎯 MVP

**Goal:** a client sends a level-1 `SELECT` and gets typed, distinct, ordered bindings from the
current generation, with structured, positioned errors. Short-form scene-scoped facts join only
within their scene. US2 ships with US1 because the scope rule is part of the rewrite every
short-form query passes through (plan, "Delivery order").

**Independent Test:** on the fixture graph, the known-answer queries return exactly their
expected bindings (SC-001). On the scope fixture, short form equals graph form, and no Role is
matched from another scene (SC-002).

### Tests for US1 and US2 (write first; they must fail)

- [X] T005 [P] [US1] Write `crates/engine/tests/query_parse.rs` against
  `engine::query::parse::parse` (contracts/query-language.md, Grammar):
  - **accepted:** `PREFIX` lines; `SELECT`, `SELECT DISTINCT` and `SELECT *`; optional `WHERE`;
    `;` and `,` lists; `a`; IRIs, prefixed names with percent encoding; string literals with
    `@lang` and `^^type`; integers, decimals, doubles, booleans; `#` comments; several `FILTER`s
    with each of `= != < <= > >=`; keywords in any case;
  - **positions:** every node carries its 1-based line and column, across lines;
  - **refused, `Unsupported` with the construct and its position:** `ASK`, `CONSTRUCT`,
    `DESCRIBE`, `INSERT`, `BASE`, `OPTIONAL`, `UNION`, `MINUS`, `GRAPH`, `SERVICE`, `BIND`,
    `VALUES`, a nested `{ }`, a sub-`SELECT`, each property-path operator, `_:b`, `[ ]`, `( )`,
    `&&`, `||`, `!`, arithmetic, a function call in `FILTER`, `GROUP BY`, `HAVING`, `ORDER BY`,
    `LIMIT`, `OFFSET`, `COUNT(…)`, `FROM`;
  - **`Syntax`:** unbalanced braces, a missing `.` between patterns, an unterminated string, a
    projected variable in no pattern, a variable starting `__se`, a redefined built-in prefix
    (`sem:`, `tag:`, `recursive:` and the rest).
- [X] T006 [P] [US1] Write `crates/engine/tests/query_names.rs` against `engine::query::run` on
  the fixture store (research R2):
  - **built-in prefixes** resolve: `sem:` and `:` to vocabulary terms; `tag:`, `performer:`,
    `scene:` and `image:` with an id to item IRIs carrying the fixture's server id;
  - **a tag by name:** `:Name` and `:Full%20Service` find the one tag with that exact
    `rdfs:label`;
  - **refused with `Unknown` at the name's position:**
    - a name matching no tag;
    - a name matching two tags (build a fixture variant with two tags of one label);
    - an unknown prefix;
    - `sem:hasTag` (Planned) and `sem:inStage` (Derived), with the status in the message;
    - `scene:999999`;
    - a vocabulary IRI that is not a term.
  - **A term beats a tag:** a tag named like a term is reached as `tag:{id}`.
- [X] T007 [P] [US1] Write `crates/engine/tests/fixtures/query/known.json`: at least 30
  `{ "name", "query", "expected": [ { var: N-Triples value } ] }` entries over the compatibility
  fixture (SC-001). The expected bindings are derived by hand from
  `tests/fixtures/build/expected.nt`, and each entry names the facts it relies on in a
  `"because"` field. Together they cover:
  - every Active property the fixture holds, by name;
  - joins of two and three performers in one scene;
  - each comparison operator, on times (`atTime`), positions and strings;
  - `?a != ?b`;
  - `SELECT *`;
  - images as well as scenes;
  - Traits, Labels, categories, parent tags, relations (graph form) and pairings.
- [X] T008 [US1] Write `crates/engine/tests/query_fixture.rs`. It runs every entry of
  `known.json` through `common::query::answer` and compares the bindings exactly, naming the
  entry on failure. Add checks that:
  - **values are typed:** an item value has `kind` and `id`, an instance IRI is a `node`, a
    vocabulary IRI is a `term` with its `name`, and a literal has its `datatype`;
  - **answers are distinct:** a query whose hidden variables would repeat a binding returns it
    once;
  - **answers are deterministic:** two runs give the same order;
  - **the generation is reported:** `Page.generation` is the store's current one;
  - **no graph:** an empty store gives `NotBuilt`, and a store with another format gives
    `NeedsRebuild` with `otherFormat`.
- [X] T009 [P] [US2] Write `crates/engine/tests/fixtures/query/scope.json`, the scope library in
  `LibraryRead` form (as `tests/fixtures/build/library.json`):
  - performer A is Dom in scene 1 and Sub in scene 2;
  - performer B is Sub in scene 1 and Dom in scene 3;
  - performer C is in scene 2 with no Role;
  - A carries a Trait;
  - scene 3 has a Mise-en-scène pick;
  - image 1 repeats scene 1's Roles.
- [X] T010 [US2] Write `crates/engine/tests/query_scope.rs` on a store built from `scope.json`
  (research R3; SC-002):
  - "scenes with a performer holding Dom" gives scenes 1 and 3, and image 1;
  - the documents' "multiple bound performers" query gives only (scene 1, A, B) and the
    image's;
  - Role plus Trait on one performer: the Role must be in the bound scene, and the Trait holds
    in any scene;
  - `?m :hasMiseEnScene X` matches scene 3 only;
  - `?p :hasRole :Dom` with no scene pattern gives A and B;
  - **short form equals graph form:** for every short-form query of this file, the hand-written
    graph-form query (`sem:hasRole ?i . ?i sem:ofTag … ; sem:inMedia ?m`) gives equal bindings;
  - **ambiguous scope:** a performer linked to `?s1` and `?s2` with a short-form fact is
    `ScopeAmbiguous` at the fact's position;
  - **a variable object that is a subject elsewhere** is taken as graph form, not short form.

### Implementation for US1 and US2

- [X] T011 [US1] Implement the tokenizer in `crates/engine/src/query/lex.rs`:
  - tokens with `Pos`: SPARQL 1.1 `IRIREF`, `PNAME_NS`, `PNAME_LN` (with `PLX` percent and
    backslash escapes), `VAR1`/`VAR2`, string forms, `LANGTAG`, numbers, punctuation and
    operators;
  - every SPARQL keyword as a `Keyword` token, so the parser can name refused constructs;
  - `#` comments skipped.
- [X] T012 [US1] Implement `parse(text) -> Result<Ast, QueryError>` in
  `crates/engine/src/query/parse.rs`, per data-model "Ast" and contracts/query-language.md:
  - expand `;` and `,` lists into `TriplePattern`s, each keeping its own `Pos`;
  - `a` becomes `rdf:type`;
  - refuse each construct of "Refused constructs" with `Unsupported { construct }` at its token;
  - check "a projected variable must appear in a triple pattern" and "variables starting
    `__se` are reserved";
  - `SELECT *` projects variables in order of first appearance.

  Makes T005 pass.
- [X] T013 [US1] Implement name resolution in `crates/engine/src/query/names.rs` (research R2;
  data-model "Resolved"):
  - **built-in prefixes** `sem:`, `:`, `tag:`, `performer:`, `scene:`, `image:`, `recursive:`,
    `recursive-tag:`, `rdf:`, `rdfs:` and `xsd:`. User `PREFIX` lines add to them; redefining
    one is `Syntax`;
  - percent-decode local names;
  - **vocabulary checks** through `semantic_vocabulary::embedded()`: an Active term is accepted;
    another status is `Unknown` with the status in `why`;
  - **`:` local names:** a vocabulary term if the name is one, otherwise a tag by name;
  - **lookups** with `Reader::select`, against the one reader passed in:
    - a tag by exact `rdfs:label`, among IRIs of kind tag (`owner_kind`);
    - an item's existence (`ASK`-like `SELECT … LIMIT 1` on subject);
    - for recursive constants, `rdf:type sem:Role` or `sem:Relation`;
  - record each constant's `recursive` mark for T027.

  Makes T006 pass.
- [X] T014 [US2] Implement the rewrite in `crates/engine/src/query/rewrite.rs`, per research R3
  and contracts/query-language.md "Short form and scope":
  - **short form:** verb `hasRole`, `hasAction`, `hasPluggable` or `hasMiseEnScene`, and an
    object that is a tag constant or a variable never the subject of another pattern;
  - **scope of a performer `S`:** the distinct media terms `M` of `M sem:hasPerformer S`. One:
    tie the fact to it. None: a fresh hidden variable. More than one: `ScopeAmbiguous` at the
    fact's position;
  - **emit** `S P ?__seN . ?__seN sem:ofTag O . ?__seN sem:inMedia M`. For `hasMiseEnScene`,
    `S P ?__seN . ?__seN sem:ofTag O`;
  - **pass every other pattern and filter through** unchanged.

  The result is a `Plan`.
- [X] T015 [US1] Implement `print(plan, offset, limit) -> String` in
  `crates/engine/src/query/print.rs`:
  `SELECT DISTINCT ?v… WHERE { … } ORDER BY ?v… OFFSET o LIMIT l`. It writes:
  - full IRIs in angle brackets;
  - literals with escapes;
  - `VALUES` blocks from `plan.values`;
  - `FILTER (a op b)` lines.

  Add a unit test that the printed text of each `known.json` query parses with `Reader::select`.
- [X] T016 [US1] Implement values and a single page in `crates/engine/src/query/page.rs`
  (data-model "Page and continuation"; research R4):
  - **`Value`:**
    - `Item { iri, kind, id }` when `owner_kind` gives a kind;
    - `Term { iri, name }` for `urn:semantic-stash:vocab:` IRIs;
    - `Node { iri }` for other IRIs;
    - `Literal { value, datatype, language }`;
  - **`Page`:** with `generation` and `offset: 0`;
  - **a `serde` serialization** matching contracts/operations-1.4.md "Value types".
- [X] T017 [US1] Implement `engine::query::run(store: &GraphStore, text: &str, req:
  &PageRequest) -> Result<Page, QueryError>` in `crates/engine/src/query/mod.rs`:
  1. open the current reader (errors through `From<GraphError>`);
  2. parse, resolve with that reader, rewrite and print;
  3. evaluate with `Reader::select`;
  4. convert the rows to values.

  Paging arguments are ignored until US5. There is one page, capped at `PAGE_ROWS_MAX`
  bindings, and cut where its serialized `bindings` would pass `PAGE_BYTES`, without a
  continuation. Makes T008 and T010 pass.
- [X] T018 [US1] Write `crates/plugin/tests/ops_query.rs` first, on a plugin folder holding a
  store built from the compatibility fixture:
  - the envelope of contracts/operations-1.4.md: `variables`, `bindings`, `page`, `graph`
    (`generation`, `publishedAt`, `pending`), `seconds`;
  - `INVALID_INPUT` without `query`, or with `pageSize` 0 or over 20,000, or `limit` 0;
  - each `QUERY_*` code with `details.line` and `details.column`;
  - `GRAPH_NOT_BUILT` and `GRAPH_NEEDS_REBUILD`;
  - `CONTRACT_MISMATCH` for `"contract": "2.0.0"`, and success for `"1.0.0"` and `"1.9.0"`;
  - `QUERY_TIMEOUT`, with the limit lowered by the test-only environment variable
    `SEMANTIC_ENGINE_QUERY_TIME_LIMIT_MS` and a cross-join over a synthetic store.
  - the operation makes no request to Stash (a server connection pointing at a closed port).
- [X] T019 [US1] Implement the `query` operation in `crates/plugin/src/query.rs`, and register
  it in `crates/plugin/src/ops.rs`:
  - read and check the arguments;
  - check `contract` (research R10);
  - run `engine::query::run` on a worker thread, and wait up to `QUERY_TIME_LIMIT` (10 s, or
    the test variable). On timeout, write the `QUERY_TIMEOUT` envelope to stdout, flush, and
    `std::process::exit(0)` (research R7);
  - map `QueryError` to the envelope (`code`, message, `details`);
  - `graph.pending` stays `false` until T024.

  Makes T018 pass.

**Checkpoint:** US1 and US2 work on their own: the fixture's known answers and the scope
fixture pass through the engine and through the plugin operation.

---

## Phase 4: User Story 3 — The graph follows saves on its own (P1)

**Goal:** saves of tags, performers, scenes and images record a pending marker, and start one
"Update semantic graph" job per burst. Scene and image saves still start "Apply relations".
Recorded changes are never lost.

**Independent Test:** on the test instance, save one item of each kind, delete and merge tags,
and query without running a task: each change shows (SC-007, SC-008). The hook adds at most
7 ms per saved item (SC-006).

### Tests for US3 (write first; they must fail)

- [X] T020 [P] [US3] Extend `crates/plugin/tests/journal.rs` for the graph markers (research R8;
  data-model "Plugin folder state"):
  - `mark_graph_pending` returns `true` once, then `false`, and `true` again for a marker older
    than `STALE_AFTER`;
  - `take_graph_pending` renames `graph.pending` to `graph.applying`;
  - `finish_graph` removes `graph.applying`;
  - `restore_graph_pending` writes `graph.pending` with a modification time older than
    `STALE_AFTER`, so the next `mark_graph_pending` returns `true`;
  - `graph_pending()` is true for `graph.pending`, or for `graph.applying` younger than
    `STALE_AFTER`, and false otherwise.
- [X] T021 [P] [US3] Extend `crates/plugin/tests/ops_record_change.rs`, with a fake Stash that
  records `runPluginTask` calls:
  - **every trigger of contracts/operations-1.4.md:** each records the graph marker;
  - **the first save of a burst** starts "Update semantic graph"; the next ones start nothing;
  - **`Scene.Update.Post` and `Image.Create.Post`** still append to the relations journal and
    start "Apply relations" as before;
  - **`Tag.Update.Post` and `Performer.Destroy.Post`** do not touch the relations journal;
  - **the result** is `{ recorded, started, graph: { recorded, started } }`;
  - **a failed `runPluginTask`** clears the graph marker, so the next save tries again;
  - **an unknown hook type** records nothing and does not fail.
- [X] T022 [P] [US3] Extend `crates/plugin/tests/ops_update.rs`:
  - an `updateGraph` run takes the marker before reading, and leaves neither marker after
    success;
  - a run ending `BUSY` (a held writer lock), or with `STASH_UNREACHABLE`, leaves a stale
    `graph.pending`;
  - a marker created while the run is going is still there after it.

### Implementation for US3

- [X] T023 [US3] Add the graph markers to `crates/plugin/src/journal.rs`:
  - `GRAPH_PENDING = "graph.pending"`, `GRAPH_APPLYING = "graph.applying"`;
  - `mark_graph_pending`, `take_graph_pending`, `finish_graph`, `restore_graph_pending` and
    `graph_pending`, with the same stale rule as `mark_pending`;
  - the backdated modification time is set with `File::set_modified`.

  Makes T020 pass.
- [X] T024 [US3] Widen `record_change` in `crates/plugin/src/relations.rs`:
  - parse the hook type `Kind.Operation.Post` for Tag, Performer, Scene and Image;
  - Scene and Image `Create`/`Update` keep the spec 004 path;
  - every recognized type calls `mark_graph_pending`. When that returns `true`, call
    `run_plugin_task(PLUGIN_ID, "Update semantic graph")`; on failure, log it and clear the
    marker;
  - return the new result shape;
  - set `graph.pending` in the `query` result (T019) from `graph_pending()`.

  Makes T021 pass.
- [X] T025 [US3] In `crates/plugin/src/build.rs`, make the `updateGraph` task:
  - call `take_graph_pending` before the update takes its catch-up point;
  - call `finish_graph` at the end;
  - call `restore_graph_pending` when it ends with `BUSY` or any error (research R8).

  "Build semantic graph" does not touch the markers. Makes T022 pass.
- [X] T026 [US3] Replace the hook entry in `plugin/semantic-engine.yml` with "Record changes for
  the semantic engine" and the trigger list of contracts/operations-1.4.md. Then, on the test
  instance, check that Stash v0.31.1 loads the manifest, and that each trigger fires. Use
  `spike-bench hooks` for single saves, a `tagsMerge`, a `performerMerge`, a `sceneMerge` and
  deletes of each kind. Remove any trigger that is refused or never fires, and record the
  outcome in `contracts/operations-1.4.md` and `results/hook-triggers.json`.

  Done with the real plugin installed instead of `spike-bench hooks`: each save was made from
  idle, and it counted as fired when the plugin's graph marker appeared or an update followed.
  `Performer.Merge.Post` and `Scene.Merge.Post` never fire, and were removed.

**Checkpoint:** US3 works on its own: saves start updates, and markers survive failures.

---

## Phase 5: User Story 4 — Recursive Role and Relation criteria (P2)

**Goal:** `recursive:` and `recursive-tag:` constants match the tag and every narrower Role or
Relation, by spec 004's hierarchy with loops removed.

**Independent Test:** 004 SC-004 on its library: Full Service recursively gives exactly the Full
Service and Extra Care scenes, and without the mark only Full Service (SC-003).

### Tests for US4 (write first; they must fail)

- [X] T027 [P] [US4] Write `crates/engine/tests/hierarchy_links.rs`: on each definitions fixture
  of `tests/fixtures/relations/`, `Hierarchy::from_links` over the (child, broader) pairs of
  same-type parents equals `Hierarchy::build`, in `broader`, `narrower(_, true)` and
  `expand_criterion`. Include the loop fixture.
- [X] T028 [P] [US4] Write `crates/engine/tests/query_recursive.rs` on a store built from a
  library holding the 004 SC-004 example: Full Service with narrower Extra Care, Customer with
  narrower Patient, scenes of each, and a Role loop. Check:
  - `?r sem:ofRelation recursive:Full%20Service ; sem:inMedia ?s` returns exactly the Full
    Service and Extra Care scenes;
  - the same query with `:Full%20Service` returns exactly the Full Service scenes;
  - `?p :hasRole recursive:Customer` in a scene matches the Patient scenes;
  - `recursive-tag:{id}` gives the same as `recursive:` by name;
  - a Role on the loop answers like `Hierarchy::expand_criterion` and does not fail;
  - `recursive:` on a Trait is `NotRecursive` at its position.

### Implementation for US4

- [X] T029 [US4] Add `Hierarchy::from_links` to `crates/engine/src/relations/hierarchy.rs`. Share
  the cycle removal with `Hierarchy::build` by moving it into one private function that both
  call. Makes T027 pass.
- [X] T030 [US4] In `crates/engine/src/query/rewrite.rs`, for each recursive constant:
  - read the generation's `sem:broaderRole` and `sem:broaderRelation` links once per query;
  - build `Hierarchy::from_links`;
  - replace the constant by a hidden variable, with a `values` entry of
    `expand_criterion(tag, true)` as tag IRIs.

  `names.rs` (T013) already refuses marks on non-Role, non-Relation tags with `NotRecursive`.
  Makes T028 pass.

**Checkpoint:** US4 works on its own on top of US1.

---

## Phase 6: User Story 5 — Large answers come in pages (P2)

**Goal:** pages of at most 1 MiB with stateless continuations tied to one generation and one
query; expired and altered continuations refused.

**Independent Test:** 30,000 bindings read page by page: every page within 1 MiB, every binding
once, even with a publish between pages (SC-005).

### Tests for US5 (write first; they must fail)

- [X] T031 [P] [US5] Extend `crates/engine/tests/graph_read.rs`: `GraphStore::reader_at(n)`
  opens the current generation and the one before it, after a publish. It reports the
  generation gone once it is no longer kept (G7: past `GENERATION_GRACE`, set short in the test
  through the existing test hook, or by removing the folder). It never opens `work/`.
- [X] T032 [US5] Write `crates/engine/tests/query_paging.rs` on a synthetic store of at least
  30,000 scenes (`engine::build::synth`):
  - **"every scene" read page by page:** every page's serialized `bindings` within
    `PAGE_BYTES` (1,048,576); the union equals the single unpaged answer at a large `pageSize`;
    no binding repeats;
  - **`pageSize` 100** gives pages of 100; **`limit` 250** stops after 250 over all pages;
  - **a generation published between pages** (an update with one change) does not change the
    remaining pages, and each page reports the first page's generation;
  - **an expired continuation:** its generation removed gives `PageExpired`;
  - **an invalid continuation:** one altered byte, another query's continuation, and plain
    garbage each give `PageInvalid`;
  - **a single binding over 1 MiB** gives `TooLarge` (a fixture tag with a 1.1 MiB label).

### Implementation for US5

- [X] T033 [US5] Add `GraphStore::reader_at(generation)` in `crates/engine/src/graph/mod.rs`
  and `crates/engine/src/graph/layout.rs`. It opens a kept generation read-only, with the same
  checks as `reader()`, and returns `GraphError::GenerationGone` when the generation is not
  kept. Makes T031 pass.
- [X] T034 [US5] Implement continuations and byte-bounded pages in
  `crates/engine/src/query/page.rs` and `crates/engine/src/query/mod.rs` (research R6;
  data-model "Page and continuation"):
  - **`Continuation`:** `{ v: 1, generation, offset, page_size, limit, query_hash, check }` as
    lower-case hex of its JSON;
  - **`query_hash`:** FNV-1a 64 of the query text;
  - **`check`:** FNV-1a 64 over the key from `query.key` (16 random bytes, created on first use,
    in the store's parent folder) followed by the other fields;
  - **with a continuation:** decode and check it (`PageInvalid`), then open `reader_at`
    (`PageExpired` when gone);
  - **evaluate** with `OFFSET offset` and `LIMIT min(page_size or PAGE_ROWS_MAX, limit −
    offset) + 1`;
  - **fill the page** while the serialized `bindings` stay within `PAGE_BYTES`. A first binding
    over it is `TooLarge`;
  - **continuation:** present when a binding remains and the limit is not reached.

  `PageRequest` validation: `page_size` "1 to 20,000"; `limit` "≥ 1". Makes T032 pass.
- [X] T035 [US5] Pass `pageSize`, `limit` and `continuation` through in
  `crates/plugin/src/query.rs`, and extend `crates/plugin/tests/ops_query.rs`:
  - a two-page answer through the operation;
  - `PAGE_EXPIRED` and `PAGE_INVALID` envelopes;
  - every response under 2 MiB.

**Checkpoint:** US5 works on its own on top of US1.

---

## Phase 7: User Story 6 — Ask the engine how it is (P3)

**Goal:** `status` reports the graph's state, generation, how it was made, counts and pending,
from the generation file alone.

**Independent Test:** after a build, an update and a recorded save, `status` reports what the
store and the markers hold (SC-009).

### Tests for US6 (write first; they must fail)

- [X] T036 [P] [US6] Extend `crates/engine/tests/update_current.rs` (or a new
  `crates/engine/tests/stamp_items.rs`):
  - after a full build of the fixture, the stamp's `items` equals the owners per kind;
  - after each update sequence of `update_equivalence`'s first five, it equals the owners per
    kind of the rebuilt store;
  - a stamp without `items` (written by hand) reads back as absent.
- [X] T037 [P] [US6] Extend `crates/plugin/tests/ops_status.rs`: `graph` in each state of
  contracts/operations-1.4.md:
  - **`ready`** after a build, with `how: "build"`, and after an update, with `how: "update"`;
  - **`notBuilt`** with an empty folder;
  - **`needsRebuild`** with reason `otherFormat`;
  - **`pending: true`** after a `recordChange`;
  - **`how` and `items` `null`** for a generation without them.

  `graph` also appears in `STASH_UNREACHABLE` details. Check that `status` does not open the
  store, by holding the writer lock and timing the call under 50 ms.

### Implementation for US6

- [X] T038 [US6] Add `items` to `BuildStamp` in `crates/engine/src/build/stamp.rs`, as an
  optional field `{ tags, performers, scenes, images }`:
  - **`crates/engine/src/build/run.rs`:** counts the owners it writes per kind;
  - **`crates/engine/src/build/update.rs`:** previous counts plus added, minus removed, per kind.
    A previous stamp without `items` counts the owners once, with `Writer::owners`, and records
    the result.

  Makes T036 pass.
- [X] T039 [US6] Add `GraphStore::describe() -> Description` in
  `crates/engine/src/graph/mod.rs`, with `Ready(Generation)`, `NotBuilt` or
  `NeedsRebuild(RebuildReason)`. It reads `current`, the stamp and the generation file only, and
  never opens a store.
- [X] T040 [US6] Add the `graph` object to `status` in `crates/plugin/src/ops.rs`, in the result
  and in the `details` of `STASH_UNREACHABLE` and `STASH_TOO_OLD`. Its fields:
  - `state`, `format`, `generation`, `publishedAt`, `facts` and `reason`, from `describe()`;
  - `how` and `items`, from the stamp;
  - `pending`, from `graph_pending()`.

  Makes T037 pass.

**Checkpoint:** all six stories work.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T041 Implement `query-bench run` in `crates/plugin/src/bin/query-bench.rs` (research R11).
  It:
  - runs the release plugin binary once per call, with a plugin input JSON (dummy server
    connection, plugin folder pointing at `--dir`), over the fixed query set: plain;
    two-performer join with `!=`; three-performer join; Trait and Role; time comparison;
    recursive Role; recursive Relation; and "every scene" first page at 1 MiB;
  - reports median, p95 and maximum per query and overall;
  - writes `specs/009-query-operations/results/query-250k.json`.
- [X] T042 Run the 250k budget (quickstart §2), and record it in quickstart "Results". The
  targets: p95 under 200 ms for every query set, recursive included, and the 1 MiB page under
  200 ms (SC-004; 004 SC-007). If a query misses, profile and fix before continuing (for
  example the printed pattern order, or the `VALUES` placement), then record before and after.
- [X] T043 [P] Update the contract documents to 1.4.0 (contracts/operations-1.4.md):
  - `api/operations.json`: `query`, `status.graph`, the twelve error codes, and the hook;
  - `docs/api.md`: the `query` section, linking a copy of the language as
    `docs/query-language.md`; the `status` fields; the hook; the error table; and a "1.4.0"
    entry under Changes;
  - the agreement test `crates/plugin/tests/api_agreement.rs` passes again.
- [X] T044 Implement `query-bench fresh` in `crates/plugin/src/bin/query-bench.rs`. It reads the
  test-instance URL and credentials from environment variables, then:
  1. changes one scene's role assignments through the adapter's existing scene custom-field
     write;
  2. calls `runPluginOperation` `query` every 0.5 s until the change shows;
  3. restores the scene;
  4. records the time.

  Add a typed `runPluginOperation` operation to `crates/stash-adapter` if none exists, generated
  against the pinned schema.
- [X] T045 On the test instance (quickstart §3, §4), with the plugin installed by
  `scripts/install.sh`:
  - **hook cost:** run `spike-bench hooks`, single and `--bulk 1000`, with the plugin and with
    it disabled (SC-006, at most 7 ms per item);
  - **freshness:** run `query-bench fresh` (SC-007, under 30 s);
  - **equivalence:** do the manual edits of §4 without running a task, then compare with a full
    rebuild (SC-008).

  Record each in `results/` and quickstart "Results".
- [X] T046 [P] Update `ROADMAP.md` Phase 3: tick the seven items with `specs/009-query-operations`
  and the measured figures. Note in Phase 4 that clients can now adopt `query`.
- [X] T047 [P] Write the plugin notes for the semantic-tagging and viewer maintainers in
  `specs/009-query-operations/client-notes.md`:
  - the language level;
  - the built-in prefixes and tag names;
  - the short form's scope rule, and the ambiguous-scope refusal;
  - paging;
  - `graph.pending`.

  It names the document sections it relies on (Principle II).
- [X] T048 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`, and fix everything. Scan the diff for credentials before any commit.
- [X] T049 Update the graphify graph for the new and changed files (memory: pass
  `crates/engine/src/build/*.rs` to `extract()` explicitly).

### The query index (added in implementation: research R12, after T042's first run)

- [X] T050 Prototype `engine::qindex` (the file, the evaluator) and `query-bench index`; check
  that it answers as Oxigraph does on every known query of the three fixtures
  (`crates/engine/tests/qindex_equivalence.rs`), and measure it at 250k (research R12).
- [X] T051 Owners in the index: entries `(s, p, o, owner)`; the dictionary append-only, with a
  lookup permutation; the rank table removed. Equivalence still holds.
- [X] T052 Streamed answers in `crates/engine/src/qindex/eval.rs` and `query::run`: the page is
  the bindings from `offset` in evaluation order; evaluation stops once one binding more than
  the page is found. The Oxigraph path for generations without an index keeps its sorted order.
  Update the contracts (`query-language.md` Order rule, `operations-1.4.md`, `docs/api.md`,
  `docs/query-language.md`), R4 and R6.
- [X] T053 Names on the index: `names::resolve` and `rewrite::hierarchy_expand` through a
  lookup trait that both `Reader` and `QIndex` implement; `query::run` opens no Oxigraph store
  when the generation has an index.
- [X] T054 Index at publish, full build: `engine::build::run` feeds the index builder as it
  writes, and the writer publishes `snapshots/<n>.qindex` with the generation. Cleanup removes it
  with its generation (`crates/engine/src/graph/writer.rs`).

  Done in the writer itself rather than in `engine::build::run`: a writer that starts empty
  collects every fact it is given, so every way of writing a store gets its index.
- [X] T055 Index at publish, update: the writer records the owners it touched; publishing merges
  the base generation's index, without those owners' facts, with their current facts read from
  the working store. A base without an index is indexed in full from the store once.
- [X] T056 Tests: after every sequence of `update_equivalence`, the updated generation's index
  answers every known query as a rebuilt one does; a generation without an index is answered by
  Oxigraph; a damaged index file falls back to Oxigraph and is reported in the log.
- [X] T057 Re-run T042 on the index, with `query-bench build` writing it as the build does;
  record before and after in quickstart "Results" and `results/query-250k.json`.
- [X] T058 The constitution: record the Principle V deviation for the owner's decision on an
  amendment (`/speckit-constitution`).

  Recorded in plan.md (Constitution Check, Complexity Tracking) and research R12. The
  constitution itself is unchanged, pending the owner's decision.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)** and **Foundational (Phase 2)** block everything.
- **US1+US2 (Phase 3)** depend on Phase 2. They are the MVP.
- **US3 (Phase 4)** depends only on Phase 2. It can run in parallel with Phase 3, except T024's
  last step (`graph.pending` in the `query` result), which needs T019.
- **US4 (Phase 5)** depends on Phase 3: names (T013), rewrite (T014) and run (T017).
- **US5 (Phase 6)** depends on Phase 3: `page.rs` (T016) and `run` (T017).
- **US6 (Phase 7)** depends on Phase 2. T040's `pending` field needs T023.
- **Polish** depends on the stories it measures: T041–T042 on US1, US4 and US5; T044–T045 on US3.

### Within each story

Tests first, written to fail. Then the engine code in pipeline order (lex → parse → names →
rewrite → print → page → run). Then the plugin shell.

### Parallel opportunities

- **Phase 2:** T003 and T004.
- **Phase 3 tests:** T005, T006, T007 and T009 in parallel; T008 after T007; T010 after T009.
- **US3:** T020, T021 and T022 in parallel. The whole of Phase 4 can go alongside Phase 3.
- **US4:** T027 and T028. **US5:** T031 with T032. **US6:** T036 and T037.
- **Polish:** T043, T046 and T047.

## Parallel Example: US1 + US2

```text
Task: "T005 Write crates/engine/tests/query_parse.rs"
Task: "T006 Write crates/engine/tests/query_names.rs"
Task: "T007 Write crates/engine/tests/fixtures/query/known.json"
Task: "T009 Write crates/engine/tests/fixtures/query/scope.json"
```

## Implementation Strategy

### MVP (Phases 1–3)

1. Setup and Foundational.
2. US1 and US2: parse, resolve, rewrite, print, run, and the `query` operation.
3. **Stop and validate:** the known answers and the scope fixture, through the engine and the
   operation. Clients could adopt `query` from here.

### Incremental delivery

1. **US3:** saves keep the graph current. It can be developed alongside the MVP.
2. **US4:** recursive criteria.
3. **US5:** paging. Until it lands, answers are capped at `PAGE_ROWS_MAX`, and nothing over 1 MiB
   is sent.
4. **US6:** `status` reports the graph.
5. **Polish:** budgets at 250k, hook cost, freshness, equivalence, contract documents, roadmap.
