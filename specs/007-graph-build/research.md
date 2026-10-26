# Research: The Semantic Graph from the Documents

Each decision below resolves an open point from the plan's Technical Context. These sources are
taken as settled:
- vocabulary 1.3.0 (`vocabulary/*.ttl`) and the item-IRI scheme (`semantic_vocabulary::iri`);
- the reader of spec 005 (`stash_adapter::reader`) and the store of spec 006 (`engine::graph`);
- the plugin's design documents at `2c04141`, with `src/ui/timeline.js`, `src/ui/actions.js`
  and `src/semantic/graph.js` read as implementation evidence (the documents win);
- Stash 0.31.1's source (`pkg/plugin/raw.go`), for how a task is stopped;
- the captured test-instance library (`crates/stash-adapter/tests/fixtures/test-instance/`).

## R1. Where the builder lives

**Decision:** a new module, `engine::build`, in two layers:
- **`map`:** pure functions from the reader's records to facts and notes. It takes a
  `LibraryRead` and an `ItemIris`, touches no store and no Stash, and is what the fixtures test.
- **`run`:** the build itself. It takes a `LibraryReader`, a `GraphStore` and a progress
  callback, and runs the order of R4.

The plugin gains one handler, `crates/plugin/src/build.rs`, for the task (R12). It only wires
Stash's input, the logger and the store folder to `engine::build::run`.

**Rationale:**
- **Testable without Stash (FR-022):** the mapping is a pure function of the records, so the
  fixtures need neither a server nor a store.
- **Principle III:** the mapping lives in the engine, where Phase 3's queries will also live.
  The plugin binary stays a thin shell, as in 004.

**Alternatives considered:**
- **Mapping inside the plugin crate:** rejected. The engine is where every rule of the graph
  belongs, and `graph-bench`-style tools need it without the plugin.
- **Mapping driven by reading `stash-mapping.ttl` at run time:** rejected. The mapping file
  records which key produces which term, not the shape of each structure (instances, picks,
  rule nodes). The rules are code, and the coverage test (R10) keeps the code and the file in
  line.

## R2. How Stash stops a task

**Finding:** in Stash 0.31.1, stopping a raw-interface plugin task kills the process
(`rawPluginTask.Stop` calls `cmd.Process.Kill()`). The plugin is never told, and cannot log
anything after it.

**Decision:** stopping is handled by the guarantees spec 006 already gives, not by a stop flag:
- the build publishes only once, at the end. A killed build never reaches `publish()`, so
  `current` still names the previous generation (SC-005);
- the operating system releases `writer.lock` when the process dies (006 R3);
- `work.dirty` is left behind, so the next writer resets the working store from the current
  generation (006 FR-008);
- a kill during `publish()`, before `current` is switched, leaves a generation folder that was
  never published. The next publish reuses its number and removes it first (006 `publish()`).

The reader's `should_stop` is passed a closure that is always false in the plugin. Tests and the
bench may pass their own.

Stash shows the stopped job as cancelled in its job list. That is the "job reports that it was
stopped" of US2-3: the engine has no line to add.

**Test:** a test starts a build in a child process (the test binary re-run in a build mode, as
006 R9 does), kills it while it is adding facts, and checks that:
- `current` is unchanged;
- a reader still answers from the previous generation;
- the next build succeeds.

The test runs 20 kills at varied points, including during publish.

**Alternatives considered:**
- **A stop file or signal handler:** rejected. Stash sends SIGKILL on Unix and `TerminateProcess`
  on Windows, and neither can be caught.

## R3. A fresh working store for a full build

**Decision:** add one method to the store, `GraphStore::rebuild_writer()`. It:
1. takes the lock, as `writer()` does;
2. marks the store dirty;
3. replaces `work/` with an empty store;
4. leaves `current` and the published generations untouched.

The facts are then added to an empty store, so the bulk loader is used for every batch (006 R3).

This is an additive change to the 006 interface. Its contract
(`specs/006-graph-store/contracts/graph-store-interface.md`) gains the method.

**Rationale:**
- **Cost:** `Writer::clear()` on a working store of about 3 million facts deletes every key in
  RocksDB. That leaves tombstones that later compactions must clean, and it is slower than
  starting empty.
- **Previous graph kept:** `clear_for_rebuild()` would remove the published generations too,
  which breaks "stopping leaves the previous graph" (FR-014). The new method keeps them.

**Alternatives considered:**
- **`writer()` then `clear()`:** rejected for the cost above.
- **Making `clear()` itself recreate `work/`:** rejected. `clear()` belongs to an open writer,
  and recreating the folder under an open store is unsafe on Windows.

