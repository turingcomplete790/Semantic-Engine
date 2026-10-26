# Feature Specification: The Semantic Graph from the Documents

**Feature Branch**: `master` (spec directory `007-graph-build`)

**Created**: 2026-10-25

**Status**: Draft

**Input**: User description: "Phase 2: Vocabulary from Documents." This covers three Phase 2
items of the roadmap:
- **Vocabulary from the documents:** Scene and Image media entities, performers, the six
  flagged tag types plus Variations, reified Role and Action instances, tag categories and
  variations, parent tags, Label formulas, and archetype parents.
- **Data contract read exactly as documented.**
- **Compatibility fixtures:** a plugin-tagged library produces the documented graph.

## Context

The engine now has its two halves:
- **The library reader (spec 005):** it returns every semantic tag, performer, scene and image
  of a Stash library as records.
- **The graph store (spec 006):** it holds facts, publishes them as generations, and answers
  SPARQL `SELECT` queries.

What is missing is the step between them: turning the library's records into the semantic graph
the plugin's design documents describe. This feature builds the graph.

**The vocabulary defines the graph.** `vocabulary/` (version 1.3.0) already defines every term,
and records, for each Stash key, which terms it produces (`stash-mapping.ttl`). This feature
applies those mappings. It does not invent new meanings (Principle II). Where a document and the
vocabulary disagree, the documents win, and the vocabulary is corrected in the same feature.

**Sources (Principle II).** The semantic-tagging plugin's design documents, read at commit
`2c04141` (2026-10-24):
- `semantic-query-engine-roadmap.md`: "The semantic graph is the abstraction boundary", its
  vocabulary corrections, Milestone 1B (Archetypes) and Milestone 1D (Variations, Field Mapping);
- `README.md` § Features;
- `archetype-system-plan.md` § Data model.

The vocabulary's own additions are deliberate and documented: Relations (1.2.0), Field Mapping
and derived measurements (1.3.0).

**What the build does.** A build reads the whole library, maps every record, writes the facts to
the store, and publishes one generation. It runs as a Stash background task the owner can start,
watch and stop. Keeping the graph current between builds, from save hooks, is the next Phase 2
item. Until then, a build is how the graph is made and refreshed.

## User Scenarios & Testing *(mandatory)*

The users of this feature are:
- **The owner:** runs the build task and sees its progress and outcome in Stash's job list.
- **The engine's query layer (Phase 3):** reads the published graph.
- **Developers:** check the graph against compatibility fixtures.

### User Story 1 - Every documented fact is in the graph (Priority: P1)

Given the records of a library, the builder produces the facts the vocabulary's mappings define.

- **Media:** each scene and image becomes a media entity. Its performers are linked to it.
- **Role, Action and Pluggable assignments:** each becomes its own instance node, tied to its
  media, its performer and its tag. Action variations are kept. These facts are scene-scoped, so
  a performer's role in one scene never leaks to another.
- **Pairings, relations and the timeline:** pairings, relation instances, the timeline's slots
  and stages, and Mise-en-scène picks become their documented structures.
- **Tags:** each tag gets its type from its flags, along with its categories, its declared
  variations, its parent tags (and broader roles and relations), the variant link of a common
  variant, its Label formula, its relation ends and rules, and its Field Mapping slots.
- **Performers:** each gets its Traits, Labels and Variations, its Archetype type and parents,
  and its derived measurements.

**Why this priority**: This is the graph itself. Every semantic query of Phase 3 depends on it
being complete and correct.

**Independent Test**: Map a fixture library that uses every mapping entry. Compare the facts with
a recorded expected graph: no fact missing, none extra.

**Acceptance Scenarios**:

1. **Given** a scene whose role assignments give performer 7 the Role Dom, **When** it is mapped,
   **Then** the graph holds:
   - a role instance for (that scene, performer 7, Dom), with its media, performer and tag;
   - the performer's link to the instance;
   - the scene's link to performer 7.
2. **Given** two scenes where performer 7 is Dom in one and Sub in the other, **When** both are
   mapped, **Then** a query joining role instances within one scene never pairs Dom with the
   other scene. Every scene-scoped fact goes through an instance node of its own media.
3. **Given** an action assignment with a variation, **When** it is mapped, **Then** the action
   instance carries that variation.
4. **Given** a tag with `isRole` set and a parent that is also a Role, **When** it is mapped,
   **Then** the tag is typed Role, has its parent tag, and has the parent as a broader role.
5. **Given** a Label tag with a formula, **When** it is mapped, **Then** the graph holds the
   Label's threshold and one rule node per rule, by the rule's own id. A field rule naming
   `__derived_bust` resolves to the bust-size property.
