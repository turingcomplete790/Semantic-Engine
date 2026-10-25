# Feature Specification: Stash Adapter: the Library Reader

**Feature Branch**: `master` (spec directory `005-stash-adapter`)

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "Stash adapter crate" (ROADMAP Phase 2, first unticked item: "the one
place that talks to Stash, using the Cynic GraphQL library with typed operations generated against
a pinned copy of Stash's schema")

## Context

The adapter already exists, but it was built one need at a time. Spec 002 added the connection,
the server identifier, and read-only fixture capture. The spike (003) added test-only writes.
Spec 004 added the reads and writes that relations need. No part of it can yet read everything the
semantic graph is built from, which is what the rest of Phase 2 needs first.

This feature completes the adapter as the **library reader**. It is the single way the engine
learns what a Stash library holds:

- **Full read:** everything the semantic graph is built from, for a first build or a rebuild.
- **Change read:** what changed since a given moment, for keeping the graph current.
- **Id listing:** which items still exist, for detecting deletions.

It reads the data contract exactly as the plugin's design documents define it, and fails in ways
callers can tell apart.

**Sources (constitution Principle II).** The design documents of the semantic-tagging plugin, read
at commit `2c04141` (2026-10-24):

- `README.md` § Features
- `semantic-query-engine-roadmap.md`: "The semantic graph is the abstraction boundary" and its
  vocabulary corrections; Milestone 1D (Variations, Field Mapping)
- `archetype-system-plan.md`: Archetypes

The engine's vocabulary (`vocabulary/stash-mapping.ttl`, vocabulary 1.2.0) records each field.
Commit `2c04141` adds two keys the vocabulary does not yet map:

- **`fieldMapping`:** on Trait, Label, and Variation tags.
- **`archetypeGraphPosition`:** on Archetype performers, an `{x, y}` position saved when someone
  drags a node in the plugin's Hierarchy tab. Nodes nobody dragged have none and are laid out
  automatically. It is display data, not a semantic fact.

**Derived performer fields: a gap in the documents.** The plugin's code (`src/ui/matrix.js` at
`2c04141`, `parseMeasurements` and `DERIVED_PERFORMER_FIELDS`) derives four values from a
performer's free-text `measurements` field (for example `32D-24-34`):
- Bust Size
- Waist
- Hip
- Waist-to-Hip Ratio

Label field rules and the Matrix's advanced filters can name them (`__derived_bust`,
`__derived_waist`, `__derived_hip`, `__derived_whr`). Stash cannot filter on them, so the plugin
downloads every performer's measurements and computes the values itself. No design document
describes them. Labels use them, so the engine must resolve them (Principle III). This feature
adds them deliberately (Principle II), with vocabulary terms, and records the documentation gap
for the plugin.

## Clarifications

### Session 2026-10-24

- Q: Should this feature's reader read the performers' `measurements` text, so the engine can work
  out the derived fields later? → A: Yes, and derive them now (option B). It is a gap in the
  plugin's documents. Labels use the derived fields, so they need vocabulary terms and must be
  resolved by the engine.

## User Scenarios & Testing *(mandatory)*

The users of this feature are the engine's other parts: the graph builder, the change applier,
and the plugin's operations and tasks. Owners benefit indirectly. Builds are fast, and their
library is read only as far as needed. When something goes wrong, the message says what.

### User Story 1 - Read the whole semantic library for a build (Priority: P1)

The graph builder asks for the whole semantic library. The adapter returns:

- **Tags:** every semantic tag, with its flags, its semantic fields, and its parents.
- **Performers:** every performer that carries semantic data, with its tags and semantic fields.
- **Scenes and images:** every one that carries semantic data, with its performers, its tags, and
  its semantic fields.

It reads in pages and reports progress after each page. The builder can stop it between pages.

**Why this priority**: Every later Phase 2 item (the store, the vocabulary build, incremental
updates) needs a complete, correct read of the library. Without it nothing can be built.

**Independent Test**: Read the recorded test-instance library offline. Compare the result with the
expected records: every semantic tag, performer, scene, and image is present once, with every
documented field, and nothing outside the semantic data is fetched.

**Acceptance Scenarios**:

1. **Given** a library tagged by the plugin, **When** the builder asks for a full read, **Then**
   it receives every tag that has at least one semantic flag, with its flags, its semantic fields,
   and its parent tags. No other tag is returned.
