# Feature Specification: Query Operations

**Feature Branch**: `master` (spec directory `009-query-operations`)

**Created**: 2026-10-27

**Status**: Draft

**Input**: User description: "phase 3". The roadmap's Phase 3, "Query operations (Milestone 2)":
- a versioned operation protocol: request, response, structured errors;
- SPARQL `SELECT`: triple patterns, variables, joins, projection, basic filters;
- Relation and Role criteria that accept `recursive` and expand through the engine's hierarchy,
  verifying 004 SC-004 and SC-007;
- scoping rules enforced: scene-scoped predicates only join within one scene;
- paging and result-size limits: 1 MiB pages, never over 2 MiB;
- hook-driven incremental updates, with a manual rebuild task;
- a status operation: engine version, store format, last update, graph counts.

## Context

Phase 2 built the semantic graph and keeps it current when the owner runs "Update semantic
graph". Nothing can ask it anything yet. This feature lets clients query it, through Stash, and
keeps it current on its own.

**The users:**
- **The clients:** the Semantic Stash Viewer and the semantic-tagging plugin, with its
  semantic-search plugin. They send queries through Stash's `runPluginOperation` and show the
  answers (constitution Principles I and III). Phase 4 moves them onto the engine; this feature
  gives them what to move onto.
- **The owner:** saves in Stash as usual, and expects the answers to follow without running a
  task.

**The query language** is the documents' SPARQL-inspired one, at their Phase 1 level: `SELECT`,
`WHERE`, triple patterns, variables, constants, joins and projection, plus basic filters
(Milestone 2). Boolean logic and further SPARQL constructs are the roadmap's Phase 5.

**Sources** (constitution Principle II; semantic-tagging read at commit `3aab0be`, 2026-10-24):
- `semantic-query-engine-roadmap.md`:
  - § Query Language Direction (basic query, multiple relationships, multiple bound performers);
  - § Query Features, Phase 1 (minimal) and Phase 2 (boolean logic; only comparisons and
    inequality are taken here);
  - § Core Design Decisions 2, "The semantic graph is the abstraction boundary", and its
    Vocabulary corrections: `hasRole`, `hasAction` and `hasPluggable` are scene-scoped, and
    "this is a modeling rule the evaluator must enforce through how it joins; the query syntax
    itself stays exactly as simple as the examples";
  - § Step 7 (what a query UI shows: syntax errors, execution errors, result count, bindings);
  - § Milestone 2.
- The 003 decision record: rules 2 (readers), 4 and 5 (hooks), 6 as superseded by spec 004
  research R6 (the hook starts a background job), 9 (paging), 10 (no snapshots to clients), 11
  (hook budget) and 12 (query budget); § 3 Freshness (merges fire only on the destination).
- Spec 004: User Story 3 (recursive search), SC-004 and SC-007, and its save hook and journal.
- Specs 006 to 008: the store's generations, the graph's mapping, and the update task.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Ask the graph a question (Priority: P1)

A client sends a `SELECT` query through Stash and gets back the matching bindings, for example
every scene with a performer who holds the Role Dom:

```sparql
SELECT ?scene WHERE {
  ?scene :hasPerformer ?person .
  ?person :hasRole :Dom .
}
```

The answer lists the variables and, for each binding, the value of each variable. An item in an
answer (a scene, image, performer or tag) also gives its kind and Stash id, so the client can
open it in Stash without parsing identifiers. The answer says which generation of the graph it
came from, when that generation was published, and whether saves are still waiting to reach it.

A query with a mistake gets a structured error naming what is wrong and where in the query
text, not free text.

**Why this priority**: This is what the engine is for. Every semantic feature of the clients
becomes a query (Principle III).

**Independent Test**: Build the graph from the compatibility fixture, run a set of queries with
known answers, and compare each answer with the expected bindings exactly.

**Acceptance Scenarios**:

1. **Given** the fixture graph, **When** a client queries for scenes with a performer holding a
   given Role, **Then** the answer holds exactly the scenes the fixture's expected graph gives.
