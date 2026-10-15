# Data Model: Engine Plugin Foundation

## Plugin input (from Stash, stdin)

| Field | Type | Notes |
|---|---|---|
| `server_connection.Scheme` | string | `http` or `https` |
| `server_connection.Host` | string | `0.0.0.0` or `::` means use `localhost` |
| `server_connection.Port` | integer | |
| `server_connection.SessionCookie` | object or null | `Name`, `Value`, …; never logged |
| `server_connection.Dir` | string | Stash config directory |
| `server_connection.PluginDir` | string | this plugin's directory |
| `args` | object | operation arguments; `op` selects the operation or task |

Validation: missing or malformed input yields an envelope error `INVALID_INPUT` (or a plain
`error` string when no envelope can be produced) and exit code 0.

## Response envelope (stdout `output`)

| Field | Type | Rule |
|---|---|---|
| `ok` | boolean | true on success |
| `op` | string | the operation name as received |
| `contractVersion` | string | MAJOR.MINOR.PATCH of the operation contract |
| `result` | object | present when `ok` is true |
| `error` | object | present when `ok` is false: `code` (string, from the error list), `message` (string), `details` (object, optional) |

The top-level Stash `error` is null whenever an envelope is produced.

## Server identifier

| Attribute | Rule |
|---|---|
| Location | Stash configuration `plugins.settings.semantic-engine.serverId` |
| Format | lower-case hyphenated UUID, regex `^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$` |
| Lifecycle | absent → generated once by `generateServerId` → permanent; never overwritten by the plugin |
| Malformed value | reported as `SERVER_ID_INVALID`; not overwritten |

State transitions: `absent` --task--> `present`; `present` --task--> `present` (unchanged,
reported as existing); `invalid` --task--> `invalid` (reported, unchanged).

## Versions reported by `status`

| Field | Source |
|---|---|
| `engineVersion` | plugin crate version |
| `contractVersion` | operation contract (starts at `1.0.0`) |
| `vocabularyVersion` | `sem:version` of the embedded vocabulary |
| `oxigraphVersion` | `oxigraph` dependency version |
| `rocksdbVersion` | bundled RocksDB version embedded at build time |
| `stashMinimumVersion` | `v0.31.1` |
| `stashVersion` | from the server, when reachable |
| `build` | target triple, git commit (when available), build profile |

## Release package

| Item | Rule |
|---|---|
| Name | `semantic-engine-<version>-<target>.tar.gz` (Unix) or `.zip` (Windows) |
| Contents | `semantic-engine/semantic-engine.yml`, `semantic-engine/semantic-engine[.exe]`, `semantic-engine/vocabulary/*`, `semantic-engine/VERSIONS.json` |
| Integrity | one line per package in `SHA256SUMS` |

## Fixture set

| Item | Rule |
|---|---|
| Files | one JSON file per query, named by query, under the chosen directory |
| Scrubbing | no cookie, header, or API key values; checked by a test |
| Report | `index.json` listing each query, item counts, and which were empty |
