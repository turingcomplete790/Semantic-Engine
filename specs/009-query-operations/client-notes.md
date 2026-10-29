# Notes for client maintainers: the `query` operation

For the maintainers of the semantic-tagging plugin and the Semantic Stash Viewer. Contract
1.4.0 adds a `query` operation that answers the design documents' Phase 1 queries on the shared
semantic graph. The full contract is [`docs/api.md`](../../docs/api.md#query), and the language is
[`docs/query-language.md`](../../docs/query-language.md). These notes cover what is easy to get
wrong.

**Sources:** constitution Principle II requires the engine to build what the semantic-tagging
design documents describe. These notes rely on `semantic-query-engine-roadmap.md`, read at commit
`3aab0be` (2026-10-24):
- § Query Language Direction: the basic query, multiple relationships, and multiple bound
  performers;
- § Query Features, Phase 1, and the comparisons and inequality of Phase 2;
- § Core Design Decisions 2, "The semantic graph is the abstraction boundary", with its
  Vocabulary corrections (scene-scoped Roles, Actions and Pluggables);
- § Step 7, what a query UI shows.

Where these notes and the documents disagree, the documents win. Please report the disagreement.

## Calling it

```graphql
mutation {
  runPluginOperation(plugin_id: "semantic-engine", args: {
    op: "query",
    query: "SELECT ?scene ?p WHERE { ?scene :hasPerformer ?p . ?p :hasRole :Dom }",
    contract: "1.4.0"
  })
}
```

Send `contract` with the version you were written for. An engine of another MAJOR version
answers `CONTRACT_MISMATCH` instead of guessing.

## The language level

The engine accepts level 1, the documents' Phase 1:
- `SELECT` with variables or `*`, and an optional `DISTINCT`, which changes nothing;
- one `WHERE` group of triple patterns, with `;`, `,` and `a`;
- `FILTER` with one comparison each: `=`, `!=`, `<`, `<=`, `>`, `>=`.

Everything else is refused with `QUERY_UNSUPPORTED`. The error's `details.construct` names the
construct, with its line and column. This covers `OPTIONAL`, `UNION`, `&&`/`||`/`!`, `ORDER BY`,
`LIMIT`, paths and sub-queries. Show that message as it is; it is written for the person who
typed the query. Boolean logic arrives with the roadmap's Phase 5, as a new language level.

## Names

| Write | Means |
|---|---|
| `:Dom`, `:Red%20Hair` | a vocabulary term when the local name is one (`:hasRole`), otherwise the tag with exactly that Stash name; spaces and other characters are percent-encoded |
| `sem:hasRole` | a vocabulary term; it must be Active (`sem:hasTag` is Planned and refused) |
| `tag:12`, `performer:7`, `scene:100`, `image:3` | Stash items by id, on this server |
| `recursive:Full%20Service`, `recursive-tag:40` | a Role or Relation tag and every narrower one (below) |
| `rdf:`, `rdfs:`, `xsd:` | the W3C namespaces |

- **Tag names must match exactly, and only once.** A name that matches no tag, or several tags,
  is `QUERY_UNKNOWN`, and the message says which. When names can repeat, write the tag by id
  (`tag:12`).
- **Built-in prefixes cannot be redefined.** A `PREFIX` line may add new prefixes, but one that
  redefines a built-in is `QUERY_SYNTAX`.

## The short form, and its scope rule

Write Roles, Actions and Pluggables the way the documents do, as performer facts:

```sparql
SELECT ?scene ?dom ?sub WHERE {
  ?scene :hasPerformer ?dom . ?scene :hasPerformer ?sub .
  ?dom :hasRole :Dom . ?sub :hasRole :Sub .
  FILTER (?dom != ?sub)
}
```

Following Core Design Decisions 2, the engine joins each such fact within the one scene or image
that links its performer through `hasPerformer`. A performer who is Dom in one scene and Sub in
another never matches both in the same scene.

- **Ambiguous scope.** When a performer variable is linked by `hasPerformer` to two different
  scene or image variables, a short-form fact about it could hold in either, and the engine
  refuses with `QUERY_SCOPE_AMBIGUOUS` rather than choose. Link each performer to one scene, or
  write the fact in the graph's own form (`?p sem:hasRole ?i . ?i sem:ofTag :Dom ; sem:inMedia
  ?scene`).
- **No `hasPerformer` link.** The fact may hold in any scene or image.
- **Traits and Labels** are global to the performer and need no scope.

## Recursive criteria

`recursive:` matches the tag and every narrower Role or Relation at any depth, by the same
hierarchy as the `relations` operation, with links on a loop ignored. Without the mark, a tag
matches only itself, as in spec 004. A mark on a tag that is neither a Role nor a Relation is
`QUERY_NOT_RECURSIVE`.

## Answers and paging

- **Values have a type:** `item` (with `kind` and `id`, ready for Stash), `node`, `term` (with
  `name`) and `literal`. Use `kind` and `id` rather than parsing IRIs.
- **Answers are sets in a fixed but unsorted order.** The same query on the same generation
  always gives the same order, which is what makes paging exact, but the order is the engine's
  own. Sort on your side when it matters. This keeps first pages fast on large libraries: the
  engine stops as soon as a page is full.
- **Pages** are at most 1 MiB, with `pageSize` up to 20,000 bindings. When
  `page.continuation` is not `null`, send it back with the same `query` text to get the next
  page; `pageSize` and `limit` travel inside it.
  - Every page of one answer comes from the generation in `graph.generation`, even after a newer
    one is published.
  - A continuation whose generation has been cleaned up (kept at least 15 minutes) is
    `PAGE_EXPIRED`: start again from the first page.
  - An altered continuation, or one sent with another query, is `PAGE_INVALID`.
- **Deep pages cost more.** Each page re-runs the query up to its end. Use `limit` when you only
  need the first results.
- **A query still running after 10 s** is stopped with `QUERY_TIMEOUT`.

## Freshness: `graph.pending`

Saves made through Stash reach the graph without anyone running a task. The save hook records
each save, and the first save of a burst starts "Update semantic graph". On the test instance a
change showed in answers 1.05 s after the save.

- `graph.pending` (in every `query` result, and in `status.graph`) is `true` while saves are
  recorded that the answering generation does not hold yet. A UI can say "updating…" and ask
  again shortly.
- **Merges:** a tag merge is seen at once. Stash v0.31.1 fires no hook for a performer merge or a
  scene merge, so these reach the graph with the next update, started by any later save or by
  hand.
- `status.graph` reports the state, generation, how it was made, its counts, and `pending`,
  without opening the store. It is cheap to poll.

## Errors to show

Errors about the query text carry `details.line` and `details.column` (1-based): show them with
the message, as Step 7 asks. `GRAPH_NOT_BUILT` and `GRAPH_NEEDS_REBUILD` mean the owner should
run "Build semantic graph" or "Update semantic graph".