6. **Given** a performer with a Trait tag, a Label tag, measurements `34D-26-35`, and Archetype
   parents, **When** it is mapped, **Then** the graph holds the Trait, the Label, the four
   derived measurement values, the Archetype type, and its parent links.
7. **Given** any record, **When** it is mapped, **Then** every item's identifier follows the
   vocabulary's item-IRI scheme, and includes this server's id.

---

### User Story 2 - Build and publish the graph from Stash (Priority: P1)

The owner runs the "Build semantic graph" task from Stash's Tasks page. The task:
1. reads the whole library through the reader;
2. maps it;
3. writes the result to the store, replacing what was there;
4. publishes a new generation.

The job's progress moves from 0 to 100%. When the job ends, its log line says how many records
and facts were built, how many fields were skipped and why, and how long it took. Stopping the
job leaves the previous graph in place.

**Why this priority**: Without a build, the graph exists only in tests. This is the first time
the engine holds a real library's graph.

**Independent Test**: Against a recorded library, with a stand-in reader, run the build and check
that a generation is published with the expected fact count. Stop a build partway, and check that
the previous generation is still current. On the test instance, run the task from Stash and query
the published graph.

**Acceptance Scenarios**:

1. **Given** a store with no graph yet, **When** the build runs, **Then** generation 1 is
   published with the whole library's facts, and the job reports success.
2. **Given** a published graph, **When** the library changes and the build runs again, **Then**
   the new generation holds exactly the new library's facts. Facts of deleted items are gone,
   because a build replaces the graph rather than adding to it.
3. **Given** a running build, **When** the owner stops the job, **Then** no new generation is
   published. Readers keep the previous one, and the job reports that it was stopped.
4. **Given** a store that needs a rebuild (missing, damaged, another format, or another server),
   **When** the build runs, **Then** it clears the store first, then builds. The log says why it
   cleared.
5. **Given** a build already running, **When** another build is started, **Then** the second one
   ends at once, saying a build is in progress. It changes nothing.
6. **Given** the plugin has no server id yet, **When** the build runs, **Then** it ends at once
   and tells the owner to run "Generate server identifier" first.
7. **Given** Stash is unreachable or refuses the credentials, **When** the build runs, **Then**
   it ends with that failure named, and the previous graph stays current.

---

### User Story 3 - Bad data never breaks the build (Priority: P2)

Libraries contain fields the plugin wrote long ago, fields edited by hand, and fields the reader
could not parse. The build does not stop at any of them. It maps everything it can, and skips
what it cannot, field by field. For each skipped field it notes the item, the key and the reason.
The job log summarises the notes, and the full list goes to the plugin's log.

**Why this priority**: One malformed field in a library of 80,000 items must not leave the owner
without a graph. Silently dropping data, though, would hide problems the plugin should fix.

**Independent Test**: Map a fixture with each kind of bad field. Check that the rest of each
record is mapped, that each bad field is noted once with its reason, and that nothing about it
reaches the graph.

**Acceptance Scenarios**:

1. **Given** a scene whose role assignments are unreadable, **When** it is mapped, **Then** its
   other fields are mapped, and the role assignments are noted as skipped (unreadable).
2. **Given** an assignment naming a tag or performer id that is not a valid Stash id, **When** it
   is mapped, **Then** that one entry is skipped and noted. The other entries of the field are
   mapped.
3. **Given** a timeline entry of a kind the documents do not define, **When** it is mapped,
   **Then** that entry is skipped and noted. The rest of the timeline is mapped.
4. **Given** a role assignment naming a tag that is not a Role, or a tag that does not exist,
   **When** it is mapped, **Then** the instance is still built, because the plugin's data says
   it, and the mismatch is noted. Judging it is a query's job, not the builder's.

---

### User Story 4 - The graph matches the documents, checked by fixtures (Priority: P2)

Compatibility fixtures prove that a plugin-tagged library produces the documented graph:
- a hand-written library that uses every mapping entry at least once, with its expected graph
  written out in full;
- the test instance's captured library, with checks on counts and on facts chosen from the
  documents' own examples.

The tests run without Stash.

**Why this priority**: Constitution Principle VII requires compatibility fixtures, and the
roadmap names them as a Phase 2 item. They are what makes "the graph matches the documents"
checkable, rather than a claim.

**Independent Test**: Run the fixture tests offline. They pass. Change one mapping on purpose,
and they fail, naming the fact that differs.

**Acceptance Scenarios**:

1. **Given** the hand-written fixture library, **When** it is mapped, **Then** the facts equal
   the expected graph exactly. A difference is reported as the facts missing and the facts extra.