2. **Given** that library, **When** the full read runs, **Then** every performer with a semantic
   tag or semantic field is returned. Each comes with:
   - its semantic tags (Traits, Labels, Variations)
   - its Archetype fields (`isArchetype`, `archetypeParents`), and its saved hierarchy position
     (`archetypeGraphPosition`) as display data
   - its derived measurements (Bust Size, Waist, Hip, Waist-to-Hip Ratio) when its
     `measurements` field can be parsed
3. **Given** a performer whose `measurements` field is set but who has no semantic data, **When**
   the full read runs, **Then** that performer is still returned, with its tags and derived
   measurements, because Label field rules can match any performer by them.
4. **Given** that library, **When** the full read runs, **Then** every scene and image that holds
   semantic data is returned. Each comes with:
   - its performer ids and native tag ids
   - each semantic field: role, action, pluggable, and relation assignments, the two pairing
     lists, automatic roles, and the timeline
   - its last-updated time
5. **Given** a library larger than one page, **When** the full read runs, **Then** progress is
   reported after each page as a share of the expected total.
6. **Given** a full read in progress, **When** the builder asks it to stop, **Then** it stops
   before the next page. It reports that it was stopped and returns no partial result as if it
   were complete.

---

### User Story 2 - Read what changed, and what still exists (Priority: P1)

The change applier keeps the graph current between builds. It asks the adapter for:

- the scenes, images, performers, and semantic tags changed since a given moment
- the records of named items, in batches
- the full list of ids of each kind, so items deleted while the engine was not watching can be
  found

**Why this priority**: The constitution's freshness rule (Principle IV) depends on these reads.
Hooks cover most saves. The catch-up after a gap and the deletion check use these reads. The
relations job of spec 004 already uses a narrow form of them.

**Independent Test**: With recorded fixtures, ask for changes since a moment. Check that every
item updated at or after it is returned, and none before. Ask for the id lists, and check them
against the recorded library.

**Acceptance Scenarios**:

1. **Given** a moment in the server's own time format, **When** the applier asks for changes
   since it, **Then** it receives the ids of every item of that kind updated at or after it.
2. **Given** a list of item ids of one kind, **When** the applier asks for their records, **Then**
   it receives the same record shape as a full read. Ids that no longer exist are reported as
   missing, not as an error.
3. **Given** a library, **When** the applier asks for every id of a kind, **Then** it receives
   all of them, with no other fields, so the deletion check stays cheap.
4. **Given** tags were merged in Stash, **When** the applier lists semantic tag ids, **Then** the
   merged-away tags are absent. Merges fire a hook only for the destination tag, so the id list is
   how the engine learns the source tags are gone.

---

### User Story 3 - Fail clearly (Priority: P2)

When Stash is unreachable, refuses the credentials, is older than the supported version, or
returns an error, the adapter reports which of these happened. Callers can then give the owner a
clear, structured error instead of a wrong or empty answer (Principle IV).

**Why this priority**: A silent empty read would make the engine build an empty graph and return
wrong answers, which the constitution forbids. The reads still work without this story, so it
comes after US1 and US2.

**Independent Test**: Run each read against recorded failures: refused connection, timeout,
rejected credentials, old server version, and a response with errors. Check that each gives its
own distinct failure kind and that no message contains a credential.

**Acceptance Scenarios**:

1. **Given** Stash is not reachable, **When** any read runs, **Then** it fails as "unreachable".
2. **Given** Stash rejects the session or API key, **When** any read runs, **Then** it fails as
   "not authorized", distinct from "unreachable".
3. **Given** a Stash server older than the minimum supported version (v0.31.1), **When** the
   adapter connects, **Then** it fails as "unsupported version" and names both versions.
4. **Given** a response that is not what the pinned schema promises, **When** a read runs,
   **Then** it fails as "unexpected response". It never returns a half-read record.
5. **Given** any failure, **When** its message is written to a log or an error, **Then** the
   message contains no cookie, API key, or other credential.

---

### User Story 4 - Test every read without a live Stash (Priority: P2)

A developer working on the graph builder runs the full test suite with no Stash server. Every read
in this feature has recorded fixtures. The capture tool can record new ones, read-only, from the
test instance. The builder can be tested against a stand-in reader with the same interface.

**Why this priority**: Constitution Principle VII requires tests that run without a live Stash.
The graph builder's tests depend on this.

**Independent Test**: Run the adapter's tests and the builder's tests with networking unavailable.
They pass. Run the capture tool against the test instance. It writes fixtures that contain no
credentials and performs no writes.

