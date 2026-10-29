# Research: Query Operations

Each decision below resolves a question the plan's Technical Context left open. Code facts were
read at commit `cdbffab`; the plugin's documents at semantic-tagging `3aab0be` (2026-10-24).

## R1. Parsing the query language

**Decision:** the engine parses queries itself, with a small hand-written parser for the
accepted language ([contracts/query-language.md](contracts/query-language.md)). It produces the
engine's own query tree, with the line and column of every token. Only after checking,
resolving and rewriting does the engine print a SPARQL text for the store to evaluate.

**Rationale:**
- **Positions everywhere.** FR-027 needs the line and column of an unknown tag, an ambiguous
  scope or an unsupported construct, not only of syntax errors. A SPARQL algebra keeps no
  source positions, so a parse into it would lose them for every error but the first.
- **Refusing precisely.** The tokenizer knows every SPARQL keyword (`OPTIONAL`, `UNION`,
  `BIND`, `ORDER`, `GROUP`, and the rest), so it names the construct and its position (FR-003)
  instead of a generic parse error.
- **Small.** The accepted language is a prologue, `SELECT`, and one group of triple patterns and
  filters, with `;` and `,` lists: a few hundred lines.
- **Stays behind the engine's interface.** The store still sees SPARQL text (spec 006 G9: no
  Oxigraph type in the interface), and the existing `Reader::select` runs it.

**Alternatives considered:**
- **Parse with Oxigraph's SPARQL parser and walk the algebra:** no positions after parsing, and
  rewriting the algebra would tie the engine to Oxigraph's types.
- **Pass client SPARQL through to the store:** clients would get every SPARQL construct, which
  then becomes contract (Principle VI), and the short form's scoping (R3) could not be enforced.

## R2. Names: vocabulary terms, tags and items

**Decision:** built-in prefixes, always available, which a query's own `PREFIX` lines may add
to but not redefine:

| Prefix | Means | Example |
|---|---|---|
| `sem:` | a vocabulary term, `urn:semantic-stash:vocab:` | `sem:hasRole` |
| `:` (default) | a vocabulary term if the name is one; otherwise a tag by its Stash name | `:hasRole`, `:Dom` |
| `tag:`, `performer:`, `scene:`, `image:` | an item by its Stash id; the engine adds the server id | `scene:12` |
| `recursive:` | a Role or Relation by name, recursive (R5) | `recursive:Customer` |
| `recursive-tag:` | a Role or Relation by id, recursive | `recursive-tag:30` |
| `rdf:`, `rdfs:`, `xsd:` | the usual W3C namespaces | `xsd:decimal` |

- **Names with spaces** use SPARQL's percent encoding in local names: `:Full%20Service`. The
  local name is percent-decoded before lookup.
- **Tag names** are looked up exactly, among tags, by their `rdfs:label` (the tag's Stash name,
  mapping rules of spec 007). No match, or more than one, is `QUERY_UNKNOWN` at the name's
  position. Matching is exact: Stash's own names are what the clients show.
- **Vocabulary terms** are checked against the embedded vocabulary. An Active term is accepted.
  A Planned term (for example `hasTag`) or a Derived term that is not stored (`inStage`) is
  `QUERY_UNKNOWN`, with the term's status in the message, never a silent empty answer.
- **Item ids** are checked to exist in the generation (one indexed lookup each). A missing item
  is `QUERY_UNKNOWN`.
- **Full IRIs** (`<urn:semantic-stash:…>`) are accepted as written, and checked the same way
  when they are vocabulary or item IRIs.

**Rationale:** the documents write `:hasRole :Dom`, so the default prefix must cover both terms
and tag names. The id prefixes let the viewer, which holds Stash ids, write queries without
knowing the server id. A vocabulary term wins over a tag of the same name; that tag stays
reachable as `tag:{id}`.

