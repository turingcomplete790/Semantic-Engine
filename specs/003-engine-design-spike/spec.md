# Feature Specification: Engine Design Spike

**Feature Branch**: `003-engine-design-spike`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "Phase 1 spike" (`ROADMAP.md` Phase 1: settle Principle IV's open
questions with measured answers before any feature spec; the output is a written decision record).

## Overview

The constitution (Principle IV) leaves four design questions open until a spike answers them:
how state persists between plugin calls, how concurrent calls are kept from corrupting it, how
the engine learns that Stash data changed, and whether running inside Stash's plugin system is
fast enough at all. The roadmap adds two practical limits clients will hit (how large one answer
can be, and how a client gets a whole snapshot) and the cost of building and shipping the engine.

This feature produces no client-facing behaviour. It delivers measurements taken by repeatable
commands, small throwaway prototypes that make those measurements possible, and one decision
record that the Phase 2 and Phase 3 specs cite instead of guessing. Its last item is a go/no-go:
proceed with the shared engine, or fall back to each client keeping its own in-process engine.

Known from feature 002 (Stash v0.31.1, local test server): starting the plugin process costs
about 4 ms; `ping` through `runPluginOperation` takes about 9 ms and `status` about 21 ms; the
first call after Stash reloads its plugins takes about 1.3 s. Stash's source shows that both of
its external plugin interfaces start a new process for every call, so a process that outlives a
call is something the engine would have to arrange itself.

## User Scenarios & Testing *(mandatory)*

The "users" here are the people who write the next specs: the engine maintainer, and the
maintainers of the Semantic Stash Viewer and the semantic-tagging plugin, who must decide whether
to depend on the engine.

### User Story 1 - Decide the process and persistence model (Priority: P1)

The engine maintainer needs to know what one engine call costs when the semantic graph is stored
on disk and reopened on every call, and whether any alternative that keeps state warm between
calls is both allowed by the constitution and worth its complexity. They also need to know which
way of sharing that on-disk graph between calls is safe when calls overlap: one writer with
read-only readers, or readers that only see published snapshots.

**Why this priority**: Every later phase depends on it. If reopening a stored graph per call is
too slow, the whole "engine as a Stash plugin" design changes or is abandoned.

**Independent Test**: Run the measurement command on the test server with a graph built from the
test library and read the recorded numbers: per-call cost with the graph reopened, and the result
of overlapping a long write with repeated reads.

**Acceptance Scenarios**:

1. **Given** a stored graph built from the test library, **When** the measurement command runs a
   small query through Stash 100 times, **Then** it records the median and 95th-percentile time
   per call and how much of it is opening the stored graph.
2. **Given** a write in progress on the stored graph, **When** read calls arrive at the same time,
   **Then** the record states for each sharing option whether reads succeed, wait, or fail, and
   whether any read ever saw a half-written state.
3. **Given** an engine process that outlives a call is considered, **When** it is evaluated,
   **Then** the record states whether it can work without opening a listening socket or talking
   to anything but Stash, and what it would save per call compared with reopening.

---

### User Story 2 - Decide how the engine stays fresh (Priority: P1)

The engine maintainer needs to know which saves in Stash reach the engine as hook calls, and how
quickly: a tag, scene, image, or performer saved from Stash's web interface, from the
semantic-tagging plugin, and from the viewer. Where a change does not arrive as a hook, they need
to know what other signal can catch it up (for example the "updated since" times Stash keeps).

**Why this priority**: The viewer's spec requires a change to show up within 60 seconds. If hooks
miss changes, the freshness design must include a catch-up path from the start.

**Independent Test**: With a hook-recording prototype installed on the test server, make each kind
of save from each client and check the recorded table of which hooks fired, with what content, and
how long after the save.

**Acceptance Scenarios**:

1. **Given** the hook-recording prototype is installed, **When** a tag, scene, image, and performer
   are each saved from the web interface, the semantic-tagging plugin, and the viewer, **Then**
   the record has one row per save and client, saying whether a hook arrived, which one, what it
   identified, and the delay.
2. **Given** a bulk change (for example tagging many scenes at once, or merging two tags), **When**
   it is made, **Then** the record states how many hook calls Stash made and whether any change was
   missed or arrived out of order.
3. **Given** a change that produced no hook, **When** the catch-up signal is checked afterwards,
   **Then** the record states whether that signal shows the change, including deletions.

---

### User Story 3 - Know the limits clients will hit (Priority: P2)

Client maintainers need to know the largest answer one engine call can return through Stash before
it fails or slows sharply, and how a client should obtain a whole snapshot of the engine's state:
inside an operation result, or as a file Stash serves from the plugin's folder.