**Acceptance Scenarios**:

1. **Given** no Stash server, **When** the test suite runs, **Then** every read in this feature
   is exercised against recorded fixtures and passes.
2. **Given** the test instance, **When** the capture tool runs, **Then** it records fixtures for
   every read in this feature, sends only read requests, and writes no credential.
3. **Given** the graph builder, **When** it is tested, **Then** it can use a stand-in reader that
   serves records without any network.

---

### Edge Cases

- **Field that is not valid JSON:** a semantic field holds text that is not valid JSON, or JSON of
  the wrong shape (for example a list where a map is expected). The record is returned with that
  field marked unreadable, the raw text kept, and the others intact. The whole read does not fail.
- **Measurements that do not parse:** `measurements` is free text. A string that does not start
  with a band number (for example `natural` or `N/A`) gives no derived
  values; the performer is still returned with the raw text. A half-cup written `½`, `.5` or
  `1/2` reads as a half step, as in the plugin.
- **Flag spelling:** a flag is stored as `1`, `"1"`, `true`, or `"true"`. All four mean set; any
  other value means not set, as spec 004 settled.
- **Server-local time:** Stash stores `updated_at` as server-local text and compares it as text to
  the second (003 decision record). Change reads take and return times in that form. A UTC time is
  never sent as a threshold.
- **Edits during a full read:** an item edited after its page was read keeps its older record in
  this read. The change read catches it next time. The full read reports where it started
  (FR-007), so the caller can catch up from there.
- **Pages shifting under deletes:** if an item is deleted during a full read, items must not be
  skipped or repeated because the pages moved. Each item appears at most once.
- **Very large libraries:** a library of 500,000 scenes is read without holding more than one page
  of raw responses at a time.
- **Performers in scenes:** a performer who appears in a semantic scene but carries no semantic
  data of their own is still listed by id in that scene. Their record is not fetched.
- **Empty library:** a library with no semantic data gives an empty, successful result, not an
  error.
- **Timeouts:** a slow Stash response times out after a stated limit. Reads are retried a stated
  number of times for transient failures (unreachable, timeout), never for rejected credentials or
  errors in the response.

## Requirements *(mandatory)*

### Functional Requirements

**Full read**

- **FR-001**: The adapter MUST read every semantic tag. A semantic tag has at least one of the flags
  `plugtag`, `isAction`, `isRole`, `isTrait`, `isLabel`, `isMiseEnScene`, `isVariation`, or
  `isRelation`, or is a common variant of another tag (`variantOfTagId`), which may carry no flag
  of its own. Each tag MUST come with:
  - its name and parent tag ids
  - every semantic field the documents define: `categories`, `variations`, `variantOfTagId`,
    `variationData`, `labelFormula`, `fieldMapping`, `relationEnds`, and `relationRules`
- **FR-002**: The adapter MUST read every performer that carries semantic data. That means a
  semantic tag, one of `isArchetype` or `archetypeParents`, or a non-empty `measurements` field
  (FR-002b). Each performer MUST come with those fields and its tag ids. It MUST NOT read the
  retired keys `isGeneric` and `looks` (vocabulary 1.0.0, spec 001 FR-016), nor the plugin's
  application and cache keys (`archetypeRecipe`, `directTraitVariations`, `inheritedVariations`,
  `inheritedTraitVariations`). `measurements` is the only native performer field read. Other
  Label field rules are resolved by Stash's own performer filter, not by comparing field values
  (plan research R2).
- **FR-002b**: For each performer with a `measurements` field, the adapter MUST derive the four
  values the plugin defines in `src/ui/matrix.js` at `2c04141` (`parseMeasurements`), with the
  same results:
  - **Bust Size:** band, plus the cup's step on the half-cup scale AA to L times 2.5, plus 2 for
    each inch of waist under 28 when a waist is given. DD, DDD and DDDD read as E, F and G. An
    unknown cup counts as band alone.
  - **Waist** and **Hip:** the second and third numbers, when present.
  - **Waist-to-Hip Ratio:** waist divided by hip, rounded to 2 decimal places, when both are
    present.

  A value that cannot be derived is absent, never an error. A bust-only string such as `36DD`
  gives Bust Size alone. The raw text MUST be kept with the derived values, so the derivation can
  be checked and redone. The derived values MUST get vocabulary terms (FR-015), and the formula
  and its constants MUST be recorded as a deliberate addition with a note for the plugin's
  documents (Principle II).
