# Semantic Engine API

Contract version: **1.0.0**

This is the contract for clients of the Semantic Engine Stash plugin, such as the Semantic Stash
Viewer and the semantic-tagging plugin. It lists every operation and task, what to send, what
comes back, and which errors can occur. The same list in machine-readable form is
[`api/operations.json`](../api/operations.json); an automated test keeps this document, that
file, and the plugin in agreement.

The engine is reached only through Stash. There is no separate server, port, or API key: you call
Stash's GraphQL API as you already do, and Stash runs the plugin.

## Calling the engine

The plugin's id is `semantic-engine`.

**Operations** run immediately and return a result:

```graphql
mutation {
  runPluginOperation(plugin_id: "semantic-engine", args: { op: "status" })
}
```

**Tasks** go into Stash's job queue. The mutation returns a job id; the task's result appears in
Stash's log, not in the response:

```graphql
mutation {
  runPluginTask(plugin_id: "semantic-engine", task_name: "Generate server identifier")
}
```

Both need a Stash session or API key with the usual permissions, exactly like any other Stash
GraphQL call.

## Result envelope

Every operation returns one JSON object as the value of `runPluginOperation`:

```json
{ "ok": true, "op": "status", "contractVersion": "1.0.0", "result": { } }
```

```json
{ "ok": false, "op": "getServerId", "contractVersion": "1.0.0",
  "error": { "code": "SERVER_ID_NOT_GENERATED", "message": "no server identifier yet; run the \"Generate server identifier\" task" } }
```