2. **Given** a query that joins two performers of one scene and filters `?a != ?b`, **When** it
   runs, **Then** no binding has the same performer twice.
3. **Given** a query that compares two times (`FILTER (?t1 < ?t2)`), **When** it runs, **Then**
   only bindings where the first time is earlier remain.
4. **Given** a query with a syntax error, **When** it runs, **Then** the error gives a code, a
   message, and the line and column of the mistake.
5. **Given** a query using a construct outside this feature's language (for example `OPTIONAL`
   or `UNION`), **When** it runs, **Then** it is refused with an error naming the construct and
   its position, and nothing is evaluated.
6. **Given** a query naming a tag that is not in the graph, **When** it runs, **Then** the
   error names that tag and its position.
7. **Given** no graph built yet, **When** a client queries, **Then** it gets an error saying
   the graph is not built, and what to run.

---

### User Story 2 - Scene-scoped facts only join within one scene (Priority: P1)

Roles, Actions and Pluggables hold for a performer within one scene or image. A performer can
be Dom in one scene and Sub in another. A query in the documents' short form, such as the one
in User Story 1, matches only when the performer holds the Role in that same scene. The client
writes the query as simply as the documents show; the engine ties each scene-scoped fact to the
scene or image the query links that performer to. Traits and Labels are global, and hold in
every scene.

A query may also use the graph's own shape, with the instance nodes, `inMedia` and `ofTag`
written out. Both forms give the same answers.

**Why this priority**: Without it, the short form gives false matches: two performers who were
Dom and Sub in scenes they never shared. That is the false positive the plugin's Role Matrix was
written to avoid.

**Independent Test**: On a fixture where performer A is Dom in scene 1 and Sub in scene 2, and
performer B the reverse, query for scenes with a Dom and a Sub. Only scenes where each holds the
Role in that scene match.

**Acceptance Scenarios**:

1. **Given** performer A, Dom in scene 1 only, and also in scene 2 without a Role there,
   **When** a client queries for scenes with a performer holding Dom, **Then** scene 1 matches
   and scene 2 does not.
2. **Given** the documents' "multiple bound performers" query, **When** it runs on the
   fixture, **Then** every binding's two performers hold their Roles in the bound scene.
3. **Given** a query mixing a scene-scoped fact (a Role) and a global one (a Trait) on one
   performer, **When** it runs, **Then** the Role must hold in the bound scene, and the Trait
   anywhere.
4. **Given** the same question in the short form and in the graph's own form, **When** both
   run, **Then** their answers are equal.
5. **Given** a short-form query that links one performer variable to two different scene
   variables, and also gives it a scene-scoped fact, **When** it runs, **Then** it is refused
   with an error saying the scope is ambiguous, at that fact's position.

---

### User Story 3 - The graph follows saves on its own (Priority: P1)

The owner saves in Stash as usual: tags, performers, scenes and images, created, edited,
deleted or merged. Each save is recorded by a hook, and the first save of a burst starts one
"Update semantic graph" job in Stash's queue. Within seconds the change shows in query answers,
without the owner running a task and without any client call waiting for it. "Build semantic
graph" stays as the manual full rebuild.

**Why this priority**: Answers that lag the library until someone runs a task are wrong
answers. Principle IV requires hooks to keep the graph current.

**Independent Test**: On the test instance, save one item of each kind in Stash, wait, and
query: each change shows. Delete and merge tags and check that they leave the answers. Time
saves with and without the hook.

**Acceptance Scenarios**:

1. **Given** the plugin installed and the graph built, **When** the owner edits a scene's Roles
   in Stash, **Then** an update job starts by itself, and a query after it ends gives the new
   answer.
2. **Given** a bulk edit of 1,000 scenes, **When** it saves, **Then** one update job (or a few)
   runs, not 1,000, and every change shows after it.
3. **Given** a tag merged into another, **When** the update runs, **Then** the merged-away
   tag is gone from answers (only the destination fires a hook; the update's id comparison finds
   the source gone).
4. **Given** saves made while the plugin was disabled, **When** the next save or update runs,
   **Then** those changes are caught up too (spec 008).
