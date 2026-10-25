# Data Model: Graph Store

These are the engine's own types for the graph store (`engine::graph`, research R1), and the
store's files on disk (research R2). Everything here is a disposable cache derived from Stash
(Principle I).

## Term

| Variant | Fields | Rules |
|---|---|---|
| `Iri` | `String` | an absolute IRI, for example `urn:semantic-stash:vocab:Role`. Invalid IRIs are rejected with `InvalidFact` |
| `Blank` | `String` | a blank node label, `[A-Za-z0-9_-]+` |
| `Literal` | `value: String`, `datatype: Option<String>`, `language: Option<String>` | at most one of `datatype` and `language`. With neither, it is a plain string |

**Values:** any Unicode text, of any length, is stored and returned unchanged (edge case
"Unicode and long values").

## Fact

| Field | Type | Rules |
|---|---|---|
| `subject` | `Term` | `Iri` or `Blank` only |
| `predicate` | `Term` | `Iri` only |
| `object` | `Term` | any |

**Graph:** every fact is in the store's default graph (spec Assumptions).

**Duplicates:** adding a fact that is already present has no effect. Removing one that is absent
has no effect.

## Pattern

`{ subject: Option<Term>, predicate: Option<Term>, object: Option<Term> }`, used by
`remove_matching`. `None` matches anything. All three `None` is allowed and removes every fact;
`clear()` does the same.

## Generation

| Field | Type | Rules |
|---|---|---|
| `number` | u64 | starts at 1, increases by 1 per publish, no gaps or repeats (FR-007) |
| `published_at` | RFC 3339 text, UTC | when `current` switched to it |
| `facts` | u64 | counted at publish (research R6) |
| `format` | u32 | `STORE_FORMAT` when published |
| `server_id` | string | the stamp's server id |

It is stored as `snapshots/<n>.json`, next to the generation's folder.

**States:**
1. **Building:** the working store, with `work.dirty` present.
2. **Published:** `snapshots/<n>/` and its `.json` exist.
3. **Current:** `current` names it.
4. **Previous:** it is current minus one, and is kept.
5. **Old:** it is neither current nor previous. Old generations past the grace period are
   removed (R5).

A generation's files never change after it is published.

## Store stamp (`store.json`)

| Field | Type | Rules |
|---|---|---|
| `format` | u32 | must equal `STORE_FORMAT` (1), or the store needs a rebuild |
| `serverId` | string | must equal the plugin setting `serverId`, or the store needs a rebuild |
| `createdAt` | RFC 3339 text, UTC | when the store was created or last cleared |

## Health

`check()` gives one of these:

| Variant | Meaning | Read allowed |
|---|---|---|
| `Ready { generation }` | stamp matches; current generation opens | yes |
| `NotBuilt` | stamped but never published | no: "not built yet" (FR-013) |
| `NeedsRebuild(Missing)` | no store folder, or no stamp in it | no; `clear_for_rebuild` creates the store |
| `NeedsRebuild(Damaged { detail })` | stamp or `current` unreadable, generation folder missing, or open failed | no |
| `NeedsRebuild(OtherFormat { found, expected })` | stamp's format differs | no |
| `NeedsRebuild(OtherServer)` | stamp's server id differs | no |

A writer may open a store that is `Ready` or `NotBuilt`. A store that needs a rebuild must be
cleared with `clear_for_rebuild` first (FR-016).

## Solutions

| Field | Type | Rules |
|---|---|---|
| `variables` | `Vec<String>` | in the query's projection order, without `?` |
| `rows` | `Vec<Vec<Option<Term>>>` | one row per solution, in the query's order. Each row has one cell per variable; an unbound variable is `None` |

## Summary

| Field | Type |
|---|---|
| `health` | `Health` |
| `generation` | `Option<Generation>` (the current one) |
| `format` | u32 |
| `server_id` | `Option<String>` |
| `disk_bytes` | u64, the whole store folder |
| `writing` | bool, whether a writer holds the lock now |

## GraphError

| Kind | Fields | When |
|---|---|---|
| `Busy` | — | another writer holds the lock (FR-004) |
| `NotBuilt` | — | read with nothing published (FR-013) |
| `NeedsRebuild` | reason, as in Health | read or write on a store that needs a rebuild (FR-015) |
| `InvalidQuery` | `message`, `line: Option<u32>`, `column: Option<u32>` | parse error, or a query form other than `SELECT` (FR-012) |
| `InvalidFact` | `message` | a bad IRI, or a literal with both a datatype and a language |
| `NoServerId` | — | no server id to stamp with (spec Assumptions) |
| `Io` | `message` | disk or file-system failure, disk full included |
| `Internal` | `message` | any other failure inside the graph library |

Every kind has a short `kind()` name for logs. No message contains a credential (FR-020).
