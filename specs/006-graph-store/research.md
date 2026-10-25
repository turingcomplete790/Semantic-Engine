# Research: Graph Store, Oxigraph Embedded

Each decision below resolves an open point from the plan's Technical Context. These sources are
taken as settled:
- the Phase 1 spike's store (`crates/spike/src/store.rs`) and its measurements
  (`specs/003-engine-design-spike/decision-record.md` §1 and §2, rules 1 to 3);
- Oxigraph 0.4.11, the version in `Cargo.lock`, with spargebra 0.3.5;
- Rust 1.99.

## R1. The engine-owned interface

**Decision:** a module `engine::graph` that exposes only engine types. Callers never see
`oxigraph` types (FR-001).

- **Terms:**
  - `Term`: `Iri(String)`, `Blank(String)` or `Literal { value, datatype, language }`;
  - `Fact { subject, predicate, object }`, where `subject` is an IRI or a blank node and
    `predicate` is an IRI.
- **Writing:** `GraphStore::writer(...) -> Writer`, with `add(&[Fact])`, `remove(&[Fact])`,
  `remove_matching(subject?, predicate?, object?)`, `clear()` and `publish() -> Generation`.
- **Reading:** `GraphStore::reader(...) -> Reader`, with `generation()` and
  `select(query) -> Solutions`. `Solutions` holds `variables: Vec<String>` and
  `rows: Vec<Vec<Option<Term>>>`.
- **Other entry points:** `GraphStore::check(...) -> Health`, `GraphStore::summary(...)` and
  `GraphStore::clear_for_rebuild(...)`.
- **Errors:** `GraphError`, with the kinds of FR-019.

Conversion between `Term` and Oxigraph's types is inside the module, in one place.

**Rationale:**
- **Replaceable library:** Principle V asks that Oxigraph can be replaced without touching
  callers. Keeping its types out of every signature is what makes that true.
- **`remove_matching`:** the change applier of the next feature replaces "every fact about item
  X". A pattern removal lets it do that without first reading the facts back.

**Alternatives considered:**
- **Re-exporting Oxigraph's `Store`:** rejected, because it couples every caller to Oxigraph
  (Principle V).
- **A trait with one implementation:** not needed yet. The module boundary gives the same
  replaceability, and a trait can be added when a second backend exists.

## R2. Layout and the stamp

**Decision:** under `{PluginDir}/graph/`:

```text
store.json           stamp: {"format": 1, "serverId": "<uuid>", "createdAt": "<RFC 3339>"}
work/                the working store (only the lock holder opens it)
work.dirty           present while the working store has unpublished changes
snapshots/<n>/       published generation n (an Oxigraph checkpoint), never written again
snapshots/<n>.json   {"generation", "publishedAt", "facts", "format", "serverId"}
current              the published generation number, replaced by atomic rename
writer.lock          advisory lock held for the length of a write
```

**Format number:** `STORE_FORMAT = 1`. Any change to the layout or to how facts are kept bumps
it. A store whose `store.json` has another format, older or newer, needs a rebuild (FR-014,
FR-015).

**Server id:** the value of the plugin setting `serverId` (spec 002), passed in by the caller.
Before any read, the store's `serverId` must equal it.

**Rationale:**
- **The spike's layout, proven:** about 7–8 ms to open, and 0 partial reads across 5,187 reads.
- **The stamp:** it lets a copied plugin folder, or a store from an older engine, be refused
  before any of it is read.
- **The per-generation `.json`:** it holds the fact count and the publication time, so the
  summary (FR-018) never has to count the store (R6).

**Alternatives considered:**
- **The stamp inside the RDF data:** rejected. It would need the store open before it can be
  checked, and a damaged store could not be told apart from a foreign one.
- **One folder per server id:** rejected. The plugin folder is already per Stash server (Stash
  gives each server its own `PluginDir`), and the stamp catches copies.