| Field | Type | Meaning |
|---|---|---|
| `ok` | boolean | `true` on success |
| `op` | string | the operation name as received |
| `contractVersion` | string | version of this contract, `MAJOR.MINOR.PATCH` |
| `result` | object | present when `ok` is `true`; shape per operation below |
| `error.code` | string | one of the [error codes](#error-codes) |
| `error.message` | string | a readable explanation; never contains credentials |
| `error.details` | object | optional, extra data per error |

**Always check `ok`.** Errors from the engine come back as a normal result with `ok: false`,
because Stash would otherwise reduce them to a plain string. A GraphQL-level error from
`runPluginOperation` means the plugin could not run at all (for example it is not installed or
is disabled).

## Operations

### `ping`

Answers without contacting Stash or doing any work; useful for checking that the plugin is
installed and for measuring call overhead.

- Arguments: none
- Result: `{ "pong": true }`
- Errors: none
- Since: 1.0.0

### `status`

Reports versions, the server's Stash version, and the server identifier.

- Arguments: none
- Since: 1.0.0
- Result (example from a test server):

```json
{
  "engineVersion": "0.1.0",
  "contractVersion": "1.0.0",
  "vocabularyVersion": "1.1.0",
  "oxigraphVersion": "0.4.11",
  "rocksdbVersion": "10.3.0",
  "rocksdb": { "version": "10.3.0", "available": true },
  "stashMinimumVersion": "v0.31.1",
  "stashVersion": "v0.31.1",
  "serverId": "b8b41b17-046a-4bf6-9862-1d120f081263",
  "build": { "target": "x86_64-unknown-linux-gnu", "profile": "release" }
}
```

| Field | Meaning |
|---|---|
| `engineVersion` | version of the installed plugin |
| `contractVersion` | version of this API contract |
| `vocabularyVersion` | version of the semantic vocabulary the engine uses |
| `oxigraphVersion`, `rocksdbVersion`, `rocksdb` | versions of the bundled graph store and its storage library |
| `stashMinimumVersion` | oldest supported Stash release |
| `stashVersion` | the version of the Stash server that called the plugin |
| `serverId` | the server identifier, or `null` if it has not been generated or is invalid (call `getServerId` to see why) |
| `build` | target platform and build profile of the plugin binary |

Errors:

- `STASH_UNREACHABLE`: the plugin could not call back into Stash. `error.details` still holds all
  the local fields above.
- `STASH_TOO_OLD`: the server is older than `stashMinimumVersion`. `error.details` holds the same
  fields plus `stashVersion`.

### `getServerId`

Returns this server's permanent identifier. It never creates one.

- Arguments: none
- Result: `{ "serverId": "b8b41b17-046a-4bf6-9862-1d120f081263" }`
- Errors: `SERVER_ID_NOT_GENERATED`, `SERVER_ID_INVALID`, `STASH_UNREACHABLE`
- Since: 1.0.0

## Tasks

### Generate server identifier (`generateServerId`)

Creates the server identifier if this server does not have one, and otherwise leaves it alone.
Run it once per server; running it again is safe.

- Run with: `runPluginTask(plugin_id: "semantic-engine", task_name: "Generate server identifier")`
- Outcome, written to Stash's log: `serverId=<uuid> (generated)` the first time,
  `serverId=<uuid> (existing)` afterwards, or an error line with its code.
- It can also be called as an operation, `args: { op: "generateServerId" }`, which returns
  `{ "serverId": "<uuid>", "outcome": "generated" | "existing" }`.
- Errors: `SERVER_ID_INVALID` (a value is stored but is not a valid identifier; it is reported and
  never overwritten), `STASH_UNREACHABLE`
- Since: 1.0.0

## The server identifier

Every Stash server gets one identifier, a lower-case hyphenated UUID such as
`b8b41b17-046a-4bf6-9862-1d120f081263`. Clients use it to build the same identifiers for
scenes, performers, tags, and other items, so their data lines up (vocabulary item IRIs:
`urn:semantic-stash:{serverId}:{type}:{stashId}`, see
[`specs/001-semantic-vocabulary/data-model.md`](../specs/001-semantic-vocabulary/data-model.md)).

You can read it **without the engine**, straight from Stash's configuration:

```graphql
query {
  configuration { plugins(include: ["semantic-engine"]) }
}
```

The value is at `semantic-engine.serverId` in the returned map. If it is missing, ask the server
owner to run the "Generate server identifier" task; never generate one yourself, or two clients
would disagree.

## Vocabulary terms

Results that refer to the semantic model use the terms of the semantic vocabulary by their IRIs
in the namespace `urn:semantic-stash:vocab:` (for example `urn:semantic-stash:vocab:Role`). The
terms, their definitions, and their scopes are in [`vocabulary/TERMS.md`](../vocabulary/TERMS.md).
This document never redefines them. Contract 1.0.0 has no operation that returns vocabulary
terms yet.

## Error codes

| Code | Meaning |
|---|---|
| `INVALID_INPUT` | Stash's input to the plugin was missing, not JSON, or had no connection details |
| `UNKNOWN_OPERATION` | `args.op` is missing or not an operation; `error.details.known` lists the names |
| `STASH_UNREACHABLE` | the plugin could not call back into Stash's GraphQL API |
| `STASH_TOO_OLD` | the server is older than the minimum supported Stash version |
| `SERVER_ID_NOT_GENERATED` | the server has no identifier yet |
| `SERVER_ID_INVALID` | the stored identifier is not a lower-case hyphenated UUID |
| `INTERNAL` | an unexpected failure inside the plugin; the message says what failed |

## Compatibility

- Adding an operation, a task, an optional argument, a result field, or an error code changes the
  MINOR version. Clients must ignore result fields they do not know.
- Removing or renaming any of these, or changing a type, changes the MAJOR version.
- This document, `api/operations.json`, and the plugin change in the same revision; the agreement
  test fails the build otherwise.
- `status` reports `contractVersion`, so a client can check it before relying on newer features.

## Changes

### 1.0.0 (2026-10-16)

First contract: operations `ping`, `status`, and `getServerId`; task "Generate server identifier"
(`generateServerId`); the result envelope; error codes `INVALID_INPUT`, `UNKNOWN_OPERATION`,
`STASH_UNREACHABLE`, `STASH_TOO_OLD`, `SERVER_ID_NOT_GENERATED`, `SERVER_ID_INVALID`, and
`INTERNAL`.

Every change to the operations or error codes adds an entry here, in the same revision as the
matching changes to `api/operations.json` and the plugin.