**Derived terms** (added in implementation): a Derived term that a Stash mapping entry
produces (`sem:produces` in `stash-mapping.ttl`, for example `bustSize` and the other
measurement fields) is stored by the builder, and is accepted like an Active one. A Derived
term no mapping produces (`inStage`) is refused.

**The server id** (added in implementation): `query` never contacts Stash, so it cannot read
the `serverId` setting. It opens the store with `GraphStore::stamped`, which takes the server
id from the store's own stamp. The folder is the plugin's own, and the stamp is checked on
every read as before.

**Alternatives considered:** tag names as string literals (`"Dom"`) in object position, which
would be ambiguous with real literals; a request argument mapping names to ids, which moves the
lookup into every client.

## R3. The short form and scoping

**Decision:** the rewrite, after resolving names:

1. **Short-form facts.** A triple `S P O` is short form when `P` is `hasRole`, `hasAction`,
   `hasPluggable` or `hasMiseEnScene`, and `O` is a tag (a tag constant, or a variable that is
   never the subject of another pattern in the query). Otherwise it is in the graph's own shape
   and is left as written (FR-011).
2. **Its scope.** For `hasRole`, `hasAction` and `hasPluggable`, `S` is a performer. Its scope is
   the media term `M` of the query's `M sem:hasPerformer S` patterns:
   - exactly one distinct `M`: the fact is tied to `M`;
   - none: the fact is tied to a fresh hidden variable, so it holds in at least one scene or
     image (spec edge case);
   - more than one: `QUERY_SCOPE_AMBIGUOUS` at the fact's position (FR-012).

   For `hasMiseEnScene`, `S` is the media entity itself.
3. **The rewrite.** `S P O` in scope `M` becomes
   `S P ?i . ?i sem:ofTag O . ?i sem:inMedia M .` with `?i` a fresh hidden variable. For
   `hasMiseEnScene`, the `inMedia` line is not needed: the instance is the media's own.
4. **Global facts** (`hasTrait`, `hasLabel`, `parentTag`, `broaderRole`, categories, and every
   other term whose scope is `Global`) are left as written.

Hidden variables are named `?__se1`, `?__se2`, …; user variables starting `__se` are refused.

**Rationale:** the documents require the short form to stay as simple as their examples and
the evaluator to enforce scope "through how it joins". Tying each instance to the bound scene
through `inMedia` is exactly that. The graph-form test (SC-002) checks the rewrite: the same
question written with the instance nodes by hand must give equal answers.

**Relations and pairings** need no short form: a `RelationInstance` or `Pairing` names its
media by `inMedia`, and its performers by `atEndA`, `atEndB` or `pairsA`/`pairsB` instances, so
graph-form queries are already scoped (spec FR-010).

**Alternatives considered:**
- **Tie each fact to every linking media variable:** a performer bound to two scenes would need
  the Role in both, which is a guess the documents do not make. Refusing is clearer.
- **A separate short-form vocabulary** (for example `q:hasRole`): it would redefine terms,
  which FR-004 forbids.

## R4. Answers: distinct, ordered, typed

**Decision:**
- **Distinct.** Every answer is a set of bindings over the projected variables (`DISTINCT`).
  The rewrite's hidden variables would otherwise repeat a binding once per instance, which no
  client wants. A query's own `DISTINCT` is accepted and changes nothing.
- **Ordered.** **Superseded by R12:** answers on the query index come in evaluation order,
  deterministic for one generation and not sorted, so a page returns as soon as it is full. The
  sort below remains for generations without an index. Answers are ordered by every projected
  variable, in projection order, so the
  same query on the same generation gives the same pages (FR-008). **Changed in implementation
  (T034):** the engine sorts and slices the answer itself, instead of printing `ORDER BY`,
  `OFFSET` and `LIMIT`. On 30,000 scenes in release, `SELECT DISTINCT ?s { ?s a sem:Scene }`
  took 137 ms, and the same with `ORDER BY ?s` 2.8 s: Oxigraph's sort was about twenty times
  the evaluation, on every page. Sorting in the engine brought a page to 0.24–0.30 s. The
  order within one variable: unbound, blank nodes, IRIs, then literals; IRIs and blank nodes
  by text; numeric literals by value, before other literals; other literals by lexical form,
  datatype and language. It is total, and close to SPARQL's term order, which leaves mixed
  literals partly to the implementation.