## R3. Writes, crashes, and "start from the last published state"

**Decision:**
- **Getting the lock:** a writer takes `writer.lock` with `File::try_lock` (stable since Rust
  1.89, the same on Linux, macOS and Windows). If the lock is taken, it gives `Busy` at once
  (FR-004). The operating system releases the lock when the process ends, so a crashed writer
  never blocks later ones (FR-009).
- **Before its first change:** a writer creates `work.dirty`.
- **`publish()`:** flush; checkpoint into `snapshots/<n>/`; count the facts; write
  `snapshots/<n>.json`; replace `current` atomically (write `current.tmp`, then rename);
  remove `work.dirty`; run cleanup (R5).
- **A writer that finds `work.dirty` when it opens:** the last writer stopped before publishing.
  It replaces `work/` with a fresh copy of the current generation before it changes anything
  (FR-008):
  - table files (`.sst`) are hard-linked, as the spike's `copy_generation` does;
  - every other file is copied;
  - with no current generation, `work/` is emptied.

**Rationale:**
- **Atomic for readers:** readers see only published generations, so any set of changes between
  two publishes appears to them all at once, or not at all (FR-006, acceptance scenario US1-5).
- **Crash recovery:** the dirty marker is what makes "start from the last published state" true
  after a crash. Without it, the next writer would build on a half-applied change and publish it.
- **Cheap copy:** hard-linking the table files makes the copy fast. RocksDB never modifies a
  table file after writing it.

**Adding facts:**
- **When `work/` is empty, or a batch holds at least 10,000 facts** (builds): Oxigraph's bulk
  loader. It is not transactional, but the dirty marker makes that safe. Otherwise a large build
  would load every batch after the first transactionally, holding it in memory.
- **Otherwise:** `Store::extend`.
- **Removing facts:** `remove` and `quads_for_pattern` for the patterns.

**Alternatives considered:**
- **One Oxigraph transaction per whole write session:** rejected. Large rebuilds would hold every
  change in memory, and crash safety comes from the generation model anyway.
- **Rebuilding `work/` from the current generation on every writer open:** rejected. It costs a
  copy on every write, when it is only needed after a crash.

## R4. Damage detection

**Decision:** before any read, `check()` runs these steps in order. Each failure is reported
as a distinct `Health` reason (FR-014, FR-015).
1. **`store.json` missing:** `Missing`, or `NotBuilt` if the folder exists but nothing was ever
   stamped.
2. **`store.json` unreadable, or not valid JSON:** `Damaged`.
3. **Format differs from `STORE_FORMAT`:** `OtherFormat { found, expected }`.
4. **`serverId` differs:** `OtherServer`. The ids are not included in messages, since they are
   identifiers.
5. **`current` missing:** `NotBuilt` (FR-013).
6. **`current` unreadable, or naming a generation folder that does not exist:** `Damaged`.
7. **Opening the generation read-only fails:** `Damaged`, with Oxigraph's or RocksDB's message
   kept.

**Rationale:** each step costs at most one small file read, apart from the open, which a read
does anyway. Oxigraph's `validate()` checks every index and would take seconds at 250k scenes,
so it is left for a later "verify store" task.

**Alternatives considered:**
- **Running `validate()` on every read:** rejected. It is far over the open budget (SC-002).
- **Checksums of the store's files:** rejected. RocksDB already checks blocks as it reads them.

## R5. Cleanup of old generations

**Decision:**
- **When:** `publish()` runs cleanup after switching `current`.
- **What it keeps:** the current generation, the one before it, and any generation younger
  than the grace period of 15 minutes (`GENERATION_GRACE`). Every other generation is removed,
  however many have built up.
- **What it removes:** each generation's folder, and its `.json`.
- **If a removal fails** (on Windows, a reader may still have files open): the generation is
  skipped and tried again at the next publish. It never fails the publish.

