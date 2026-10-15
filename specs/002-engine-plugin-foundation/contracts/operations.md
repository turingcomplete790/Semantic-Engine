# Contract: Plugin Operations

Contract version: **1.0.0**. Clients call operations with Stash's
`runPluginOperation(plugin_id: "semantic-engine", args: {"op": "<name>", ...})` and tasks with
`runPluginTask(plugin_id: "semantic-engine", task_name: "<Task Name>")`.

## Envelope

Every operation returns, as the operation result, an object:

```json
{"ok": true, "op": "status", "contractVersion": "1.0.0", "result": { }}
{"ok": false, "op": "getServerId", "contractVersion": "1.0.0",
 "error": {"code": "SERVER_ID_NOT_GENERATED", "message": "Run the Generate server identifier task first."}}
```

Stash's own GraphQL error is used only when the plugin cannot produce an envelope (it crashed or
its output is not JSON). Clients MUST check `ok`.

## Operations

| Op | Arguments | Result | Errors | Since |
|---|---|---|---|---|
| `ping` | none | `{"pong": true}` | none | 1.0.0 |
| `status` | none | versions per data-model "Versions reported by status"; `serverId` (string or null); `rocksdb` `{"version", "available": true}` | `STASH_UNREACHABLE` (status still returns local versions inside `details`) | 1.0.0 |
| `getServerId` | none | `{"serverId": "<uuid>"}` | `SERVER_ID_NOT_GENERATED`, `SERVER_ID_INVALID`, `STASH_UNREACHABLE` | 1.0.0 |

## Task

| Task name (manifest) | `args.op` | Behaviour | Log result |
|---|---|---|---|
| `Generate server identifier` | `generateServerId` | Read plugin settings; if `serverId` absent, generate a UUID v4, merge it into the existing settings, write, re-read, report. If present and valid, report it unchanged. If present and invalid, report and change nothing. | info line `serverId=<uuid> (generated|existing)` or error line with the code |

## Error codes

| Code | Meaning |
|---|---|
| `INVALID_INPUT` | stdin was missing, not JSON, or lacked `server_connection` |
| `UNKNOWN_OPERATION` | `args.op` missing or not a known operation; `details.known` lists the names |
| `STASH_UNREACHABLE` | the adapter could not reach Stash's GraphQL endpoint |
| `STASH_TOO_OLD` | the server's version is below v0.31.1 (FR-017) |
| `SERVER_ID_NOT_GENERATED` | no `serverId` setting yet |
| `SERVER_ID_INVALID` | the stored `serverId` is not a lower-case hyphenated UUID |
| `INTERNAL` | an unexpected failure; message says what failed, never includes credentials |

## Compatibility

Adding an operation, an optional argument, a result field, or an error code is MINOR. Removing or
renaming any of them, or changing a type, is MAJOR. The API document and `api/operations.json`
change in the same revision as the code (FR-011, FR-012).