- **Typed values.** Each value is `{ "type": "item", "iri", "kind", "id" }`,
  `{ "type": "node", "iri" }` (an instance, pairing, timeline entry or rule),
  `{ "type": "term", "iri", "name" }` (a vocabulary term), or
  `{ "type": "literal", "value", "datatype" }` (and `language` where present). `kind` and `id`
  come from the item IRI (`urn:semantic-stash:{server}:{kind}:{id}`, spec 008 `owner_kind`).

**Rationale:** set answers and a stable order make paging by position (R6) exact. Telling items
from nodes saves every client from parsing IRIs.

**Alternatives considered:** SPARQL's bag semantics with duplicates; ordering in the store with
`ORDER BY`, first chosen to avoid holding the whole answer, but the store holds it to sort
anyway, and much more slowly (above).

## R5. Recursive Role and Relation criteria

**Decision:**
- **The mark** is the `recursive:` or `recursive-tag:` prefix on a constant (R2). A marked
  constant that is not a Role or a Relation is `QUERY_NOT_RECURSIVE` at its position (FR-015).
- **The hierarchy** is read from the generation itself: every `sem:broaderRole` and
  `sem:broaderRelation` fact (one small query; a few hundred facts on Production). From these
  links the engine builds the spec 004 `Hierarchy`, with the same loop removal, through a new
  constructor `Hierarchy::from_links`. `expand_criterion(tag, true)` then gives the tag and every
  narrower one.
- **The rewrite** replaces the constant by a hidden variable bound by a `VALUES` block of the
  expanded tags. `VALUES` is printed by the engine only; clients cannot write it.

**Rationale:**
- **One meaning of "narrower".** The `relations` operation and auto-apply use `Hierarchy` with
  its loop rule. The graph's `broaderRole` facts keep looping links (mapping rules,
  `parents` row), so a SPARQL path `sem:broaderRole*` would answer differently on a library with
  a loop.
- **No Stash call.** The generation holds the links, so queries stay local (Principle VII).
- **Cheap.** 004 SC-007 needs recursive queries within the same 200 ms. A `VALUES` block of a
  few tags costs no more than the plain query.

**Alternatives considered:** a `recursive: true` request argument for all criteria, which
cannot mark one criterion and gives no position for FR-015; reading the definitions from Stash,
as the `relations` operation does, which adds a round trip to every query.

## R6. Paging without state

**Decision:**
- **Each page call evaluates the query again**, on the continuation's generation. On the query
  index (R12) evaluation stops once it has the bindings up to the page's end and one more;
  otherwise the engine orders the answer and takes the page from the continuation's offset (R4: `OFFSET`
  and `LIMIT` were first printed in the query, until Oxigraph's sort proved too slow). Nothing
  is kept between calls.
- **A page** holds bindings until the next one would take the page's serialized `bindings` past
  1 MiB (`PAGE_BYTES = 1,048,576`), or until the page-size or total limit is reached. A
  binding left after the page tells that a continuation is needed.
  Bindings per page are capped at `PAGE_ROWS_MAX = 20,000`.
- **The continuation** is an opaque string: a version, the generation number, the next offset,
  the page size, the total limit, a hash of the normalized query, and a check value over all of
  them keyed by a random secret kept in the plugin folder (`query.key`, created on first use).
  - Its generation no longer current or kept: `PAGE_EXPIRED`.
  - A wrong check value, or a query hash that does not match the query sent with it:
    `PAGE_INVALID`.