**Why this priority**: These limits decide how Phase 3 pages its results and how the viewer catches
up, but they do not block the go/no-go on their own.

**Independent Test**: Run the measurement command with answers of growing size and read the
recorded size at which calls fail or exceed the latency budget; fetch a snapshot both ways and
read the recorded times and any access rules.

**Acceptance Scenarios**:

1. **Given** answers of increasing size, **When** each is returned through Stash, **Then** the
   record states the time per size and the size at which calls fail or exceed 1 second, and proposes
   a page size for Phase 3.
2. **Given** a snapshot of the test library's graph, **When** a client fetches it as an operation
   result and as a file Stash serves, **Then** the record states the time for each, whether each
   needs the client's usual Stash login, and how a client would know the snapshot is current.

---

### User Story 4 - Make the go/no-go decision (Priority: P2)

The engine maintainer and both client maintainers need one decision record that collects the
answers, compares them with the budgets the clients already have, and says whether to proceed with
the shared engine, proceed with stated changes, or fall back to clients keeping in-process engines
only.

**Why this priority**: It is the deliverable the roadmap asks for; it depends on the other stories.

**Independent Test**: Read the decision record alone and check that every Phase 1 roadmap question
has an answer, a number or observation behind it, the command that reproduces it, and a resulting
rule for later specs.

**Acceptance Scenarios**:

1. **Given** the measurements are recorded, **When** the decision record is written, **Then** each
   of the seven Phase 1 roadmap items has a decision, the evidence for it, and how to reproduce it.
2. **Given** the decision is to proceed, **When** Phase 2 is specified, **Then** its spec can state
   persistence, concurrency, and freshness by citing the record, as Principle IV requires.
3. **Given** the decision is not to proceed, **When** the record is read, **Then** it names which
   measured limit failed and what each client should do instead.

### Edge Cases

- Stash reloads its plugins, or restarts, in the middle of a measurement run: the first-call cost
  (about 1.3 s) is recorded separately and never mixed into the per-call figures.
- The stored graph is missing, damaged, or locked by a call that crashed: the record states what
  the next call sees and how long recovery takes.
- Two writes start at once (for example two hooks fired together): the record states which sharing
  option serialises them and what the losing call reports.
- A hook arrives while a rebuild is running: the record states whether the change is lost, queued,
  or applied after the rebuild.
- A save changes only a field the engine ignores: the record states whether a hook still fires, so
  later specs can decide whether to filter.
- Measurements vary between runs: each figure is taken from repeated runs, and the record gives the
  spread, not a single sample.
- The test library is smaller than the owner's largest library: figures that depend on library size
  are reported per item (for example per scene or per tag) so they can be scaled, and the scaling
  is stated as an estimate.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The spike MUST produce one decision record, kept in this feature's folder, that
  answers each Phase 1 item in `ROADMAP.md` with a decision, the evidence, and the command that
  reproduces it.
- **FR-002**: Every figure in the record MUST come from a repeatable command kept in the repository,
  as constitution Principle VII requires; a reader MUST be able to re-run it against a Stash server
  and get comparable numbers.
- **FR-003**: The spike MUST measure per-call cost through Stash with the semantic graph stored on
  disk and reopened on every call, for a small query, reporting median and 95th percentile over at
  least 100 calls, and separating time spent opening the stored graph.
- **FR-004**: The spike MUST evaluate at least two ways of sharing the stored graph between
  overlapping calls (one writer with read-only readers; readers of published snapshots only) and
  record, for each, whether concurrent reads succeed, wait, or fail during a write, and whether a
  reader ever sees a partly written state.
- **FR-005**: The spike MUST evaluate whether an engine process that outlives a single call is
  possible without opening a listening socket, contacting any host other than the invoking Stash,
  or persisting credentials (Principle IV), and record what it would save per call. If it cannot
  meet those rules, the record MUST say so and drop it.
- **FR-006**: The spike MUST record, for saves of a tag, a scene, an image, and a performer made from
  Stash's web interface, the semantic-tagging plugin, and the viewer, whether a hook reaches the
  plugin, which one, what it identifies, and the delay from the save.
- **FR-007**: The spike MUST record hook behaviour for bulk edits and tag merges and deletions, and
  check whether Stash's "updated since" times reveal each change (and how deletions are found) so a
  catch-up path can be specified.
- **FR-008**: The spike MUST find the largest answer one call can return through Stash before it
  fails or exceeds 1 second, and propose a page size for Phase 3.
