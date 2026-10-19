# Data Model: Engine Design Spike

The spike stores no engine data that outlives it. These are the shapes of what it measures and
records. JSON field names are camelCase, and times are in milliseconds unless the name says
otherwise.

## Measurement

One repeatable figure. `spike-bench` writes one JSON file per run to `results/<id>.json`.

| Field | Type | Rule |
|---|---|---|
| `id` | string | `<question>-<variant>-<scale>`, e.g. `persist-snapshot-250k` |
| `question` | enum | `process`, `persistence`, `freshness`, `limits`, `snapshot`, `build`, `decision` (the seven roadmap items) |
| `variant` | string | e.g. `reopen`, `B`, `first-call` |
| `kind` | enum | `latency` (needs ≥ 100 runs), `build` and `size` (≥ 3), `observation` (≥ 1, e.g. first calls after a reload) |
| `command` | string | exact `spike-bench` command line that produced it, with no credentials |
| `server` | object | `{ stashVersion, host }`; host is always the test server or `local` (no Stash) |
| `library` | object | `{ kind: "test" \| "synthetic", scenes, images, performers, tags, triples }` |
| `runs` | integer | ≥ 100 for latency figures (spec FR-003); ≥ 3 for build and size figures |
| `samples` | number[] | raw values, kept so later runs can be compared (SC-002) |
| `median`, `p95`, `min`, `max` | number | derived from `samples` |
| `parts` | object | optional breakdown, e.g. `{ processStart, storeOpen, query, serialise }` |
| `failures` | object[] | `{ run, kind, message }` with credentials stripped |
| `takenAt` | string | RFC 3339 |
| `engineCommit` | string | git commit of the workspace (`-dirty` when uncommitted changes exist) |
| `extra` | object | optional further results, e.g. a size curve or a proposed page size |

**Validation**:
- `median` and `p95` are recomputed from `samples` on load, so they can never disagree with the
  raw data.
- A figure with fewer runs than its `kind` needs is rejected (`latency`: 100).
- The first call after a plugin reload is never in `samples`. It goes in its own measurement,
  with `variant = "first-call"`.

## Option

A candidate answer to one question.

| Field | Type | Rule |
|---|---|---|
| `question` | enum | as above |
| `name` | string | e.g. `A live read-only`, `B published snapshots`, `C writer only` |
| `measurements` | string[] | Measurement ids |
| `observedFailures` | string[] | e.g. "reader saw partial write", "busy after 2 s" |
| `constitution` | enum | `meets`, `needs-amendment`, `violates` (with principle) |
| `verdict` | enum | `chosen`, `rejected`, `fallback` |

## Hook observation

One row per save. The harness writes them to `results/hooks.json`, merged from the spike
plugin's hook log, `hooks.jsonl`.

| Field | Type | Rule |
|---|---|---|
| `entity` | enum | `tag`, `scene`, `image`, `performer` |
| `client` | enum | `web-ui`, `semantic-tagging`, `viewer` |
| `mode` | enum | `scripted`, `manual` (owner-driven confirmation) |
| `mutation` | string | GraphQL mutation sent, e.g. `sceneUpdate`, `bulkSceneUpdate` |
| `items` | integer | items changed (1 for single saves; 10/100/1000 for bulk) |
| `hooksReceived` | object[] | `{ type, id, inputFields, inputBytes, receivedAt }` |
| `delay` | number | from save request sent to last hook received |
| `saveLatency`, `saveLatencyNoHook` | number or null | median client-visible save time with and without the spike's hooks enabled (null for manual rows and saves a client does not send) |
| `repeats` | integer | saves timed for the latency figures |
| `note` | string | optional: what the payload stands for, or why a client sends nothing |
| `updatedAtBumped` | boolean | the item appears in an `updated_at > t` filter after the save |
| `missed`, `outOfOrder` | boolean | derived |

**Coverage rule** (SC-003): there must be at least one scripted row for each of the 12
`entity` × `client` pairs, and at least one manual row for each client.

## Decision

One per roadmap item, in `decision-record.md` (format in
[contracts/decision-record.md](contracts/decision-record.md)).

| Field | Rule |
|---|---|
| question | one of the seven roadmap items |
| decision | the chosen option, or "not applicable" with a reason |
| evidence | Measurement ids and Hook observation rows |
| reproduce | the `spike-bench` commands |
| rule for later specs | the sentence Phase 2 or 3 must follow, e.g. "readers open only published snapshots" |
| amendment | principle and proposed text, if any (spec FR-015) |

## Decision record

| Field | Rule |
|---|---|
| outcome | `proceed`, `proceed-with-changes`, `fall-back` (spec FR-014) |
| date, stashVersion, engineCommit | recorded as of the final measurement run |
| decisions | exactly seven, one per roadmap item (SC-001) |
| persistence, concurrency, freshness rules | required when the outcome is not `fall-back` |
| failed limit | required when the outcome is `fall-back` |

## Seed manifest

Written by `spike-bench seed` to `results/seed-manifest.json` before any write. Cleanup needs it.

| Field | Rule |
|---|---|
| `server` | must equal the test server URL; cleanup refuses any other |
| `statsBefore` | `{ scenes, images, performers, tags }` from Stash `stats` |
| `createdTags` | tag ids created by the seed |
| `changed` | `{ type, id, customFieldsBefore, keys }` for every scene, image or performer touched; `keys` are the custom fields the seed writes, so cleanup restores those it overwrote and removes those it added |
| `state` | `seeding` → `seeded` → `cleaned`; a crash leaves `seeding`, and cleanup still reverses whatever was recorded |

**Invariant** (SC-005): after cleanup, `stats` equals `statsBefore` and every `changed` item's
custom fields equal `customFieldsBefore`.

## Snapshot generation

The layout on disk under the spike plugin's folder, for option B in research R2.

```text
store/
├── work/            # writer's read-write store (only the lock holder opens it)
├── snapshots/<n>/   # published checkpoints, never written after publication
├── current          # text: the published generation number, replaced by atomic rename
├── writer.lock      # advisory lock (std File::try_lock)
└── assets/          # snapshot files and manifest.json served at /plugin/semantic-engine-spike/assets/
```

The state transitions for one write are:
1. Take `writer.lock`.
2. Write `work/`.
3. Check the writes landed with `flush`.
4. Run `backup` into `snapshots/<n+1>/`.
5. Rename the temporary file over `current`.
6. Release the lock.
7. Garbage-collect generations older than the grace period, keeping at least the newest two.