5. **Given** the hook's work, **When** a single item is saved, **Then** the save takes at most
   7 ms longer than without the plugin, counting every hook the plugin adds, including the
   relations hook of spec 004.
6. **Given** an update job already waiting or running, **When** more saves arrive, **Then**
   they are caught by that job or by one more, never lost.

---

### User Story 4 - Recursive Role and Relation criteria (Priority: P2)

A query can mark a Role or Relation criterion as recursive, so narrower Roles or Relations
also match: a recursive search for the Relation Full Service also finds Extra Care, which is
narrower. Without recursive, only the exact tag matches. "Narrower" means the same hierarchy the
`relations` operation reports (spec 004), with its loops ignored.

**Why this priority**: The hierarchy is only useful in search if recursion works, but plain
queries deliver value first.

**Independent Test**: On the library of 004 SC-004, query Full Service recursively and not.
Recursive returns exactly the scenes with Full Service or Extra Care; non-recursive exactly the
Full Service scenes.

**Acceptance Scenarios**:

1. **Given** scenes with Full Service and scenes with Extra Care, **When** a client queries Full
   Service recursively, **Then** both kinds are returned, and no other scene.
2. **Given** the same scenes, **When** the query is not recursive, **Then** only Full Service
   scenes are returned.
3. **Given** performers with the Role Patient, which is narrower than Customer, **When** a
   client queries Customer recursively, **Then** scenes with those Patients match.
4. **Given** a Role hierarchy with a loop, **When** a recursive query names a Role in it,
   **Then** it answers as the `relations` operation's hierarchy does, without the looping
   links, and does not fail.

---

### User Story 5 - Large answers come in pages (Priority: P2)

An answer larger than one page comes back as a first page and a continuation. The client asks
for the next page with the continuation until there is none. Every page is at most 1 MiB, and no
single response ever exceeds 2 MiB. All pages of one answer come from the same generation, in one stable
order, so no binding is missed or repeated across pages, even when an update publishes a new
generation in between. A client may also ask for fewer bindings per page, or for a total limit.

**Why this priority**: Answers pass through Stash, and the 003 spike measured that an unpaged
16 MiB answer takes 1.2 s. Most semantic queries return far less than a page, so this comes
second.

**Independent Test**: Query a large synthetic graph for every scene. Read every page, and check
that their union equals the unpaged answer with no repeats, that each page is within 1 MiB, and
that publishing a generation between pages changes nothing.

**Acceptance Scenarios**:

1. **Given** an answer of 30,000 bindings, **When** a client reads it page by page, **Then**
   every page is at most 1 MiB, and together they hold every binding once.
2. **Given** a continuation, **When** a new generation was published after the first page,
   **Then** the next page still comes from the first page's generation.
3. **Given** a continuation whose generation has since been retired, **When** the client asks
   for the next page, **Then** it gets an error saying the answer expired and to run the query
   again, never a page from another generation.
4. **Given** a continuation that was altered or belongs to another query, **When** it is sent,
   **Then** it is refused with a structured error.

---

### User Story 6 - Ask the engine how it is (Priority: P3)

The `status` operation also reports the graph: the store format, the current generation and
when it was published, how it was made (build or update) and when, the counts of facts and of
items by kind, and whether saves are waiting to reach it. A client shows this when answers look
stale, and the owner checks it after a save.

**Why this priority**: Useful for trust and support, but queries already say which generation
answered them.

**Independent Test**: Build, update and save on the test instance, and check after each step
that `status` reports the generation, the counts and the pending state that the store and the
journal hold.

**Acceptance Scenarios**:

1. **Given** a built graph, **When** a client calls `status`, **Then** it gets the generation,
   its time, how it was made, and the counts, alongside today's fields.
2. **Given** no graph yet, or a store that needs a rebuild, **When** a client calls `status`,
   **Then** the graph section says so and why, and `status` still succeeds.
3. **Given** saves recorded and not yet applied, **When** a client calls `status`, **Then** it
   reports changes pending.

---

### Edge Cases

- **A query while an update runs:** it answers from the current published generation, at the
  usual speed, and says changes are pending. It never waits for the update or reads its work
  (Principle IV; decision record rule 2).
