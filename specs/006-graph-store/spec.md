# Feature Specification: Graph Store, Oxigraph Embedded

**Feature Branch**: `master` (spec directory `006-graph-store`)

**Created**: 2026-10-25

**Status**: Draft

**Input**: User description: "Phase 2: Semantic Graph - Oxigraph Embedded." (ROADMAP Phase 2:
"Oxigraph embedded behind an engine-owned interface, its on-disk store on the bundled RocksDB",
and "Store with published snapshot generations, a single writer, and readers of published
generations only")

## Context

The engine answers semantic questions from a graph built out of the Stash library. Spec 005 gave
it a reader for the library. This feature gives it the place the graph lives: an embedded graph
store, kept on disk in the plugin's folder, that every engine call can open quickly.

Stash starts a new engine process for every call. Hooks, tasks and client queries can run at the
same time, so the store must stay correct when one process writes while others read. The Phase 1
spike measured the options (`specs/003-engine-design-spike/decision-record.md` §1, §2, rules 1
to 3), and constitution v3.0.0 Principle IV fixed the model:

- **Working store:** one working store, written only by the holder of the writer lock.
- **Generations:** numbered read-only snapshot generations, published by an atomic switch.
- **Readers:** they open only a published generation, never the store being written. In the
  spike, readers of the live store saw a partial write in 72% of reads.

The spike's measurements for this model:
- **Opening:** opening a generation took 7–8 ms at every library size.
- **Queries:** a query call's p95 was 88.7 ms at 250,000 scenes.
- **Size:** the store held about 8.3 KB per scene.

Constitution Principle V keeps the store behind an interface the engine owns, so the graph
library can be replaced without touching its callers. Principle I makes the store a disposable
cache: rebuilt when it is missing, damaged or from an older format, and never shared between
Stash servers.

This feature does not decide what goes into the graph. Turning the reader's records into graph
facts with the vocabulary is the next Phase 2 item. Here the store accepts facts, holds them,
publishes them, and answers queries over them.

## User Scenarios & Testing *(mandatory)*

The users of this feature are the engine's other parts:
- the graph builder and change applier, which write;
- the operations and tasks, which read;
- the status operation, which reports.

Owners benefit indirectly. Answers are never half-written, and a broken or stale store repairs
itself instead of returning wrong results.

### User Story 1 - Write facts and publish them for readers (Priority: P1)

The graph builder opens the store for writing. It holds the writer lock, so no other process can
write. It adds and removes facts, and then publishes. Publishing makes the new state a numbered
generation that any reader can open. Until it publishes, readers keep seeing the previous
generation, unchanged.

**Why this priority**: Nothing else in Phase 2 or Phase 3 works without a place to write the graph
and a safe way to hand it to readers.

**Independent Test**: In a test, write a set of facts and publish them. Open the published
generation in another process and find exactly those facts. Write more without publishing, and
check that the reader still sees the first set.

**Acceptance Scenarios**:

1. **Given** an empty store, **When** the builder writes facts and publishes, **Then** a reader
   that opens the store sees exactly those facts, in generation 1.
2. **Given** a published generation, **When** the builder writes more facts but has not
   published, **Then** every reader still sees the published generation, unchanged.
3. **Given** a writer holding the lock, **When** a second process asks to write, **Then** it is
   told the store is busy within 50 ms. It neither waits nor writes.
4. **Given** a writer that crashes before publishing, **When** a reader opens the store, **Then**
   it sees the last published generation, whole and unchanged. **When** the next writer starts,
   **Then** it can continue: its work begins from the last published state.
5. **Given** a writer that adds and removes facts in one change, **When** it publishes, **Then**
   readers see either all of that change or none of it.

---

### User Story 2 - Read and query a published generation (Priority: P1)

An engine call opens the current published generation read-only and asks a question in the
graph query language the constitution names (SPARQL `SELECT`). It gets the answer's bindings. A
generation the call has opened stays the same until the call ends, even if a newer one is
published meanwhile.

**Why this priority**: Every operation that answers a client depends on this. Phase 3 builds the
query operations on top of it.

**Independent Test**: Publish a known graph. Run a set of queries against it with expected
answers. Publish a newer generation while a reader holds the old one open, and check that the
reader's answers do not change mid-call.

**Acceptance Scenarios**:

1. **Given** a published generation, **When** a call opens it and runs a `SELECT` query,
   **Then** it gets the bindings the query defines, in the order the query asks for.
2. **Given** a reader with generation 5 open, **When** generation 6 is published, **Then** the
   reader's later queries in the same call still answer from generation 5. The next call opens
   generation 6.
3. **Given** a query that is not valid, **When** it is run, **Then** it fails with a structured
   error naming the problem and its position in the query (Principle VI), not a panic or an
   empty answer.
4. **Given** a store with no published generation yet, **When** a call opens it for reading,
   **Then** it is told the graph is not built yet, distinct from an empty graph.

---

### User Story 3 - A store that repairs itself (Priority: P2)

Before a call reads, the store checks that it is usable. It must be present, readable, of the
current format, and built for this Stash server. If any check fails, the call is told the store
needs a rebuild and why. The store can then be cleared and started again. Old generations that no
reader can still be using are removed, so the store does not grow without bound.

**Why this priority**: Constitution Principle I requires the store to be rebuilt when missing,
damaged or from an older format, and never shared across servers. Without this, a copied plugin
folder or an engine upgrade could make the engine answer from the wrong library.

**Independent Test**: Use fixtures for each case:
- a missing store;
- a damaged one, with truncated or removed files;
- one from an older format version;
- one stamped for a different server id.

Check that each is reported as needing a rebuild, with its own reason. Check that clearing it
gives an empty, usable store. Publish many generations, and check that only the ones still
needed are kept.

**Acceptance Scenarios**:

1. **Given** no store, **When** a call checks it, **Then** the answer is "needs a build: missing".
2. **Given** a store whose files are damaged, **When** a call opens it, **Then** the answer is
   "needs a rebuild: damaged", with the underlying error kept for the log. No answer is
   returned from it.
3. **Given** a store written by an older store format, **When** a call opens it, **Then** the
   answer is "needs a rebuild: older format", naming both formats.
4. **Given** a store stamped with another server's id, **When** a call opens it, **Then** the
   answer is "needs a rebuild: belongs to another server". Nothing is read from it.
5. **Given** a store that needs a rebuild, **When** the builder clears it, **Then** the store is
   empty, stamped with this server's id and the current format, and ready for a first write.
6. **Given** many published generations, **When** a writer publishes again, **Then** old
   generations are removed. The current generation, the one before it, and any generation
   published within the grace period are kept.

---

### User Story 4 - Report what the store holds (Priority: P3)

The status operation and the owner's logs can ask the store for a summary:
- the current generation number and when it was published;
- the store format version and the server id it belongs to;
- how many facts it holds;
- its size on disk;
- whether a writer is working now.

**Why this priority**: It makes the store observable. It is not needed for correctness, so it
comes last.

**Independent Test**: Publish a known graph, ask for the summary, and compare each field with what
was written.

**Acceptance Scenarios**:

1. **Given** a published generation, **When** the summary is asked for, **Then** it reports the
   generation number, publication time, format, server id, fact count and disk size.
2. **Given** a writer holding the lock, **When** the summary is asked for, **Then** it says a
   write is in progress, without waiting for it.

---

### Edge Cases

- **Two writers start at once:** exactly one gets the lock. The other is told the store is busy.
- **Two publishes in a row:** generations are numbered without gaps or repeats, even when a
  writer crashed between publishes.
- **A reader during cleanup:** a generation a reader has open is never removed while that reader
  can still be using it. The grace period is longer than the longest call Stash allows.
- **Disk full during a write or publish:** the publish fails with a clear error. The last
  published generation stays current and readable.
- **A stale lock:** a writer that crashed while holding the lock does not block writing forever.
  The lock is released when its process ends.
- **Unicode and long values:** facts with non-ASCII text, long literals, and ids that are not
  valid in plain text are stored and returned unchanged.
- **The plugin folder copied to another Stash server:** the server-id check refuses it (US3).
- **An engine downgrade:** a store written by a newer format than the engine knows is treated
  like an older one. It needs a rebuild and is never read.
- **Many generations at once:** if cleanup has not run for a long time, the next publish removes
  every generation that is no longer needed, not just one.
- **Platforms:** the same behaviour holds on Linux, macOS and Windows, the release targets of
  spec 002.

## Requirements *(mandatory)*

### Functional Requirements

**The interface**

- **FR-001**: The engine MUST reach the graph store only through an interface it owns. No caller
  outside the store module may use the graph library's own types, so the library can be replaced
  without changing callers (Principle V).
- **FR-002**: The store MUST be embedded in the engine process, with no separate server, no
  network listener, and no system-wide installation (Principles IV and V).
- **FR-003**: The store MUST keep its data on disk under the plugin's folder, one store per Stash
  server. It uses the bundled on-disk engine the constitution names; no system copy is used
  (Technology & Platform Constraints).

**Writing and publishing**

- **FR-004**: Only the holder of the writer lock MAY open the working store for writing. A second
  writer MUST be told the store is busy at once, never made to wait, and never allowed to write.
- **FR-005**: A writer MUST be able to add facts and remove facts. It MUST be able to clear the
  working store.
- **FR-006**: Publishing MUST make the working store's current state a new numbered generation
  in one atomic step. Readers MUST see either the previous generation or the new one, never a
  mixture.
- **FR-007**: Generation numbers MUST increase by one with each publish, starting at 1, with no
  gaps and no repeats across crashes.
- **FR-008**: If a writer stops before publishing, the last published generation MUST stay
  current and readable. The next writer MUST start from the last published state.
- **FR-009**: A writer that ends, normally or not, MUST release the lock. A crashed writer's lock
  MUST NOT block later writers.

**Reading and querying**

- **FR-010**: A reader MUST open only a published generation, read-only, and MUST NOT open the
  working store (decision record rule 2).
- **FR-011**: A reader MUST keep the generation it opened for the whole call, even if a newer one
  is published meanwhile.
- **FR-012**: A reader MUST be able to run a SPARQL `SELECT` query against its generation and
  receive the bindings. An invalid query MUST give a structured error with the problem and,
  where the query language allows, its position (Principle VI).
- **FR-013**: Opening a store with no published generation MUST be reported as "not built yet",
  distinct from a built, empty graph.

**Health and repair**

- **FR-014**: The store MUST record its format version and the Stash server id it belongs to. It
  MUST check both, and its own integrity, before any read.
- **FR-015**: A store that is missing, damaged, of another format version, or stamped for another
  server MUST be reported as needing a rebuild, each with its own reason. It MUST NOT be read
  (Principle I).
- **FR-016**: The builder MUST be able to clear a store that needs a rebuild. Clearing leaves an
  empty store, stamped with this server's id and the current format.
- **FR-017**: Publishing MUST remove generations that are no longer needed. It keeps the current
  generation, the one before it, and any published within a stated grace period. The grace
  period MUST exceed the longest call Stash allows a plugin.

**Reporting**

- **FR-018**: The store MUST report a summary: current generation and its publication time,
  format version, server id, fact count, size on disk, and whether a write is in progress. It
  reports this without waiting for a writer.

**Errors and limits**

- **FR-019**: Every store failure MUST be a distinct, structured kind. The kinds are: busy, not
  built yet, needs a rebuild (with its reason), invalid query, disk or input/output failure, and
  internal failure. No failure may return a silently wrong or partial answer (Principle IV).
- **FR-020**: The store MUST NOT write any credential, and MUST NOT contact any host.

**Budgets (Principle VII)**

- **FR-021**: The store MUST state, and measure with a repeatable command, the time to open the
  current generation for reading, the time to publish, and the disk use per scene. These are
  measured at the test library's size and at a synthetic 250,000-scene library.

### Key Entities

- **Graph store**: the engine's on-disk graph for one Stash server. It holds the working store,
  the published generations, the writer lock, and a stamp of format version and server id.
- **Fact**: one statement in the graph, such as "this scene has this performer". The store
  treats facts as data; their meaning comes from the vocabulary.
- **Generation**: a numbered, read-only state of the graph, published at one moment. Exactly one
  generation is current.
- **Writer**: the one process allowed to change the working store, for as long as it holds the
  lock.
- **Reader**: a process that has opened one generation, read-only, for one call.
- **Store stamp**: the format version and server id recorded with the store, and checked before
  every read.
- **Store summary**: the report of US4.
- **Store failure**: one of the kinds in FR-019, with a message and, for rebuilds, the reason.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a concurrency test with readers running while a writer writes and publishes
  repeatedly, 0 of at least 5,000 reads see a partial change or fail with a store error. The
  spike's live-store option failed 72% of such reads.
- **SC-002**: Opening the current generation for reading takes under 15 ms at the median, on the
  test library and on a synthetic 250,000-scene library. The spike measured 7–8 ms.
- **SC-003**: A query call on a 250,000-scene store stays under 200 ms at p95, including opening
  the store (decision record rule 12).
- **SC-004**: A second writer learns the store is busy in under 50 ms in 100% of attempts.
- **SC-005**: After a writer is killed mid-write, 100% of following reads return the last
  published generation, and the next writer succeeds.
- **SC-006**: Each of the four rebuild cases (missing, damaged, older format, other server) is
  reported with its own reason in 100% of its test cases, and none is ever read from.
- **SC-007**: Once cleanup has run, the whole store takes no more than 3 times the disk of one
  generation. That covers the working store, the current generation and the one before it,
  which share unchanged data. The spike measured about 8.3 KB per scene for one generation.
- **SC-008**: The full test suite passes on Linux, macOS and Windows with no Stash server and no
  network.

## Assumptions

- **Store location:** the plugin's own folder, as Stash gives it to the plugin (`PluginDir`),
  in a subfolder named for the store. The journal and watermark files of spec 004 stay where
  they are.
- **Server id:** the id generated by spec 002's "Generate server identifier" task. If it has not
  been generated yet, the store cannot be stamped. Building then waits, and the call is told why,
  as `getServerId` already reports.
- **Grace period:** 15 minutes. Stash lets a plugin call run at most a few minutes, and the
  adapter's own request timeout is 60 s, so 15 minutes is well clear of the longest call. The
  plan may adjust it with a measurement.
- **Layout:** the spike's layout (`work/`, `snapshots/<n>/`, `current`, `writer.lock`) is the
  starting point. The plan confirms it against this spec.
- **Locking:** the operating system's advisory file lock, released by the system when a process
  ends, as the spike used. No locking library is added.
- **Out of scope:**
  - Turning library records into graph facts. That is the next Phase 2 item ("Vocabulary from
    the documents").
  - Incremental builds, the change journal, hook wiring, and Stash tasks that build the graph.
  - Query operations for clients, paging, and result-size limits (Phase 3).
  - Any change to the operations contract (it stays 1.1.0) or to the status operation's
    output. US4's summary is an internal report until Phase 3's status item exposes it.
- **Query language:** SPARQL `SELECT` is enough for this feature's tests and for Phase 3 to build
  on. `CONSTRUCT` and SPARQL Update are not exposed to readers.
- **Facts:** the store keeps facts in one default graph. Named graphs per generation are not
  needed, because a generation is a whole store state.