- **The reader opens the continuation's generation** when it is still kept (spec 006 G7:
  current, the one before, and those younger than `GENERATION_GRACE`, 15 minutes), never the
  current one in its place (FR-017).
- **A single binding over 1 MiB** is `RESULT_TOO_LARGE`. No response is over 2 MiB, since a page
  is under 1 MiB plus a small envelope.

**Rationale:** Stash starts a process per call (decision record §1), so a server-side cursor
would mean writing answers to disk on a read path, and cleaning them up. Re-evaluating with an
offset costs one evaluation per page, which the 003 spike measured as affordable: a 1 MiB page
in 102 ms at p95, with the 250k query at 89 ms.

**Alternatives considered:**
- **Answer files on disk per query:** faster later pages, but writes and cleanup on reads, and
  files that outlive their generation.
- **A plain offset as the continuation:** an altered one would silently return another page.

**Opening an older generation:** the reader today opens only `current`. It gains
`GraphStore::reader_at(generation)`, which opens a kept generation read-only, or reports it gone.

## R7. The time limit

**Decision:** the query runs on a worker thread. The operation waits for it up to
`QUERY_TIME_LIMIT = 10 s`. If the limit passes, the plugin writes a `QUERY_TIMEOUT` envelope
and ends its process at once.

**Ending the process** uses `libc::_exit`, after stdout is flushed. Found in implementation
(T018): `std::process::exit` runs RocksDB's C++ static destructors while the query thread is
still inside RocksDB, and the process crashed with exit 139 after writing its output.

**Rationale:**
- Oxigraph 0.4 has no cancellation of a running query.
- The plugin process exists only for this call (decision record §1), so ending it is a clean
  way to stop the evaluation.
- A reader holds no lock, so nothing is left behind.
- 10 s is fifty times the query budget: only runaway queries, such as unbounded cross joins,
  meet it.

**Alternatives considered:** checking the time between solutions, which cannot stop a join that
produces no solution for minutes; no limit, which leaves Stash's request waiting without bound.

## R8. Hooks and the pending marker

**Decision:**
- **One hook entry** in the manifest replaces "Apply relations after a save". Its new name is
  "Record changes for the semantic engine", and it keeps the operation `recordChange`. It is
  triggered by:
  - `Tag.Create.Post`, `Tag.Update.Post`, `Tag.Destroy.Post`, `Tag.Merge.Post`;
  - `Performer.Create.Post`, `Performer.Update.Post`, `Performer.Destroy.Post`,
    `Performer.Merge.Post`;
  - `Scene.Create.Post`, `Scene.Update.Post`, `Scene.Destroy.Post`, `Scene.Merge.Post`;
  - `Image.Create.Post`, `Image.Update.Post`, `Image.Destroy.Post`.

  The pinned schema has `tagsMerge`, `performerMerge` and `sceneMerge`, and no image merge. The
  implementation checks on the test instance that Stash v0.31.1 accepts each trigger and fires
  it, and drops any it does not know (research task in tasks.md).

  **Outcome (T026):** every trigger fires except `Performer.Merge.Post` and `Scene.Merge.Post`.
  A `performerMerge` or `sceneMerge` on v0.31.1 fires no hook at all, so both were removed; such
  a merge reaches the graph with the next update (contracts/operations-1.4.md).
- **What the hook does:**
  - for a scene or image create or update, what it does today (spec 004): append to the
    relations journal, and start "Apply relations" for the first save of a burst;
  - for every event, create the graph's pending marker `graph.pending`, and when this call
    created it, start "Update semantic graph".
- **The graph needs no per-item journal.** The update finds changes from `updated_at` and
  deletions by id comparison (spec 008), including saves whose hooks were lost. The marker only
  says "an update is wanted".