- **FR-002a**: The adapter MUST read an Archetype's `archetypeGraphPosition` and return it
  separately from its semantic fields, marked as display data. The engine passes it to clients
  alongside the hierarchy so every client can draw the hand-placed layout. It MUST NOT become a
  fact in the semantic graph, and the engine MUST NOT compute positions for nodes that have none
  (Principle III: no UI behaviour in the engine). A missing or unreadable position means "not
  placed", never an error.
- **FR-003**: The adapter MUST read every scene and every image that holds at least one semantic
  field. The fields are `roleTagAssignments`, `actionAssignments`, `pluggableAssignments`,
  `actionPairings`, `roleTagPairings`, `relationAssignments`, `autoRoleAssignments`, and
  `timeline`. Each item MUST come with its performer ids, its native tag ids, and its last-updated
  time.
- **FR-004**: Each kind of record MUST be found with "field is not null" filters that Stash
  evaluates. The adapter MUST NOT load the whole tag, performer, scene, or image collection to
  filter it itself (Principle VII).
- **FR-005**: The adapter MUST read in pages of a stated size, and MUST NOT make one request per
  item (no N+1 requests). Records of named items MUST be fetched in batches.
- **FR-006**: A full read MUST report progress after each page. It MUST be stoppable between pages,
  and a stopped read MUST say it was stopped.
- **FR-007**: A full read MUST report where it started: the newest last-updated time of each kind,
  taken before its first page, in the server's own time format. A later change read starts from
  it. Stash offers no query for its current time, so the newest saved item stands in for it, as
  spec 004's watermark already does.
- **FR-008**: A full read MUST return each item at most once, even if items are added or deleted
  while it runs.

**Change and id reads**

- **FR-009**: The adapter MUST return the ids of scenes, images, performers, and semantic tags
  updated at or after a given server time. The time is given and compared in the server's own text
  form.
- **FR-010**: The adapter MUST return the records of a given list of ids for each kind. The records
  MUST have the same shape as a full read, and ids that no longer exist MUST be listed as missing.
- **FR-011**: The adapter MUST return the complete list of ids of each kind, with no other fields,
  for the deletion check. That includes semantic tags, so merged-away tags can be detected.
- **FR-012**: The adapter MUST return the latest last-updated time of each kind of item, for the
  catch-up watermark that spec 004 already keeps.

**Reading the data as documented**

- **FR-013**: Semantic fields stored as JSON text MUST be parsed into structured values. A field
  that cannot be parsed MUST be marked unreadable with its raw text kept. The rest of the record,
  and the read, MUST continue.
- **FR-014**: Flags MUST be read with spec 004's rule: `1`, `"1"`, `true`, and `"true"` mean set,
  and anything else means not set.
- **FR-015**: Every field read MUST be one the design documents (at the commit cited above) or the
  engine's vocabulary define. A field added here that the vocabulary does not yet map MUST be
  added to the vocabulary in the same feature, or listed as a known gap for the plugin.

**Failures**

- **FR-016**: Failures MUST be reported as distinct kinds: unreachable, timed out, not authorized,
  unsupported Stash version, an error reported by Stash, and unexpected response.
- **FR-017**: The adapter MUST check the server's version before reading. It MUST refuse a server
  older than the minimum supported version, and the failure MUST name both versions.
- **FR-018**: Transient failures (unreachable, timed out) MUST be retried a stated number of times
  with a stated wait. Other failures MUST NOT be retried.
- **FR-019**: No failure message, log line, or fixture MUST contain a cookie, API key, or other
  credential (Principle IV).

**Boundaries**

- **FR-020**: The adapter MUST remain the only code that talks to Stash (Principle V). Every
  request MUST be a typed operation checked against the pinned schema, and MUST select only the
  fields its caller needs (constitution, Technology & Platform Constraints).
- **FR-021**: The reads that spec 004 and the fixture tool already use MUST be served by this
  feature's reader, or kept to the same rules, so there is one way to read each kind of record.
  Spec 004's behaviour MUST NOT change.
- **FR-022**: This feature MUST NOT add any write to Stash. The shipped plugin's writes stay those
  spec 004 and spec 002 defined; the spike's writes stay test-only.
- **FR-023**: The graph builder and other callers MUST be able to use a stand-in reader with the
  same interface, so they can be tested without a network.

**Tests and fixtures**

