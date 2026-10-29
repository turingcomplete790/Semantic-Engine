# Contract: operations 1.4.0 (additions)

These additions move the plugin's operation contract from **1.3.0** to **1.4.0**. This is a
MINOR, additive change (constitution Principle VI); nothing in 1.3.0 changes meaning.

`api/operations.json`, `docs/api.md` and the manifest get the entries below, and
`engine::CONTRACT_VERSION` becomes `1.4.0`. The agreement test keeps them in line.

## Operation `query`

Runs one query of the [query language, level 1](query-language.md) on the published graph.

**Arguments:**

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `query` | string | yes | the query text |
| `pageSize` | integer, 1 to 20,000 | no | at most this many bindings per page; pages also stop at 1 MiB |
| `limit` | integer ≥ 1 | no | at most this many bindings over all pages |
| `continuation` | string | no | from the previous page; `query` must be the same text |
| `contract` | string `MAJOR.MINOR.PATCH` | no | the contract the client was written for |

**Result:**

```json
{
  "variables": ["scene", "person"],
  "bindings": [
    { "scene":  { "type": "item", "iri": "urn:semantic-stash:b8b4…:scene:12", "kind": "scene", "id": "12" },
      "person": { "type": "item", "iri": "urn:semantic-stash:b8b4…:performer:7", "kind": "performer", "id": "7" } }
  ],
  "page": { "count": 1, "offset": 0, "continuation": null },
  "graph": { "generation": 9, "publishedAt": "2026-10-27T19:02:11Z", "pending": false },
  "seconds": 0.041
}
```

**Value types:**

| `type` | Fields | Used for |
|---|---|---|
| `item` | `iri`, `kind` (`tag`, `performer`, `scene`, `image`), `id` | Stash items |
| `node` | `iri` | instances, pairings, timeline entries, rules |
| `term` | `iri`, `name` | vocabulary terms, for example `{ "name": "Role" }` |
| `literal` | `value` (string), `datatype`, `language`? | literals; `value` is the lexical form |

**Paging:**
- `page.continuation` is `null` on the last page. Otherwise, send it back with the same
  `query` (and the same `pageSize` and `limit`, which it carries) to get the next page.
- Every page comes from the generation in `graph.generation`. The order is deterministic for one
  generation, and not sorted.
- Every page's `bindings` serialize to at most 1 MiB. No response exceeds 2 MiB.

**`graph.pending`:** true when saves have been recorded that are not yet in this generation.

**Errors.** Each query error has `details.line` and `details.column` (1-based, in the query
text) where it applies:

| Code | When |
|---|---|
| `INVALID_INPUT` | `query` missing, or an argument of the wrong type or range |
| `QUERY_SYNTAX` | the text does not parse, or redefines a built-in prefix |
| `QUERY_UNSUPPORTED` | a refused construct; `details.construct` names it |
| `QUERY_UNKNOWN` | an unknown prefix, a term that is not Active, a tag name matching no tag or several, or an item that does not exist; `details.name` |
| `QUERY_SCOPE_AMBIGUOUS` | a short-form fact whose performer is linked to several media terms |
| `QUERY_NOT_RECURSIVE` | a recursive mark on a tag that is neither a Role nor a Relation |
| `QUERY_TIMEOUT` | evaluation passed 10 s; nothing is returned |
| `RESULT_TOO_LARGE` | one binding alone is over 1 MiB |
| `PAGE_EXPIRED` | the continuation's generation is no longer kept; run the query again |
| `PAGE_INVALID` | the continuation is altered, or belongs to another query |
| `GRAPH_NOT_BUILT` | no generation yet; run "Build semantic graph" or "Update semantic graph" |
| `GRAPH_NEEDS_REBUILD` | the store needs a rebuild; `details.reason` (`damaged`, `otherFormat`, `otherServer`) |
| `CONTRACT_MISMATCH` | `contract` has another MAJOR; `details.engine`, `details.client` |
| `INTERNAL` | anything else |

`query` never contacts Stash, and never changes the graph. The first call that needs a
continuation creates the disposable check key `query.key` in the plugin folder (research R6).

## Operation `status` (additions)

`status` gains a `graph` object. It is also present in the `details` of `STASH_UNREACHABLE` and
`STASH_TOO_OLD`, like the other local fields.

```json
"graph": {
  "state": "ready",
  "format": 2,
  "generation": 9,
  "publishedAt": "2026-10-27T19:02:11Z",
  "how": "update",
  "facts": 41048,
  "items": { "tags": 17, "performers": 886, "scenes": 300, "images": 12 },
  "pending": false
}
```

| `state` | Other fields |
|---|---|
| `ready` | all of the above; `how` and `items` are `null` for a generation made before 1.4.0 |
| `notBuilt` | `pending` |
| `needsRebuild` | `reason` (`damaged`, `otherFormat`, `otherServer`), `pending` |

`status` never fails because of the graph.

## Hook "Record changes for the semantic engine" (replaces "Apply relations after a save")

```yaml
hooks:
  - name: Record changes for the semantic engine
    description: Records saved tags, performers, scenes and images, and starts "Update semantic graph" and, for scenes and images, "Apply relations". Adds only a few milliseconds to a save.
    triggeredBy:
      - Tag.Create.Post
      - Tag.Update.Post
      - Tag.Destroy.Post
      - Tag.Merge.Post
      - Performer.Create.Post
      - Performer.Update.Post
      - Performer.Destroy.Post
      - Scene.Create.Post
      - Scene.Update.Post
      - Scene.Destroy.Post
      - Image.Create.Post
      - Image.Update.Post
      - Image.Destroy.Post
    defaultArgs:
      op: recordChange
```

**Checked on the test instance (Stash v0.31.1, T026; `results/hook-triggers.json`):** Stash
loads the manifest, and each trigger above fires for its save, including a `tagsMerge`, an image
created by a scan, and a destroy of every kind. Two triggers of the first draft were removed:

- **`Performer.Merge.Post`** and **`Scene.Merge.Post`**: Stash accepts them in the manifest, but a
  `performerMerge` or a `sceneMerge` fires no hook of any kind, so neither the merge nor the
  removal of its sources starts an update. The graph shows such a merge at the next update,
  started by any later save or by hand ("Update semantic graph"); spec 008 finds the changed
  destination by `updated_at` and the removed sources by id comparison.

**`recordChange` result:**
- before: `{ "recorded", "started" }`;
- now: `{ "recorded", "started", "graph": { "recorded", "started" } }`.

`started` keeps its 1.1.0 meaning ("Apply relations" started), and `graph.started` says
"Update semantic graph" was started. A save of a kind "Apply relations" does not handle reports
`recorded: false, started: false` there, as before.

## Tasks (unchanged)

"Update semantic graph" keeps its arguments, result and errors. Started by the hook, it also
takes and clears the pending marker (research R8).