- **The update task and the markers:**
  1. At start, it renames `graph.pending` to `graph.applying`, before taking its catch-up point.
     A save after that point creates a new `graph.pending` and starts a new job.
  2. At its end, success or not, it removes `graph.applying`.
  3. When it ends with `BUSY` or a failure, it restores `graph.pending` with an old modification
     time, so the next hook sees it as stale and starts a job (FR-023).
- **Stale markers:** a marker older than 15 minutes (`STALE_AFTER`, as spec 004) no longer blocks
  a new job.
- **Pending, as reported** by `query` and `status`: `graph.pending` exists, or `graph.applying`
  exists and is younger than `STALE_AFTER`.

**Rationale:**
- **Rule 5:** the hook only records.
- **Rule 6, as superseded by 004 R6:** the hook starts a background job.
- **One hook per save:** Stash runs every matching hook entry, each one a plugin process, so a
  second entry for scene saves would double their cost. One entry keeps a scene save at one
  process, within rule 11's 7 ms.
- **Coalescing:** the marker turns a 1,000-scene bulk edit into one job, or two when saves
  continue past the job's start.
- **Taking the marker before the catch-up point** is what makes FR-023 hold: every save is
  either before the point, and read, or after it, and starts another job.

**Alternatives considered:**
- **A per-item graph journal:** it duplicates `updated_at`, and its entries would need draining
  with no use.
- **Applying in the query** (rule 6 as first written): superseded by 004 R6.

**Interaction with "Apply relations":** its writes fire `Scene.Update.Post` again. If an update
job is already queued, the marker exists and no job starts; the queued job runs after the apply
and reads its writes. Otherwise one more update runs. It changes the relation facts, and then
the graph is current, so nothing loops (spec 008 FR-008).

## R9. Status and graph counts

**Decision:**
- `status` gains a `graph` object.
- **It reads only the generation's file**, through a new `GraphStore::describe()`, which never
  opens the store. That keeps `status` at its current cost.
- **The build stamp** (spec 008 R7) gains `items`, counts per kind, kept by the builder:
  - a full build counts its owners;
  - an update adds and subtracts its changes.
- **Older generations** without `items` report `null` counts until the next build or update.

| Field | From |
|---|---|
| `state` | `ready`, `notBuilt`, or `needsRebuild` (with `reason`, as spec 006) |
| `format`, `generation`, `publishedAt`, `facts` | the generation file |
| `how` (`build` or `update`), `items` | the build stamp |
| `pending` | the markers (R8) |

**Rationale:** counting owners in the store at 250k means listing 575,000 graphs, about a second.
That is too slow for `status`, which a client may call on every page load.

## R10. Contract version in requests

**Decision:** `query` accepts an optional `contract` argument, the contract version the client
was written for. A different MAJOR than the engine's is `CONTRACT_MISMATCH`, with both versions
in `details`. A lower or higher MINOR is accepted: MINOR changes are additive.

**Rationale:** Principle VI requires that clients can detect a mismatch. `status` already
reports the version; this lets a query fail clearly in one call.

## R11. Measuring

**Decision:** three repeatable commands.

| What | Command | Basis |
|---|---|---|
| Query time at 250k, end to end | `query-bench` (new bin in the plugin crate). It runs the plugin binary once per call, as Stash does, on a synthetic 250k store, over a fixed query set: plain, two- and three-performer joins, filters, recursive, and a full 1 MiB page | rule 12; 004 SC-007; SC-004 |
| Hook cost per saved item | `spike-bench hooks`, single and `--bulk 1000`, with the plugin installed and without it, on the test instance | rule 11; SC-006 |
| Save to answer | `query-bench fresh`, on the test instance: it saves a scene's roles, then calls `query` every 0.5 s until the change shows | SC-007 |

**Rationale:** the spike's `call-cost` measured process start, store open and evaluation
together. `query-bench` measures the same path through the real operation, including parsing,
name lookup, the rewrite and paging.

## R12. The query index (added in implementation, T042)