**Rationale:**
- **Grace period:** Stash runs a plugin call for at most a few minutes, and the adapter's own
  request timeout is 60 s, so a reader that opened a generation has finished with it long before
  15 minutes pass. Keeping the previous generation as well covers a reader that opened it just
  before the switch.
- **Windows:** it refuses to delete files another process has open, so the next publish retries.

**Alternatives considered:**
- **Reader leases (a file per open reader):** rejected. A crashed reader would leave its lease
  behind, and the grace period already covers the longest call.

## R6. Counting facts and the summary

**Decision:**
- **The count:** `publish()` counts the facts once, with `Store::len`, and records the number in
  `snapshots/<n>.json`. The count runs on the writer, in a background job (spec 004's
  pattern), never in a client call.
- **The summary (FR-018):** it reads only the stamp, `current`, that generation's `.json`, and
  the disk size of the store folder. It checks whether a writer is working by trying the lock
  without waiting: if the try succeeds, the lock is released at once.

**Rationale:** `Store::len` reads every fact, which is too slow for a status call at 250k scenes.
At publish time it is part of a build that already takes seconds.

## R7. Queries

**Decision:**
- **Query forms:** `Reader::select(text)` parses the query, rejects every form other than
  `SELECT` with `InvalidQuery`, runs it against the open generation, and turns the solutions
  into `Solutions`.
- **Default graph:** a query's default graph is the store's default graph, where all facts are
  kept (spec Assumptions).
- **Error position:** spargebra's syntax error has no public position, but its message starts
  `error at {line}:{column}: …` (peg's `ParseError<LineCol>`). The engine reads the line and
  column from that text into `InvalidQuery { message, line, column }`. If the text ever has
  another form, line and column are `None`, and the message is kept.

**Rationale:**
- **Error position:** Principle VI asks for the position where available. Reading it from the
  message is the only way without forking spargebra, and the fallback keeps it safe.
- **SELECT only:** Phase 3 defines the client operations. This feature needs only `SELECT`.

**Alternatives considered:**
- **Exposing `CONSTRUCT` and `ASK` now:** rejected as out of scope. They are easy to add later
  behind the same type.

## R8. Measuring at 250k scenes

**Decision:** an engine binary, `graph-bench` (development only, never packaged), with these
commands:
- `build --scenes N` writes a synthetic graph through the interface. Its shape follows the
  spike's synthetic library: about 2 performers per scene, role, action and pluggable instances,
  and a timeline.
- `open` times opening the current generation.
- `query` times a fixed set of `SELECT` queries, including the spike's call-cost query.
- `disk` reports the size per scene and the size of the whole store.
- `stress --readers R --seconds S` is the concurrency check of SC-001, using several reader
  processes.

Results go to `specs/006-graph-store/results/`.

**Rationale:** Principle VII asks for a repeatable command for each budget, and the spike's
synthetic shape gives figures comparable with its 7–8 ms open and 88.7 ms p95. The binary stays
out of the release package. `scripts/package.sh` copies only the plugin binary, and CI's
"no spike files" check gains `graph-bench`.

**Alternatives considered:**
- **Reusing `spike-bench`:** rejected, because the spike crate is history and is not built
  against the engine's interface.

## R9. The concurrency test (SC-001)

**Decision:** an integration test in `crates/engine/tests/graph_concurrency.rs`:
- **Readers:** it starts 4 reader processes by re-running its own test binary with an
  environment variable that selects reader mode.
- **Writer:** at the same time it writes and publishes 50 generations. Each one replaces a block
  of facts with a block that carries the generation number.
- **What each read checks:** every block in the read carries the same generation number, which
  is the generation the reader opened.
- **Pass:** 0 partial reads and 0 store errors across at least 5,000 reads.

`graph-bench stress` runs the same check longer, and at 250k scenes.

**Rationale:** the failure being guarded against happens between processes, which is how Stash
runs the engine, so the test uses processes, not threads. Re-running its own binary needs no
extra binary in the package.
