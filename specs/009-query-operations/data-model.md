# Data model: Query Operations

What this feature adds, in a new module `engine::query`, and in the plugin's folder. The
language is in [contracts/query-language.md](contracts/query-language.md), and the operations in
[contracts/operations-1.4.md](contracts/operations-1.4.md).

## The query pipeline

```text
text ──parse──► Ast ──resolve──► Resolved ──rewrite──► Plan ──print──► SPARQL text
                                                         │
                       Reader (one generation) ◄─────────┘ ──► Page
```

Each step either succeeds or returns a `QueryError`. Nothing is evaluated until `print`.

## Ast (parse)

| Type | Fields | Notes |
|---|---|---|
| `Pos` | `line: u32`, `column: u32` | 1-based, on every node |
| `Ast` | `prefixes: Vec<(String, String, Pos)>`, `distinct: bool`, `projection: Projection`, `patterns: Vec<TriplePattern>`, `filters: Vec<Filter>` | one group; no nesting |
| `Projection` | `All` or `Vars(Vec<(String, Pos)>)` | |
| `TriplePattern` | `s: Node`, `p: Node`, `o: Node`, `pos: Pos` | `;` and `,` lists are expanded; `a` becomes `rdf:type` |
| `Node` | `Var(String)`, `Iri(String)`, `Prefixed { prefix, local }`, `Literal { lexical, datatype, language }` | each with its `Pos` |
| `Filter` | `left: Node`, `op: CompareOp`, `right: Node`, `pos: Pos` | |
| `CompareOp` | `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge` | |

**Validation:** the rules of the grammar. A projected variable that is in no pattern, or a
variable starting `__se`, is `Syntax`.

## Resolved (resolve)

Every prefixed name and IRI becomes a `Term`, checked against the vocabulary and the
generation.

| Type | Fields |
|---|---|
| `Resolved` | the `Ast`'s shape, with each `Node` a `Var`, a `Literal` or a `Const` |
| `Const` | `term: Term`, `what: ConstKind`, `recursive: bool`, `pos: Pos` |
| `ConstKind` | `VocabTerm`, `Item(Kind)`, `Other` |

**Lookups,** all against the one generation the query runs on:
- **a tag by name:** tags whose `rdfs:label` is the name. Exactly one must match.
- **an item by id or IRI:** the item has at least one fact as subject.
- **a recursive constant:** the tag has `rdf:type sem:Role` or `sem:Relation`.

## Plan (rewrite)

| Type | Fields | Notes |
|---|---|---|
| `Plan` | `variables: Vec<String>`, `patterns: Vec<PlanPattern>`, `values: Vec<(String, Vec<Term>)>`, `filters: Vec<Filter>` | what `print` writes |
| `PlanPattern` | `s`, `p`, `o` as `Var` or `Term` | user patterns, rewritten short forms, and hidden ones |

**Rules** (research R3, R5):
- A short-form triple becomes three patterns (two for `hasMiseEnScene`), with a hidden variable
  `?__seN`.
- A recursive constant becomes a hidden variable and a `values` entry holding
  `Hierarchy::expand_criterion(tag, true)`.
- A performer with several `hasPerformer` media terms and a short-form fact is
  `ScopeAmbiguous`.

`print(plan)` gives the text below. The engine orders the answer by the projected variables and
takes the page itself (research R4):

```text
SELECT DISTINCT ?v1 … WHERE { patterns . VALUES … FILTER(…) … }
```

## Page and continuation

| Type | Fields |
|---|---|
| `PageRequest` | `page_size: Option<u32>` (1–20,000), `limit: Option<u64>`, `continuation: Option<String>` |
| `Page` | `variables`, `bindings: Vec<Vec<Value>>`, `offset: u64`, `continuation: Option<String>`, `generation: Generation` |
| `Value` | `Item { iri, kind, id }`, `Node { iri }`, `Term { iri, name }`, `Literal { value, datatype, language }` |
| `Continuation` | `v: 1`, `generation: u64`, `offset: u64`, `page_size: Option<u32>`, `limit: Option<u64>`, `query_hash: u64`, `check: u64` |