- **FR-009**: The spike MUST compare delivering a whole snapshot as an operation result and as a file
  Stash serves from the plugin's folder, recording time, size, the access rules each one has, and
  how a client tells whether a snapshot is current.
- **FR-010**: The spike MUST record the engine's build time on a clean machine and in CI, and the
  size of each release package, and compare them with the budgets the viewer's 009 spec sets
  (SC-001: a first build of the owner's largest library within 5 minutes; SC-002: ready within 2
  seconds from a stored copy; SC-003: Phase 1 queries' first results in under 200 ms).
- **FR-011**: The spike MUST build the semantic graph from the test library well enough to measure
  build time and stored size, using bounded, flag-filtered requests only (Principle VII); it MUST
  report time and size per item so they can be scaled to a larger library.
- **FR-012**: The spike MUST NOT change any data in Stash other than test data on the test server,
  and MUST NOT write to the owner's other servers at all.
- **FR-013**: Prototype code written for the spike MUST NOT ship in a release package or change the
  operations in the API document (contract 1.0.0). The measurement commands MAY stay in the
  repository for re-use.
- **FR-014**: The decision record MUST end with one of three outcomes: proceed; proceed with listed
  changes to the roadmap or constitution; or fall back to clients keeping in-process engines. A
  "proceed" outcome MUST state the persistence, concurrency, and freshness rules Phase 2 will use.
- **FR-015**: If an answer requires a constitution change (for example a different persistence or
  process model than Principle IV allows), the record MUST name the principle and the proposed
  amendment; the amendment itself is made separately.

### Key Entities

- **Measurement**: one repeatable figure: what was measured, the command, the server and library it
  ran against, the number of runs, and the median, 95th percentile, and spread.
- **Option evaluated**: a candidate answer to one question (for example "one writer with read-only
  readers"), with its measurements, observed failures, and whether it meets the constitution.
- **Hook observation**: one row per save and client: entity type, client that saved it, hook received
  (or none), content identified, delay.
- **Decision**: per roadmap question: the chosen option, why, the evidence, and the rule later specs
  must follow.
- **Decision record**: the set of decisions plus the overall go/no-go outcome, its date, the Stash
  version, and the engine version it was measured with.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All seven Phase 1 items in `ROADMAP.md` have a recorded decision backed by at least one
  measurement or observation; none is left as "to be decided".
- **SC-002**: Every figure in the decision record can be reproduced by one documented command, and a
  second run on the same server gives a median within 25% of the recorded one.
- **SC-003**: The hook table covers all 12 combinations of entity type (tag, scene, image, performer)
  and saving client (web interface, semantic-tagging plugin, viewer).
- **SC-004**: The record states whether a small query through Stash, with the graph reopened per call,
  answers within the viewer's 200 ms budget at the 95th percentile on the test library.
- **SC-005**: The spike leaves the test library's item counts unchanged apart from test data it
  created, and touches no other server.
- **SC-006**: A maintainer of either client, given only the decision record, can say whether their
  client should depend on the engine and what page size and freshness delay to expect; checked by the
  owner reading it.

## Assumptions

- **Test environment**: measurements run against the local test server (Stash v0.31.1 at
  localhost:9998) and its test library, which is where writes are authorised. The owner's largest
  library (Production: 49,683 scenes including clips on 2026-10-16, growing daily) is not
  available to this spike. Size-dependent figures are therefore also measured on synthetic graphs
  of 50,000, 100,000, and 250,000 scenes, reported per item, and scaled beyond that as marked
  estimates. The owner may re-run the commands on another
  server later; nothing in the spike requires it.
- **Saving from each client**: the semantic-tagging plugin and the viewer are exercised as they are
  today, by the owner or by scripted use of the same Stash requests each client makes; where a
  client cannot be driven, the row records the equivalent request and says so.
- **Prototypes are throwaway**: graph-building, sharing, and hook-recording code exists only to
  measure. Phase 2 re-implements what the decision record chooses, test-first, under its own spec.
- **No new client operations**: the plugin's published operations stay as contract 1.0.0; any spike
  operation is internal, absent from release packages, and removed or hidden before Phase 2.
- **Budgets come from the clients**: the viewer's 009 spec budgets (5-minute first build, 2-second
  ready time, 200 ms query) are the yardsticks; this spike does not set new client budgets, it
  measures against them and proposes engine-side ones for Phase 2.
- **Platform**: measurements are taken on Linux; build time and package size are taken from CI for
  all five release targets. Other platforms' runtime behaviour is assumed similar unless CI shows
  otherwise.
