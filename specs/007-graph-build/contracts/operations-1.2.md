# Contract: operations 1.2.0 (additions)

These additions change the plugin's operation contract from **1.1.0** to **1.2.0**. The change
is a MINOR, additive one (constitution Principle VI); nothing in 1.1.0 changes.

`api/operations.json` and `docs/api.md` get the entries below. The agreement test keeps both in
line with the plugin, and `engine::CONTRACT_VERSION` becomes `1.2.0`.

**New error code:** `BUSY`. Another writer holds the graph store. It is added to `ErrorCode`,
`api/operations.json` and `docs/api.md`.

## Task "Build semantic graph" (operation `buildGraph`)

**Manifest entry** (`plugin/semantic-engine.yml`):

```yaml
  - name: Build semantic graph
    description: Read the whole library and rebuild the semantic graph from it. The previous graph stays in use until the new one is published.
    defaultArgs:
      op: buildGraph
```

**Arguments:** none.

**What it does:** a full rebuild, in the order of research R4. It writes only the engine's own
folder, `{PluginDir}/graph`. It reads Stash and never writes to it.

**Result:**

```json
{
  "generation": 4,
  "facts": 41022,
  "records": { "tags": 17, "performers": 886, "scenes": 300, "images": 12 },
  "cleared": "damaged",
  "notes": {
    "skipped": { "unreadable": 1, "badShape": 2 },
    "mismatches": { "tagWrongType": 5 }
  },
  "phases": { "read": 180, "map": 2100, "write": 9200, "publish": 900 },
  "seconds": 12.4
}
```

`phases` gives milliseconds per phase. `cleared` is present only when the store needed a rebuild. Its value is the 006 reason:
`damaged`, `otherFormat` or `otherServer`. A store that is simply missing (the first build) is
not reported as cleared.

**Log lines** (Stash's job log):
- **Progress:** progress lines from 0 to 1, at most one per whole percent.
- **Clearing:** one info line when the store was cleared, with its reason.
- **Notes:** up to 1,000 warning lines, one per note. A line names the item kind and id, the
  key, the entry, and the reason, and never the value. Then one line counting the notes not
  written.
- **Summary:** one info line, research R8.

**Errors:**

| Code | When |
|---|---|
| `SERVER_ID_NOT_GENERATED` | No `serverId` setting. The message says to run "Generate server identifier" first |
| `SERVER_ID_INVALID` | The setting is not a valid identifier |
| `BUSY` | Another build, or another writer, holds the store. Nothing changed |
| `STASH_UNREACHABLE` | Stash could not be reached, or timed out |
| `STASH_TOO_OLD` | Stash is older than the supported minimum |
| `INTERNAL` | Anything else, including refused credentials. The message names the failure: for example, "Stash refused the API key (401)" |

On every error, nothing is published, and the previous generation stays current.

**Stopping:** Stash's "Stop" ends the process (research R2). Nothing is published, and Stash
shows the job as cancelled.

**Reads** (spec 005): one full read, with every semantic tag, performer, scene and image.

## Store interface addition (spec 006)

`GraphStore::rebuild_writer() -> Result<Writer, GraphError>`. It behaves like `writer()`, with
the same errors (`NoServerId`, `NeedsRebuild`, `Busy`), and starts from an empty working store.
The published generations are untouched until `publish()` (research R3).
`specs/006-graph-store/contracts/graph-store-interface.md` gains this entry.