2. **Given** the list of active mapping entries in `stash-mapping.ttl`, **When** the fixture
   tests run, **Then** every active entry is exercised by at least one fixture fact. An entry
   with no fixture fact fails the test, naming the entry.
3. **Given** the captured test-instance library, **When** it is mapped, **Then**:
   - the counts of media entities, performers and tags match the library's;
   - every role, action and pluggable instance has its media, performer and tag;
   - no fact uses a term the vocabulary does not define.

---

### Edge Cases

- **Records the reader returns with no semantic fields:** an item whose fields are all unreadable
  still becomes a media entity, with its performer links. Only the bad fields are skipped.
- **Performers referenced but not read:** an assignment can name a performer the reader did not
  return, because that performer has no semantic data of their own (spec 005). The performer still
  gets its IRI and its links from the assignment. No performer facts are invented for them.
- **The same performer in many scenes:** each scene's role, action and pluggable instances are
  separate nodes. Performer-level facts (Traits, Labels, measurements) are written once per
  performer, not once per scene.
- **Retired keys:** keys the vocabulary retired (`isGeneric`, `looks`, and Mise-en-scène refs in
  the old per-slot form) are never mapped. The reader does not return them; if one appears, it
  is noted.
- **Display data:** `archetypeGraphPosition` is not a graph fact (spec 005 FR-002a). The build
  does not write it into the graph.
- **Very large libraries:** a library with 79,000 scenes and images, all semantic, builds without
  holding more than the reader's records and one batch of facts in memory at once.
- **Ids that are not valid in an IRI:** an id that the item-IRI scheme refuses (spaces, slashes)
  skips that item or entry, and is noted. It never stops the build.
- **Empty library:** a library with no semantic data builds an empty graph and publishes it. That
  is a success, distinct from "not built yet".
- **Duplicate entries:** the same role listed twice for one performer in one scene gives one
  instance, not two.

## Requirements *(mandatory)*

### Functional Requirements

**Mapping (the graph)**

- **FR-001**: The builder MUST apply every active mapping entry of `vocabulary/stash-mapping.ttl`
  at vocabulary 1.3.0, as corrected by this feature (1.4.0, plan research R6). Each MUST produce exactly the terms the entry and its terms' definitions
  name (Principle II).
- **FR-002**: Every scene and image the reader returns MUST become a media entity of its class,
  linked to its performers.
- **FR-003**: Role, Action and Pluggable assignments MUST become reified instances, one per
  (media, performer, tag). Each MUST be tied to its media, its performer and its tag, and the
  performer MUST be linked to it. Action instances MUST carry their variations. No scene-scoped
  fact MAY be written as a direct performer-to-tag link (the documents' vocabulary correction on
  scene scope).
- **FR-004**: Action and role pairings, relation instances (including automatic ones and the rule
  that applied them), and automatic roles MUST be mapped as their terms define.
- **FR-005**: The timeline MUST be mapped into its slots, stages, stage picks and Mise-en-scène
  instances, in its stored order, as `timeline.ttl` defines.
- **FR-006**: Tags MUST get their type from each set flag, and their categories and declared
  variations. They MUST get their parent tags; a parent of the same type also gives a broader role
  or broader relation. They MUST also get their variant link, their Label formula (threshold and
  rules, by rule id), their relation ends and rules, and their Field Mapping slots.
- **FR-007**: Performers MUST get a Trait, Label or Variation link for each tag of that type they
  carry. They MUST also get their Archetype type and parents, and the four derived measurement
  values where the measurements give them.
- **FR-008**: Every item IRI MUST follow the vocabulary's item-IRI scheme (`semantic-vocabulary`'s
  scheme, spec 001) and include this server's id.
- **FR-009**: The builder MUST NOT write facts for retired keys, for display data, or for any term
  the vocabulary does not define.
- **FR-010**: Mapping the same records twice MUST give the same facts. Duplicate entries within a
  record MUST give one fact.

**Skipped data**

- **FR-011**: A field the reader marked unreadable, an entry with an invalid id, an entry of an
  undocumented kind, or an id refused by the item-IRI scheme MUST be skipped on its own. Each
  skip MUST be recorded as a note of item, key and reason. The rest of the record and of the
  build MUST continue.
- **FR-012**: An entry whose tag is missing, or of an unexpected type, MUST still be mapped as the
  data says, and noted as a mismatch.

**The build task**

- **FR-013**: The plugin MUST offer a Stash task, "Build semantic graph". It reads the whole
  library through the reader, maps it, replaces the store's working graph with the result, and
  publishes one new generation.
- **FR-014**: The build MUST report progress to Stash as it reads, maps and writes. It MUST stop
  when the owner stops the job. A stopped or failed build MUST publish nothing, and the previous
  generation MUST stay current.