- **FR-024**: Every read MUST have recorded fixtures and tests that run without a live Stash.
- **FR-025**: The capture tool MUST be able to record fixtures for every read, read-only, from the
  test instance, and the recorded fixtures MUST contain no credentials.

### Key Entities

- **Semantic tag record**: one tag with at least one semantic flag. It holds the id, name, set
  flags, parent ids, and parsed semantic fields, with any unreadable field kept as raw text.
- **Performer record**: one performer with semantic data. It holds the id, tag ids, the Archetype
  fields, and the raw `measurements` text with its derived Bust Size, Waist, Hip and
  Waist-to-Hip Ratio. For an Archetype it also holds the saved hierarchy position, if any, kept
  apart as display data.
- **Media record**: one scene or image with semantic data. It holds the kind, id, performer ids,
  native tag ids, parsed semantic fields, and last-updated time.
- **Library read**: the result of a full read. It holds the tag, performer, and media records,
  and where the read started (the newest last-updated time of each kind). It is complete, or
  marked stopped.
- **Change set**: the ids of each kind updated at or after a server time.
- **Id list**: every id of one kind, for the deletion check.
- **Read failure**: the kind of failure, a message without credentials, and for a version failure
  both versions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A full read of the recorded test-instance library returns 100% of its semantic tags,
  performers, scenes, and images, each exactly once, with every documented field. It returns 0
  records without semantic data.
- **SC-002**: A full read of a Production-size library (about 50,000 scenes and 29,000 images)
  finishes in under 2 minutes, and never takes more than 5 minutes (003 decision record: first
  build budget). It is measured by a repeatable command.
- **SC-003**: Listing every id of every kind in a Production-size library takes under 5 seconds
  (003 estimate: 2.1 s).
- **SC-004**: A change read after a single save returns that item and takes under 100 ms on the
  test instance.
- **SC-005**: Memory held by the adapter during a full read stays below a stated bound that does
  not grow with library size beyond the records it returns.
- **SC-006**: Each of the six failure kinds is produced by its recorded case, and 0 of them is
  reported as another kind or as an empty success.
- **SC-007**: 0 credentials appear in any failure message, log line, or fixture produced by the
  test suite and the capture tool.
- **SC-008**: The full test suite passes with networking disabled, and spec 004's tests pass
  unchanged.
- **SC-009**: Stopping a full read takes effect within one page. The caller learns it was stopped
  in 100% of cases.
- **SC-010**: For every `measurements` string in a parity set of at least 30 cases (taken from the
  test instance and from the plugin's edge cases: bust-only, half cups, DD to DDDD, unknown cups,
  unparseable text), the four derived values equal what the plugin's `parseMeasurements` gives,
  in 100% of cases.

## Assumptions

- **Minimum version:** v0.31.1, the version the schema is pinned to (constitution).
- **Native performer fields:** only `measurements` is read, for the derived fields (FR-002b). At
  `2c04141` the plugin resolves every other Label field rule with a Stash performer filter query
  (`labels.js`, `resolveFieldRuleMatchSets`), and Field Mapping only writes fields. Resolving
  those rules is Label evaluation (ROADMAP Phase 5), which adds a "performers matching this
  criterion" read then.
- **Derivation constants:** the plugin calls its constants "a starting point, not a settled
  value". The engine copies them exactly as at `2c04141`. If the plugin changes them, its
  documents record the change, and the engine follows in a vocabulary update.
- **Page size:** the page and batch sizes follow what spec 004 measured (batches of 100). The plan
  may tune them against SC-002.
- **Retries:** a small number of retries (for example 2) with a short wait is enough for a local
  Stash. The plan states the exact values.
- **Timeout:** the existing 60-second request timeout is kept unless the plan measures a reason to
  change it.
- **`archetypeGraphPosition`:** it is read and passed through as display data (FR-002a), not as
  part of the semantic graph. The vocabulary records it as a display-only key. How the engine
  hands it to clients (a field on the hierarchy answer, for example) is decided by the operation
  that serves the hierarchy, in a later feature. Clients still lay out nodes without a saved
  position themselves.
- **`fieldMapping`:** it is semantic. It is read, and added to the vocabulary as part of this
  feature (FR-015).
- **Out of scope:** this feature does not build the graph or the store. Those are the next Phase 2
  items. It does not add writes or change hooks, and it does not change any client.
- **Cancellation:** the caller signals a stop through a flag the reader checks between pages.
  Stash's own job-stop signal is wired to it by the task that runs the build, in a later feature.
