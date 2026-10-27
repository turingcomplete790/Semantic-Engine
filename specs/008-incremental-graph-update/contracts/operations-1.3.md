# Contract: operations 1.3.0 (additions)

These additions change the plugin's operation contract from **1.2.0** to **1.3.0**. The change
is a MINOR, additive one (constitution Principle VI); nothing in 1.2.0 changes.

`api/operations.json`, `docs/api.md` and the manifest get the entries below, and
`engine::CONTRACT_VERSION` becomes `1.3.0`. The agreement test keeps them in line.

## Task "Update semantic graph" (operation `updateGraph`)

**Manifest entry:**

```yaml
  - name: Update semantic graph
    description: Bring the semantic graph up to date with what changed since the last build or update. Rebuilds in full when it has to.
    defaultArgs:
      op: updateGraph
```

**Arguments:** none.

**Result:**

```json
{
  "mode": "update",
  "generation": 7,
  "changes": {
    "tags":       { "changed": 1, "added": 0, "removed": 0 },
    "performers": { "changed": 3, "added": 1, "removed": 0, "dependents": 2 },
    "scenes":     { "changed": 4, "added": 0, "removed": 1 },
    "images":     { "changed": 0, "added": 0, "removed": 0 }
  },
  "facts": { "removed": 210, "added": 236, "total": 41048 },
  "notes": { "skipped": {}, "mismatches": {} },
  "phases": { "read": 120, "map": 3, "write": 15, "publish": 41 },
  "seconds": 0.21
}
```

**`mode`:**

| Mode | Meaning |
|---|---|
| `update` | changes applied, and a generation published |
| `current` | nothing changed; no generation published, so `generation` is the current one |
| `rebuild` | the update fell back to a full rebuild |

**`rebuildReason`:** present with `mode: "rebuild"`. It is one of:
- `firstBuild`;
- `storeNeedsRebuild`, with `cleared`, the 006 reason;
- `mappingVersion` or `vocabularyVersion`, with `from` and `to`;
- `noCatchUpPoint`;
- `tooManyChanges`, with the counts.

A rebuild's other fields are those of `buildGraph`.

**Log lines:**
- progress, as `buildGraph`;
- note lines, as `buildGraph`, for re-read items only;
- one summary line, for example:

  `update: generation 7 published; tags 1 changed; performers 3 changed, 1 added, 2 dependents;
  scenes 4 changed, 1 removed; facts -210 +236 (41048); skipped 0; mismatches 0; 0.2 s`

  `update: the graph is current; generation 6 kept; 0.1 s`

**Errors:** as `buildGraph`: `SERVER_ID_NOT_GENERATED`, `SERVER_ID_INVALID`, `BUSY`,
`STASH_UNREACHABLE`, `STASH_TOO_OLD`, `INTERNAL`. On every error nothing is published.

**Reads:**
- every semantic tag;
- per kind: two change queries, from one second before the catch-up point (what is re-read)
  and from the point itself (what counts toward the rebuild limit; research R13), and one id
  list;
- the changed and dependent records, by id, in batches.

## Task "Build semantic graph" (unchanged)

Its arguments, result and errors do not change. Its generation now records the build stamp
(research R7), so updates can follow it.
