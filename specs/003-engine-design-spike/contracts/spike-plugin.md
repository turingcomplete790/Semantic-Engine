# Contract: spike plugin `semantic-engine-spike`

This plugin is only for measurement. It is installed alongside the real plugin, never packaged,
and absent from `docs/api.md` (spec FR-013, research R9). Its replies use the same envelope as
contract 1.0.0, so the harness can reuse its parser. They are not part of that contract.

## Manifest (`spike/semantic-engine-spike.yml`)

```yaml
name: Semantic Engine Spike
description: Measurement-only build for the Phase 1 spike. Do not install on real libraries.
version: 0.0.0-spike
exec:
  - spike-plugin
interface: raw
ui:
  assets:
    /: store/assets              # served at /plugin/semantic-engine-spike/assets/
hooks:
  - name: Record semantic saves
    description: Appends one line per hook to hooks.jsonl and exits.
    triggeredBy:
      - Tag.Create.Post
      - Tag.Update.Post
      - Tag.Merge.Post
      - Tag.Destroy.Post
      - Scene.Create.Post
      - Scene.Update.Post
      - Scene.Destroy.Post
      - Image.Create.Post
      - Image.Update.Post
      - Image.Destroy.Post
      - Performer.Create.Post
      - Performer.Update.Post
      - Performer.Destroy.Post
    defaultArgs:
      op: recordHook
```

The hooks are switched off for the "without hook" save-latency figures. The harness does this by
disabling the plugin with `setPluginsEnabled`, and it re-enables the plugin afterwards.

## Operations (`args.op`)

| op | Arguments | Result | Used by |
|---|---|---|---|
| `query` | `scale`, `queryId` (1–5) | `{ rows, timings: { storeOpen, query, serialise } }` | `call-cost`, `concurrency` |
| `write` | `option` (`A`/`B`/`C`), `scale`, `lockMode` (`wait`/`fail`), `waitMs` | `{ triples, timings: { lock, write, flush, publish }, generation }` | `concurrency`, `synth` |
| `buildFromStash` | — | `{ items, triples, timings: { fetch, build } }` (flag-filtered fetch via adapter) | `build` |
| `payload` | `bytes` | `{ bindings: [...] }` of about the requested size | `payload` |
| `exportSnapshot` | `store`, `to` (`output`/`asset`), `level` (gzip 0–9, default 6) | `output`: `{ manifest, data }` (gzip + base64 N-Quads). `asset`: `{ manifest, path }` | `snapshot` |
| `snapshotInfo` | `store` (optional) | `{ manifest, currentGeneration, assetIsStale }`: the `manifest.json` contents (`null` if no asset was exported) and the store's published generation | `snapshot`, freshness check |
| `applyEdit` | `store`, `items` (default 1), `seed` | moves each picked scene's role assignments to the next role tag, writes `deltas/<to>.nq.gz` (`D `/`A ` N-Quads lines) and `deltas/<to>.json` before publishing generation `to`; returns `{ from, to, removed, added, bytes, scenes, timings }` | `delta` |
| `recordHook` | (hook context from Stash) | appends `{ receivedAt, type, id, inputFields, inputBytes }` to `hooks.jsonl` | hooks |

Store layout and the publish sequence follow the
[snapshot generation](../data-model.md#snapshot-generation) section of the data model.

## Rules

- **Connections and credentials:** it connects only to `server_connection` and logs no cookie,
  same as the real plugin (002 FR-016).
- **Hook handling:** `recordHook` does no other work. Its own run time is the floor for a hook
  handler (R4), and it is measured.
- **Writes:** it never writes to Stash. All Stash writes come from `spike-bench`.