**Finding (T042, 250,000 scenes, 24.3 million facts):** Oxigraph's evaluation cannot meet
SC-004 for join queries. Every bench query except the one anchored on a single performer ran
past the 10 s limit with the whole answer ordered (R4). Streamed with `LIMIT` and no order, the
first 8,000 bindings of a Role query took 1.4 s, and of a two-performer join 142 s. Measured
directly:
- **A point lookup costs 435–641 µs** on the generation as published, which is 145
  overlapping bulk-loaded SST files with no compaction, 12 GB on disk. After `Store::optimize()`
  (131 s): 81 files, 4.7 GB, 7 ms to open, but still 42–48 µs per lookup and 11 µs per fact
  scanned, because every term is decoded through further RocksDB reads in a process that starts
  cold.
- **Its planner joins badly** on the short form's shape: in the two-performer join it binds
  `?b hasRole ?i` before scoping `?i` to the scene, 110,055 intermediate rows for 333 answers at
  2,000 scenes. With optimizations off it is slower still.

**Decision:**
- **Each published generation gets a query index**, an engine-owned file beside it
  (`snapshots/<n>.qindex`), read through a memory map:
  - a dictionary of the generation's terms, with a permutation sorted by the encoded term for
    lookup;
  - every fact as `(s, p, o, owner)` ids, in three sorted orders (`spo`, `pos`, `osp`), each with
    a table of where each first id's block starts;
  - per predicate: facts, distinct subjects, distinct objects.
- **Level-1 queries are evaluated on the index by the engine** (`engine::qindex::eval`): the
  rewrite's plan (R3, R5) is joined depth-first by range lookups, in an order chosen by
  estimated fan-out from the predicate statistics, with each filter as soon as its variables are
  bound. Names (R2) resolve on the index too, so a query never opens Oxigraph.
- **Answers stream in the evaluation order**, which is deterministic for one generation and one
  query (FR-008), and evaluation stops when the page is full (R4 and R6 change accordingly; the
  order is no longer sorted).
- **Publishing writes the index:** a full build collects it as it writes; an update merges the
  base generation's index with its changes, dropping the facts of every owner it touched and
  adding their current facts. Cleanup removes it with its generation. A generation without an
  index (made before this) is still answered by Oxigraph.
- **Oxigraph stays the store:** writes, generations, publishing, the health checks and full SPARQL
  for later language levels.

**Measured (prototype, 250k, evaluation in process):**

| Query | Oxigraph, ordered | Index, whole answer sorted | Index, first 1 MiB page streamed |
|---|---|---|---|
| Role in a scene | over 10 s | 170 ms (137,501) | 9 ms |
| two performers, `!=` | over 10 s | 360 ms (41,667) | 83 ms |
| three performers (empty) | over 10 s | 584 ms | 578 ms |
| Trait and Role | over 10 s | 190 ms (32,054) | 54 ms |
| recursive Role | over 10 s | 322 ms (233,334) | 9 ms |
| every scene | 2.6 s | 154 ms (250,000) | 5 ms |

The index file is 1.42 GB at 250k (the store is 12 GB), and writing it took 17 s after mapping.
The prototype's answers equal Oxigraph's on every known query of the three fixtures.

**Rationale:** the budget is about 25 µs per binding for a 1 MiB page; the index costs about
0.1 µs per lookup. Keeping Oxigraph as the store keeps generations, crash safety and the
writer as specs 006–008 built them.

**Alternatives considered:**
- **Compacting at publish:** 2–3× faster lookups, still seconds for joins.
- **Planning joins in the engine over Oxigraph's pattern lookups:** the 40 µs lookup remains.
- **Relaxing SC-004:** declined by the owner.

**Constitution:** Principle V names Oxigraph for "the graph and SPARQL evaluation". Level-1
evaluation now runs on an engine-owned index derived from Oxigraph's generations. The plan
records this as a justified deviation; the owner decides on an amendment.