## R4. The order of a build

**Decision:** `engine::build::run` does, in order:
1. **Server id:** read the plugin setting `serverId`. If it is missing, end with
   `SERVER_ID_NOT_GENERATED` (FR-016).
2. **Health:** `check()`. On `NeedsRebuild(reason)`, run `clear_for_rebuild()` and log the
   reason (FR-015). `Missing`, `NotBuilt` and `Ready` need nothing.
3. **Lock:** `rebuild_writer()`. On `Busy`, end at once with `BUSY` (US2-5). The lock is taken
   before the library is read, so a second build never spends time reading.
4. **Read:** `full_read` through the reader (spec 005). A Stash failure ends the build; the
   working store is dirty, and nothing is published (US2-7).
5. **Map and write:** map the records in this order: tags, performers, scenes, images. Add the
   facts in batches of 50,000.
6. **Vocabulary:** add the vocabulary's own facts (R7).
7. **Publish:** publish once, and log the summary (R8).

**Progress (FR-014):**

| Phase | Share |
|---|---|
| Read | 0 to 30%, from the reader's own `Progress::share` |
| Map and write | 30 to 90%, by records mapped |
| Vocabulary and publish | 90 to 100% |

Progress lines are written at most once per whole percent, so a large build does not flood
Stash's log.

**Memory (the edge case on very large libraries):** the reader's records are held in full,
because one full read is what makes the build see one consistent library (spec Assumptions).
Facts are held one batch at a time. The batch size is 50,000 facts, about 15 MB of terms.

## R5. The mapping rules

**Decision:** the full rule for each key, with every fact it writes, is in
[contracts/mapping-rules.md](contracts/mapping-rules.md). The principles behind it:
- **Classes:** each node gets its most specific class only (`sem:Scene`, `sem:RoleInstance`,
  `sem:Role`). Base classes come from `rdfs:subClassOf` in the vocabulary facts (R7). A tag with
  no flag (a common variant only) gets `sem:Tag`.
- **Performers:** a performer the reader returned gets `rdf:type sem:Performer`. A performer only
  named by an assignment gets its IRI and links, and no type (spec edge case: no performer facts
  are invented).
- **Tag names:** tags get `rdfs:label` with their Stash name, so queries can answer in names
  without another call to Stash. Performer names are never read (spec 005), so they never reach
  the graph or the log.
- **Ids:** an id counts as a valid Stash id when it is a string or JSON integer of decimal digits
  (Stash ids are integers). Plugin-made ids (pairing, entry and rule ids) must pass the item-IRI
  check (letters, digits, `-`, `_`).
- **Timeline order:** the entries' `position` is their index in the stored list, starting at 0.
- **`atTime`:** an instance placed by a slot gets `atTime` with the slot's seconds. A slot with
  no seconds gives no `atTime`.
- **Label threshold:** the stored number when it is greater than 0, otherwise 100. This is the
  documented default, and the rule the plugin's own graph applies (`src/semantic/graph.js`).
- **References inside a record:** a slot or pairing that names an assignment or pairing not
  present in the same record is skipped and noted as `danglingRef`. It is not invented.
  References to tags and performers are mapped even when the reader did not return them
  (FR-012), because those are global and may simply carry no semantic data.
- **Reuse of 004:** `relationEnds` and `relationRules` are read with the same parsing as
  `engine::relations::defs`, so the graph and the `relations` operation agree (Principle III).

## R6. Vocabulary corrections (vocabulary 1.4.0)

Mapping the documents' structures showed four places where vocabulary 1.3.0 cannot say what the
data says. The spec requires the vocabulary to be corrected in this feature. All four are
additive or clarifying, so the release is a MINOR, 1.4.0. The terms are in
[contracts/vocabulary-1.4.md](contracts/vocabulary-1.4.md).

1. **Which end of a pairing is which.** `sem:pairs` links a pairing to both instances, but a
   directed Action pairing ("A gives to B") needs to know which instance is end A.
   - **Fix:** add `sem:pairsA` and `sem:pairsB`, both `rdfs:subPropertyOf sem:pairs`. The
     builder writes these two; queries that do not care about ends keep using `sem:pairs`
     through the sub-property.
2. **Pairing direction values.** `sem:pairingDirection` says "either 'bidirectional' or
   'directed'". The plugin stores `bidirectional`, `give` (A gives to B) and `receive` (A
   receives from B), and treats a missing direction as bidirectional (`src/ui/actions.js`).
   - **Fix:** the comment lists those three values and the default. The builder writes the
     stored value, or `bidirectional` when none is stored. Role pairings store no direction, so
     they get none.