- **A query that would run too long:** a query whose evaluation passes a stated time limit is
  stopped and gets a structured error. It never blocks other operations past the limit.
- **A query that matches everything:** paging bounds what is sent; a total limit bounds what is
  evaluated when the client gives one.
- **A single binding larger than a page:** refused with a result-too-large error. No single
  response exceeds 2 MiB.
- **Unknown prefix or term:** an unknown vocabulary term or prefix is an error at its position,
  never a silent empty answer.
- **Constants naming items:** a constant may name a tag by name, as the documents' examples do
  (`:Dom`), or any item by its identifier. A name that matches no tag in the graph, or more than
  one, is an error.
- **A short-form scene-scoped fact with no scene in the query:** `?p :hasRole :Dom` alone
  matches performers who hold Dom in at least one scene or image.
- **A save the relations job then rewrites:** "Apply relations" writes relation data to scenes,
  which fires their hooks again. The graph update after it picks up the written relations; no
  loop follows, since an unchanged save changes no fact (spec 008 FR-008).
- **Hooks for kinds without semantic data:** a save of a scene with no semantic fields still
  records, and the update leaves the graph unchanged.
- **The engine absent or disabled:** saves record nothing, and the next update catches up from
  `updated_at` and ids (spec 008). Clients querying get Stash's own error that the plugin cannot
  run, and say the engine is missing (Principle I).
- **The graph needs a rebuild** (other format, damaged, other server): queries get a structured
  error saying so, never an answer from a store that needs a rebuild. The next update rebuilds
  it (spec 008).
- **A client built for another contract:** a request may say which contract version it was
  written for; a different major version gets a structured mismatch error, not a misread.
- **Concurrent jobs:** Stash runs jobs one at a time, so an update started by a hook waits
  behind a running build or "Apply relations" job. A job started while another update holds the
  store ends with `BUSY` and leaves the change to the next job.

## Requirements *(mandatory)*

### Functional Requirements

**The query operation**

- **FR-001**: The plugin MUST offer a query operation that takes a `SELECT` query text and
  returns the variables, the bindings, and the generation that answered.
- **FR-002**: The query language MUST support: `SELECT` with projection of named variables,
  `WHERE` with triple patterns, variables, constants (vocabulary terms, items, and literals),
  joins over shared variables, and `FILTER`s that compare two values (`=`, `!=`, `<`, `<=`,
  `>`, `>=`). Several `FILTER`s in one query all apply.
- **FR-003**: Any other construct MUST be refused before evaluation, with an error naming the
  construct and its position. Clients can rely on everything accepted staying accepted
  (Principle VI).
- **FR-004**: Vocabulary terms MUST be written with the vocabulary's own names (for example
  `:hasRole`, or the full `urn:semantic-stash:vocab:` identifiers), as defined in
  `vocabulary/TERMS.md`. The engine MUST NOT redefine them.
- **FR-005**: A constant MUST be able to name a tag by its Stash name, as the documents'
  examples do, or any item by its identifier. A name matching no tag, or several, MUST be an
  error at its position.
- **FR-006**: Each binding MUST give every projected variable's value with its type (item,
  vocabulary term, or literal with its datatype). Each item MUST also give its kind and Stash
  id.
- **FR-007**: Every answer MUST state the generation it came from, when it was published, and
  whether recorded changes are still waiting to reach the graph.
- **FR-008**: Answers MUST be deterministic: the same query on the same generation gives the
  same bindings in the same order.
- **FR-009**: Queries MUST only read the current published generation, opened read-only, and
  MUST NOT wait for, block, or read the work of an update or build (Principle IV; rules 1–3).

**Scoping**

- **FR-010**: In the documents' short form, a scene-scoped fact about a performer (`hasRole`,
  `hasAction`, `hasPluggable`) MUST match only when it holds in the scene or image that the
  query links that performer to; a scene's or image's `hasMiseEnScene` holds in that scene or
  image. Global facts (`hasTrait`, `hasLabel` and other performer and tag facts) MUST match in
  any scene. Relation instances and pairings name their scene or image themselves (`inMedia`),
  so they are always written in the graph's own shape.
