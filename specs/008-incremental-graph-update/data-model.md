# Data model: Incremental Graph Update

The additions to the store are in [contracts/graph-store-format-2.md](contracts/graph-store-format-2.md),
and the reader's in [contracts/reader-additions.md](contracts/reader-additions.md). This page
covers the types the update adds in `engine::build`.

## Build stamp

The `build` object of a generation file (research R7).

| Field | Type | Meaning |
|---|---|---|
| `mappingVersion` | integer | `MAPPING_VERSION` of the engine that made the generation |
| `vocabularyVersion` | string | the embedded vocabulary's version |
| `catchUp` | map kind → Stash text | per kind, the newest `updated_at` when the build or update began; absent for a kind with no items |
| `how` | `build` or `update` | which task made it |

**Validation:** an unreadable or partial stamp counts as absent, and forces a rebuild
(`noCatchUpPoint`).

## Ownership

| Owner | Graph name | Facts |
|---|---|---|
| a tag, performer, scene or image | its item IRI | everything its record's mapping gives (spec 007) |
| the vocabulary | `urn:semantic-stash:vocab:` | `core.ttl`, `labels.ttl`, `timeline.ttl` |

**Rule:** owners' facts never overlap (research R1). A test checks this over the fixture and
the synthetic library.

`owner_kind(iri) -> Option<Kind>` reads the kind from an item IRI (`…:{server}:{kind}:{id}`).

## Change set

What one update works on.

| Field | Type | Meaning |
|---|---|---|
| `point` | `ReadStart` | the new catch-up point, taken first |
| `tags` | the semantic tags | every one, read whole (research R4) |
| `changed` | map kind → ids | from `changed_since`, for performers, scenes and images |
| `dependents` | ids | performers re-read because a tag they carry changed |
| `deleted` | map kind → ids | owners in the store that Stash no longer has, or that are no longer semantic |

**State changes of one owner:**

```text
not in store ──record semantic──► in store
in store ──record changed, still semantic──► replaced (or Unchanged)
in store ──record gone, or not semantic──► removed
```

## UpdateReport

The `updateGraph` result (contracts/operations-1.3.md).

| Field | Type | Meaning |
|---|---|---|
| `mode` | `Update`, `Current` or `Rebuild` | what the run did |
| `rebuild_reason` | `Option<RebuildCause>` | why it rebuilt |
| `generation` | `u64` | published, or kept when `Current` |
| `changes` | per kind: changed, added, removed; dependents for performers | the change set's outcome |
| `facts` | removed, added, total | from `Replaced` and the kept count |
| `notes` | as spec 007 | re-read items only |
| `phases` | read, map, write, publish (ms) | |
| `seconds` | `f64` | |

### `RebuildCause`

`FirstBuild`, `StoreNeedsRebuild(reason)`, `MappingVersion { from, to }`,
`VocabularyVersion { from, to }`, `NoCatchUpPoint`, `TooManyChanges { items, limit }`.

## Constants

| Constant | Value | Meaning |
|---|---|---|
| `MAPPING_VERSION` | 1 | bumped by any mapping rule change that changes a record's facts |
| `UPDATE_SHARE_LIMIT` | 1/3 | above this share of the store's items to re-map, rebuild instead; set from R10's measurement |
| `UPDATE_MIN_LIMIT` | 500 | the share limit never falls below this many items, so a small library's updates stay updates (added in implementation) |
| `STORE_FORMAT` (006) | 2 | named graphs per owner |