3. **Automatic roles.** `map_autoRoleAssignments` "marks which roles are automatic", but no term
   can mark a role instance. `sem:appliedAutomatically` has the domain `RelationInstance`, and
   changing a domain needs a MAJOR (rule V014).
   - **Fix:** add `sem:roleAppliedAutomatically`, with domain `RoleInstance` and range
     `xsd:boolean`. It is written as true for each role that auto-apply added.
4. **"Derived" values that are stored.** The status Derived reads "defined as a rule over other
   facts; not stored". The four measurement properties have that status, but their input,
   `measurements`, is not a fact. A query could never compute them, and the spec requires them
   in the graph (FR-007).
   - **Fix:** Derived now reads "defined as a rule over other data, not by a Stash key of its
     own. The engine computes it. A term's comment says whether the builder stores it."
   - The four measurement properties say "stored by the builder". `sem:inStage` keeps "not
     stored".

**Alternatives considered:**
- **Ordering the two `pairs` facts:** rejected, because RDF facts have no order.
- **Reusing `appliedAutomatically` on role instances:** rejected. It breaks its declared domain,
  and `vocab-check` (V014) would require 2.0.0.
- **Computing the measurements at query time:** rejected. It would need the raw text in the
  graph and the plugin's parser in SPARQL.

## R7. The vocabulary in the store

**Decision:** every build also writes the facts of `core.ttl`, `labels.ttl` and `timeline.ttl`,
from the vocabulary the engine embeds (`semantic_vocabulary::embedded()`). It does not write
`stash-mapping.ttl`, which describes Stash keys, not the graph.

**Rationale:**
- **Field rules resolve:** a `FieldRule` holds `hasField "__derived_bust"`. The vocabulary fact
  `sem:bustSize sem:pluginFieldName "__derived_bust"` is what turns that into the property
  (US1-5). Without it in the store, the join is impossible.
- **Base classes and sub-properties:** these work in queries
  (`?x a/rdfs:subClassOf* sem:MediaEntity`, `sem:hasRole` as a `sem:hasAssignment`) without
  the builder writing every base class.
- **Cost:** about 1,800 lines of Turtle, a few thousand facts, against millions.

**The "defined term" check (FR-009, SC-003):** it applies to facts whose subject is an item IRI
of this server. Their predicate, and the object of `rdf:type`, must be defined by the vocabulary
or be `rdf:type` or `rdfs:label`.

## R8. Build notes and the summary

**Decision:**
- **A note:** `{ kind, id, key, reason, entry? }`.
  - `kind` and `id` name the item;
  - `key` is the Stash key;
  - `entry` is the entry's position or id, when the note is about one entry of a list or map;
  - `reason` is one of the codes below.
- **Values never appear in a note.** Values can hold names (`tagName` in picks, rule tag names),
  so a note says where the problem is, not what the field held.

| Reason | Kind | Meaning |
|---|---|---|
| `unreadable` | skipped | the reader marked the field unreadable (spec 005) |
| `badShape` | skipped | the value, or one entry of it, is not the documented shape |
| `invalidId` | skipped | an id is not a valid Stash id, or is refused by the item-IRI scheme |
| `unknownKind` | skipped | a timeline entry's `ref.kind` is not one the documents define |
| `retired` | skipped | a retired key or form (the old `mise-en-scene` slot ref) |
| `danglingRef` | skipped | a slot or pairing names an assignment or pairing not in the record |
| `tagMissing` | mismatch | a referenced tag is not among the reader's tags; mapped anyway |
| `tagWrongType` | mismatch | a referenced tag lacks the expected flag; mapped anyway |
| `performerNotInMedia` | mismatch | an assignment names a performer not on the media item; mapped anyway |
| `notRecordedRole` | mismatch | an automatic role that is not in `roleTagAssignments`; mapped anyway |

- **Duplicates:** one note per (item, key, entry, reason).
- **The log:**
  - the first 1,000 notes go to the plugin's log as warnings, one line each;
  - one line then counts the rest;
  - the summary line (FR-017) gives the counts per reason.
- **The summary line**, for example: `build: generation 4 published; 1215 records (17 tags, 886
  performers, 300 scenes, 12 images); 41022 facts; skipped 3 (unreadable 1, badShape 2);
  mismatches 5 (tagWrongType 5); 12.4 s`.

**Rationale:** a note cap keeps a library with one systematic problem from writing 80,000 lines.
The counts still tell the owner how big the problem is.

## R9. Determinism and duplicates