- **FR-011**: A query MAY use the graph's own shape instead (instance nodes, `inMedia`,
  `ofTag`), and the same question in either form MUST give the same answer.
- **FR-012**: When a short-form scene-scoped fact's scene cannot be told, because its performer
  is linked to more than one scene or image variable, the query MUST be refused with an
  ambiguous-scope error at the fact's position. With no scene in the query, the fact holds in
  at least one scene or image.

**Recursive criteria**

- **FR-013**: A Role or Relation constant in a query MUST be able to be marked recursive. A
  recursive criterion matches that tag and every narrower one, through the engine's hierarchy
  as the `relations` operation reports it (spec 004), with loops ignored.
- **FR-014**: Without the mark, a Role or Relation criterion MUST match only that exact tag.
- **FR-015**: Marking a tag recursive that is neither a Role nor a Relation MUST be an error at
  its position.

**Paging and limits (Principle VII; rule 9)**

- **FR-016**: Each page of an answer MUST be at most 1 MiB of result, and no single response
  MAY ever exceed 2 MiB.
- **FR-017**: An answer that does not fit in one page MUST return a continuation. A
  continuation MUST return the next page from the same generation and in the same order, until
  the last page, which returns none.
- **FR-018**: A continuation whose generation is no longer kept MUST get an expired error, and
  an altered or foreign continuation MUST be refused. Neither MAY return bindings from another
  generation.
- **FR-019**: A client MUST be able to ask for a smaller page and for a total limit on
  bindings.
- **FR-020**: A query MUST be stopped, with a structured time-limit error, when its evaluation
  passes a stated limit. The plan sets the limit.

**Freshness from hooks (rules 4, 5, 6 as superseded, 11)**

- **FR-021**: The plugin MUST subscribe to the Create, Update, Destroy and Merge hooks of tags,
  performers, scenes and images, as far as Stash fires them (rule 4).
- **FR-022**: A hook handler MUST only record the change and, for the first change of a burst,
  start the "Update semantic graph" task. It MUST do no graph work (rule 5).
- **FR-023**: Every change recorded MUST reach the graph, through the job it started, a job
  already waiting, or one started after it. A crashed or stopped job MUST NOT lose recorded
  changes, and a stale "job waiting" record MUST NOT block new jobs.
- **FR-024**: The scene and image saves that start "Apply relations" (spec 004) MUST keep doing
  so. The plugin MUST record each save once for both purposes, so a save pays for one hook.
- **FR-025**: "Build semantic graph" MUST stay the manual full rebuild, and "Update semantic
  graph" stays runnable by hand.

**Status**

- **FR-026**: `status` MUST also report the graph: store format, current generation and its
  publish time, how and when it was last built or updated, counts of facts and of items by
  kind, and whether changes are pending. When there is no graph, or it needs a rebuild, it MUST
  say so and why, and `status` MUST still succeed.

**Contract and errors (Principle VI)**

- **FR-027**: Every query error MUST be structured: a code, a message, and the line and column
  in the query text where it applies. The error kinds MUST include: syntax error, unsupported
  construct, unknown term or tag, ambiguous scope, bad recursive mark, time limit, result too
  large, expired continuation, bad continuation, graph not built, graph needs a rebuild, and
  contract mismatch.
- **FR-028**: A request MAY state the contract version it was written for. A different major
  version MUST get a contract-mismatch error.
- **FR-029**: The operations contract (`docs/api.md`, `api/operations.json`, the manifest) MUST
  record the query operation, the status additions, the new error codes and the new hooks, as
  an additive (MINOR) change.
- **FR-030**: No operation MAY return the graph, a snapshot, or change files to a client
  (rule 10).

**Budgets (Principle VII)**

- **FR-031**: Query time MUST be stated and measured by a repeatable command at 250,000 scenes,
  for plain and recursive queries, and for a full 1 MiB page.
- **FR-032**: The hook's added save time MUST be stated and measured, per saved item, for single
  saves and bulk edits.