- **FR-015**: If the store needs a rebuild, the build MUST clear it first and say why in the job
  log. If another writer holds the store, the build MUST end at once, saying so.
- **FR-016**: If the plugin has no server id, the build MUST end at once and tell the owner to run
  "Generate server identifier".
- **FR-017**: When it ends, the build MUST log one summary line: records read, facts written, the
  generation published, skipped fields by reason, and time taken. Every note MUST go to the
  plugin's log, capped at a stated number, with the rest counted. No credential and no
  performer name may appear in any log.
- **FR-018**: The operations contract MUST record the new task, as an additive change.

**Fixtures and checks**

- **FR-019**: A hand-written fixture library MUST exercise every active mapping entry. Its full
  expected graph MUST be recorded and compared exactly.
- **FR-020**: A test MUST fail, naming the entry, when an active mapping entry has no fixture fact.
- **FR-021**: The captured test-instance library MUST be mapped in a test, with checks on counts,
  on instance completeness, and that every fact uses a defined term.
- **FR-022**: All of these tests MUST run without Stash and without a network.

**Budgets (Principle VII)**

- **FR-023**: The build MUST state and measure its time and peak memory, by a repeatable command:
  on the test instance; on the Production library, read-only through the owner; and on a
  synthetic, fully tagged library of Production size.

### Key Entities

- **Library records**: the reader's full read, with tags, performers, scenes, images, and where
  the read started (spec 005).
- **Fact**: one statement in the graph (spec 006).
- **Mapping entry**: one row of `stash-mapping.ttl`: a Stash key, its allowed values, and the
  terms it produces.
- **Item IRI**: the identifier of a scene, image, performer, tag or instance, built from the
  server id by the vocabulary's scheme.
- **Build note**: one skipped or mismatched field, holding the item kind and id, the key, and the
  reason.
- **Build report**: what a build did. It holds the counts, the generation published, the notes
  by reason, the time, and the outcome (published, stopped, or failed with its reason).
- **Compatibility fixture**: a recorded library and its expected graph.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The hand-written fixture library's graph equals its expected graph exactly: 0
  missing facts, 0 extra facts.
- **SC-002**: 100% of the active mapping entries in `stash-mapping.ttl` are exercised by at least
  one fixture fact.
- **SC-003**: Mapping the captured test-instance library gives:
  - media, performer and tag counts equal to the library's;
  - 100% of instances complete, with media, performer and tag;
  - 0 facts using an undefined term.
- **SC-004**: A build of the Production library completes in under 5 minutes, end to end from
  Stash (003 decision record: the first-build budget). A synthetic, fully tagged library of
  Production size (about 50,000 scenes and 29,000 images) also builds in under 5 minutes.
- **SC-005**: A build stopped by the owner publishes nothing in 100% of trials, and the previous
  generation stays current.
- **SC-006**: A fixture with one bad field of each kind builds successfully. It notes each bad
  field exactly once, and maps every other field of the same records.
- **SC-007**: Mapping the same library twice gives identical fact sets.
- **SC-008**: After a build on the test instance, a SPARQL query that finds "scenes where one
  performer has a given Role and another has a given Action" returns exactly the scenes the
  plugin's own data says.

## Assumptions

- **Vocabulary:** 1.3.0, as released by spec 005. If mapping shows a vocabulary term that does
  not fit the documents, this feature corrects the vocabulary as a PATCH or MINOR release and
  records why.
- **The reader:** the build reads through spec 005's `LibraryReader`, with its stop flag wired to
  Stash's job stop. Records come from one full read, so a build sees one consistent library.
- **The store:** the build writes through spec 006's `engine::graph`. It clears the working
  graph, adds the facts in batches, and publishes once.
- **Scope of a build:** a full rebuild every time. Incremental updates from hooks, the change
  journal for the graph, and catch-up are the next Phase 2 item.
- **Display data:** the build does not store `archetypeGraphPosition`. The Phase 3 operation that
  serves the Archetype hierarchy decides how to pass it to clients (spec 005 Assumptions).
- **Notes:** at most 1,000 notes are written to the log per build, and the rest are counted. The
  plan may change the cap.
- **No new operation:** queries for clients are Phase 3. The new task is the only addition to the
  contract. It moves to 1.2.0, a MINOR, additive change.
- **Mismatches:** the builder records what the data says. Deciding that a "role" pointing at a
  non-Role tag is wrong is left to queries and to the plugin. The notes make such cases visible.
- **Production measurement:** the build reads Production read-only. Writing the graph into the
  engine's own folder under Production's plugin directory happens only if the owner runs the
  task there. Otherwise the Production figure comes from a read-only run of the build against a
  local store, run by the owner, as 005's bench did.