**Decision:**
- **One set per record:** each record's facts are collected in a `BTreeSet<Fact>` before they are
  added. Duplicate entries in a record therefore give one fact (FR-010).
- **Across records:** the store keeps a set of facts, so a fact written twice (a performer's
  facts from two scenes) is held once.
- **Comparison:** fixture comparison sorts the facts as N-Triples lines. Two builds of the same
  records give the same lines (SC-007).

`Fact` needs `Ord`, which is derived on `Term` and `Fact` in `engine::graph::term`. This is
additive.

## R10. Compatibility fixtures

**Decision:** under `crates/engine/tests/fixtures/build/`:
- **`library.json`:** a hand-written `LibraryRead` (spec 005's JSON form). It uses every active
  mapping entry at least once, and includes one bad field of each note reason (SC-006).
- **`expected.nt`:** the full expected graph, as sorted N-Triples lines, without the vocabulary
  facts. It is written by hand from the rules in `contracts/mapping-rules.md`, and reviewed line
  by line.
- **`expected-notes.json`:** the expected notes.

**Tests:**

| Test | What it checks |
|---|---|
| `build_fixture` | maps `library.json` and compares with `expected.nt`. A failure lists missing and extra lines (SC-001) |
| `build_coverage` | for every `sem:Mapping` with status Active in the embedded vocabulary, its `sem:produces` term appears in the fixture graph as a predicate or as the object of `rdf:type`. A failure names the entry (FR-020, SC-002) |
| `build_test_instance` | maps the captured `library-read.json` from the adapter's fixtures, and checks the counts, the instance completeness and the defined-term rule (FR-021, SC-003) |
| `build_notes` | the bad fields of the fixture give exactly the notes of `expected-notes.json`, once each (SC-006) |
| `build_determinism` | maps the fixture twice, and maps it with its record order shuffled, and compares (SC-007) |
| `build_run` | builds through a `MemoryReader` into a temporary store: first build, rebuild after a change, busy writer, store that needs a rebuild, no server id, reader failure (US2) |
| `build_kill` | the kill test of R2 (SC-005) |

**Regenerating `expected.nt`:** an environment variable, `BUILD_FIXTURE_BLESS=1`, makes
`build_fixture` write the mapped graph instead of comparing it. The diff is then reviewed by
hand. The first version is written by hand, not blessed.

## R11. Measuring the budgets (FR-023, SC-004)

**Decision:** a development binary, `build-bench` (in `engine/src/bin`, never packaged), with two
commands:
- **`synth --scenes 50000 --images 29000 --dir D`:** generates a fully tagged `LibraryRead` of
  Production size and builds it into `D` through a `MemoryReader`. "Fully tagged" means every
  media item has role, action and pluggable assignments, pairings and a timeline with a Stage,
  in the test instance's shape.
- **`stash --dir D`:** builds from a real Stash, using `STASH_URL` and `STASH_API_KEY` from the
  environment, into a local store folder. The owner runs it against Production, read-only, as
  005's `reader-bench` was run.

Each reports, as JSON, the time per phase (read, map, write, publish), the counts, and peak
memory. On Linux, peak memory is `VmHWM` from `/proc/self/status`; elsewhere it is reported as
unknown.

**The test instance:** the task is run from Stash, and the time is taken from the summary line.

**Budgets:**

| Run | Budget |
|---|---|
| Production, from Stash | under 5 minutes (SC-004, 003 decision record) |
| Synthetic Production size | under 5 minutes |
| Peak memory at synthetic Production size | under 2 GB, stated now and measured. 006's 250,000-scene build peaked at 1.7 GB |

**Expected cost:** about 3 million facts at Production size. At 006's measured load rate (9.8
million facts in 74 s), loading is about 25 s. Mapping and the in-memory read are smaller.

## R12. The task and the contract (FR-013, FR-018)

**Decision:**
- **The task:** the manifest gains "Build semantic graph", which runs operation `buildGraph`.
- **The contract:** it moves to 1.2.0, a MINOR, additive change, recorded in
  [contracts/operations-1.2.md](contracts/operations-1.2.md).
- **One new error code, `BUSY`:** the store has another writer. It is additive.
- **Stash failures:** they map as in 004 (`stash_error`):
  - unreachable or timed out: `STASH_UNREACHABLE`;
  - too old: `STASH_TOO_OLD`;
  - anything else, including not authorized: `INTERNAL`, with the adapter's message, which names
    the failure.
- **The graph folder:** `{PluginDir}/graph`, the folder 006 fixed.

**Rationale:** Stash does not return a task's output to anyone, so the envelope is for the
contract and its tests. The owner reads the job log, as with "Generate server identifier".