- **FR-033**: The time from a save to its change showing in answers MUST be stated and
  measured on the test instance.

### Key Entities

- **Query request**: the query text, and optionally a page size, a total limit, a continuation,
  and the contract version the client was written for.
- **Answer**: the variables; the bindings of one page; the generation, its publish time and
  the pending state; and a continuation when more pages remain.
- **Binding value**: an item (with its kind and Stash id), a vocabulary term, or a literal with
  its datatype.
- **Continuation**: what lets a client ask for the next page. It is tied to one query, one
  generation and one position, and is refused when altered.
- **Query error**: a code, a message, and a position (line and column) in the query text.
- **Change record**: one save recorded by a hook (kind, id, and what happened), waiting for an
  update.
- **Graph status**: format, generation and its time, how and when made, counts, pending state,
  or why there is no usable graph.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the compatibility fixture, every query of a stated set of at least 30 known-
  answer queries returns exactly the expected bindings. The set covers each predicate of the
  vocabulary's active terms, every filter comparison, joins of two and three performers, and
  each error kind.
- **SC-002**: Zero false matches from scope: on a fixture where performers hold different Roles
  in different scenes, every short-form query's answer equals the graph-form query's answer,
  and neither holds a binding whose Role is held only in another scene.
- **SC-003**: 004 SC-004 holds: a recursive query for Full Service returns exactly the scenes
  with Full Service or Extra Care, a non-recursive one exactly the Full Service scenes, with zero
  scenes wrong in either direction.
- **SC-004**: On a library of 250,000 scenes, 95% of queries answer in under 200 ms end to
  end, including the recursive queries (004 SC-007; rule 12), and a full 1 MiB page in under
  200 ms.
- **SC-005**: Over answers of up to 30,000 bindings read page by page, no page exceeds 1 MiB,
  no single response exceeds 2 MiB, and the pages together hold every binding exactly once, including
  when a generation is published between pages.
- **SC-006**: Each saved item takes at most 7 ms longer to save with the plugin installed than
  without it, counting all its hooks (rule 11), for single saves and in a 1,000-scene bulk edit.
- **SC-007**: On the test instance, a single save shows in query answers within 30 seconds,
  with Stash's job queue otherwise idle, and without any task run by hand.
- **SC-008**: In 100% of trials on the test instance (edits, deletions and merges of every
  kind, single and bulk), the graph after the hook-started updates equals a full rebuild.
- **SC-009**: `status` reports the same generation, counts and pending state as the store and
  the change record hold, in every state the tests set up.

## Assumptions

- **The language subset.** Phase 3 takes the documents' Phase 1 features and, of their Phase 2
  boolean logic, only single comparisons and inequality (Milestone 2's "basic filters").
  `&&`, `||`, `!`, `OPTIONAL`, `UNION`, `BIND`, ordering, and other functions stay with the
  roadmap's Phase 5, so refusing them now keeps the contract stable.
- **Hooks start a job, not an apply in the query.** Rule 6's original form (the next query
  applies changes) was superseded by spec 004 research R6: the hook starts a background job, and
  no client call pays for applying. Queries answer from the current generation and say whether
  changes are pending.
- **One hook per save.** The spec 004 hook already records scene and image saves for "Apply
  relations". This feature extends it rather than adding a second hook for the same saves, so
  the 7 ms budget covers both.
- **Freshness target.** 30 seconds from save to answer, as 004 SC-002, with an idle job queue.
  Spec 008 measured a 1-scene update at 2.4 s at Production size.
- **Retention.** Continuations rely on the store keeping recent generations (spec 006 G7: the
  current one, the one before, and those younger than the grace period). A client reading pages
  slowly may see an expired error and rerun the query.
- **The time limit** for a query is set by the plan, well above the 200 ms budget, so only
  runaway queries meet it.
- **Contract.** A new operation, new result fields, new error codes and new hooks: a MINOR,
  additive change, to 1.4.0.
- **Clients adopt in Phase 4.** This feature adds no client code. The clients' adoption,
  equivalence tests and retiring their own engines are Phase 4.