**Continuation encoding:**
- lower-case hex of the JSON of the fields above (no new dependency);
- `check` is FNV-1a 64 over the key `query.key` (16 random bytes, created on first use in the
  plugin folder) followed by the other fields' bytes;
- `query_hash` is FNV-1a 64 over the query text.

**Validation:**
- **decoding fails, `check` differs, or `query_hash` differs:** `PageInvalid`;
- **the generation is no longer kept:** `PageExpired`.

**Constants:**

| Constant | Value |
|---|---|
| `PAGE_BYTES` | 1,048,576: a page's serialized `bindings` stay within it |
| `PAGE_ROWS_MAX` | 20,000 |
| `QUERY_TIME_LIMIT` | 10 s |
| `QUERY_LANGUAGE_LEVEL` | 1 |

## QueryError

| Kind | Code | Details |
|---|---|---|
| `Syntax` | `QUERY_SYNTAX` | `line`, `column` |
| `Unsupported { construct }` | `QUERY_UNSUPPORTED` | `construct`, `line`, `column` |
| `Unknown { name, why }` | `QUERY_UNKNOWN` | `name`, `line`, `column` |
| `ScopeAmbiguous { performer }` | `QUERY_SCOPE_AMBIGUOUS` | `line`, `column` |
| `NotRecursive { name }` | `QUERY_NOT_RECURSIVE` | `name`, `line`, `column` |
| `TooLarge` | `RESULT_TOO_LARGE` | |
| `PageExpired { generation }` | `PAGE_EXPIRED` | `generation` |
| `PageInvalid` | `PAGE_INVALID` | |
| `NotBuilt` | `GRAPH_NOT_BUILT` | |
| `NeedsRebuild { reason }` | `GRAPH_NEEDS_REBUILD` | `reason` |
| `Internal` | `INTERNAL` | |

`QUERY_TIMEOUT` and `CONTRACT_MISMATCH` are raised by the plugin, around the engine call.

## Graph store additions (spec 006)

| Addition | Meaning |
|---|---|
| `snapshots/<n>.qindex` | generation `n`'s query index (research R12): its facts as term ids with owners, in three sorted orders, a term dictionary and predicate statistics; written at publish, removed with the generation |
| `GraphStore::kept_generation(n)` | the record of kept generation `n`, with the stamp checked, without opening the store |
| `GraphStore::reader_at(n)` | opens kept generation `n` read-only; `Gone` when it is not kept |
| `GraphStore::describe()` | reads the current generation's file and the stamp only: `Ready(Generation)`, `NotBuilt`, or `NeedsRebuild(reason)`; never opens the store |

## Build stamp addition (spec 008 R7)

`items: { tags, performers, scenes, images }`, item counts by kind.
- **A full build** sets them from its owners.
- **An update** sets them to the previous counts plus added, minus removed.
- **Absent** in stamps written before 1.4.0, which `status` reports as `null`.

## Hierarchy addition (spec 004)

`Hierarchy::from_links(links: impl IntoIterator<Item = (child, parent)>)` builds a hierarchy from
`broaderRole` and `broaderRelation` links, with `Hierarchy::build`'s loop removal. The two
constructors share the cycle code, so "narrower" means one thing.

## Plugin folder state (research R8)

| File | Written by | Meaning |
|---|---|---|
| `graph.pending` | the hook (create-new) | an update is wanted; the creator starts the job |
| `graph.applying` | the update task (rename of `graph.pending`) | an update is running; removed at its end |
| `query.key` | the first `query` that needs a continuation | the continuation check key; disposable |

**State changes:**

```text
(none) ──hook──► pending ──update starts──► applying ──update ends──► (none)
                    ▲                           │
                    └──BUSY or failure: restored, backdated as stale
```

A marker older than `STALE_AFTER` (15 min) does not block a new job. **Pending** means
`graph.pending` exists, or `graph.applying` exists and is younger than `STALE_AFTER`.

The spec 004 files (`journal.jsonl`, `apply.pending`, `apply-watermark`) are unchanged.
