# Research: Engine Plugin Foundation

Sources: the Stash v0.31.1 source tree (`~/Projects/Stash-Plugins/Stash-Source/stash-0.31.1`),
read 2026-10-15, cited by file below; the Oxigraph 0.4 / `oxrocksdb-sys` 0.4.11 crates from the
local Cargo registry; and a probe build of Oxigraph on this machine (2026-10-15).

## R1. How Stash runs a raw-interface plugin

- **Decision**: Use Stash's `raw` plugin interface (the default when `interface` is omitted). The
  plugin is one executable; Stash spawns it per call, writes one JSON object to its stdin, reads
  stdout to the end, and reads stderr as log lines.
- **Facts** (`pkg/plugin/raw.go`, `pkg/plugin/common/msg.go`, `pkg/plugin/plugins.go`):
  - stdin: `{"server_connection": {"Scheme", "Host", "Port", "SessionCookie": {cookie}, "Dir",
    "PluginDir"}, "args": {...}}`. Field names are Go-cased exactly as shown.
  - stdout: `{"error": string|null, "output": any}`. If stdout is not that JSON, Stash treats the
    whole of stdout as `output`. A non-zero exit with no `error` makes the exit error the error.
  - `runPluginOperation(plugin_id, args)` returns `output` directly; if `error` is set, Stash
    returns a GraphQL error whose message is the `error` string only (structure is lost).
  - `runPluginTask(plugin_id, task_name, args_map)` queues a job and returns a job id; the task's
    `output` is not returned to the caller. Tasks report through logs and progress.
  - The plugin id is the manifest file name without `.yml` (`config.go` `loadPluginFromYAMLFile`).
  - `exec[0]` is looked up on `PATH`, then relative to the plugin directory; on Windows the `.exe`
    extension is optional. `{pluginDir}` in later arguments is replaced with the plugin
    directory. The working directory is Stash's, not the plugin's (`config.go`
    `getExecCommand`).
  - Task `defaultArgs` (string map) and `execArgs` come from the manifest (`OperationConfig`).
- **Consequence for the response envelope**: structured errors (code, message, operation) MUST
  travel inside `output`, because Stash flattens `error` to a string. `error` is reserved for
  failures that prevent the plugin from producing an envelope at all.
- **Alternatives considered**: the `rpc` interface (Go net/rpc; no Rust implementation and no
  benefit for request-response); a long-lived process (Phase 1 spike).

## R2. Calling back into Stash from the plugin

- **Decision**: The adapter connects to `{Scheme}://{Host}:{Port}/graphql` from
  `server_connection`, sending the `SessionCookie` as a cookie. When `Host` is `0.0.0.0` or
  `::` (Stash listening on all interfaces) it connects to `localhost` instead.
- **Facts**: Stash's own Go plugin client builds exactly this URL and cookie jar
  (`pkg/plugin/util/client.go`). The cookie is a session made for the calling user
  (`pkg/session/plugin.go` `MakePluginCookie`), so the plugin needs no API key.
- **Rationale**: Principle IV (the only host contacted is the Stash server that started the
  plugin); credentials never enter plugin configuration or logs.

## R3. GraphQL client and pinned schema (constitution: Cynic)

- **Decision**: `cynic` with its blocking `reqwest` transport, `reqwest` built with `rustls`
  (no OpenSSL, no extra C dependency). The schema is pinned as one SDL file,
  `schemas/stash-v0.31.1.graphql`, produced by concatenating `graphql/schema/schema.graphql` and
  `graphql/schema/types/*.graphql` from the v0.31.1 source, and registered with
  `cynic-codegen` in the adapter's build script. Custom scalars used by the operations here
  (`Map`, `Any`, `PluginConfigMap`) are declared as JSON values.
- **Rationale**: typed operations checked at compile time against the exact schema of the
  minimum supported Stash; a schema bump is one reviewed file change.
- **Alternatives considered**: introspection at build time (needs a live server in CI);
  `graphql_client` (constitution names Cynic).

## R4. Where the server identifier lives

- **Decision**: As a plugin setting of this plugin: `plugins.settings.semantic-engine.serverId`
  in Stash's configuration, declared in the manifest as a `STRING` setting named `serverId`.
  Read with `configuration { plugins(include: ["semantic-engine"]) }`, written with
  `configurePlugin(plugin_id: "semantic-engine", input: {...})`.
- **Facts** (`internal/manager/config/config.go`, `internal/api/resolver_mutation_configure.go`):
  plugin settings are stored under `plugins.settings.<pluginId>`; `configurePlugin` **replaces
  the whole map** for the plugin, so a writer MUST read the current map, add `serverId`, and
  write the merged map. `configuration.plugins(include:)` returns the map to any authenticated
  client without the plugin running (FR-008).
- **Concurrency**: Stash runs queued jobs one at a time in its job manager, so two identity tasks
  do not run concurrently. As a guard, the task re-reads after writing and reports the stored
  value; if another value is already present it never overwrites it.
- **Alternatives considered**: a tag or `custom_fields` on some entity (pollutes library data);
  the plugin's own file (not readable by clients without the engine; breaks FR-008).

## R5. Logging and progress

- **Decision**: Write log lines to stderr with Stash's level prefix: byte `0x01`, a level
  character (`t` trace, `d` debug, `i` info, `w` warning, `e` error, `p` progress), byte `0x02`,
  then the message (`pkg/logger/plugin.go`). Progress lines carry a number from 0 to 1. Logs
  never include the session cookie or any setting value marked secret.

## R6. Bundled RocksDB and its version (constitution v2.0.0)

- **Decision**: Depend on `oxigraph` 0.4 with default features (bundled RocksDB through
  `oxrocksdb-sys`, compiled by the `cc` crate; no CMake). The engine crate's build script reads
  the bundled RocksDB version from `rocksdb/include/rocksdb/version.h` inside the
  `oxrocksdb-sys` source (located through Cargo metadata of the dependency) and embeds it, so the
  status operation reports it (FR-022).
- **Facts**: probe build on 2026-10-15: first release build 221 s on 12 cores, 12 MB binary, only
  `libstdc++` linked dynamically; the bundled RocksDB is 10.3.0. Building against an installed
  RocksDB 11.1 failed (C++20 headers, then a changed `OpenForReadOnly` signature).
- **Build requirements**: Rust stable, a C++17 compiler, and clang (libclang for `bindgen`), on
  build machines and CI only.

## R7. Platforms and packages

- **Decision**: Release packages (tar.gz for Unix, zip for Windows) for
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`,
  `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`, each with a `SHA256SUMS` entry. Linux
  packages are built on the oldest supported Ubuntu runner for the widest glibc compatibility.
  BSD and 32-bit ARM have no package; the install script says so (FR-019).
- **Package integrity**: the install scripts verify the package against the published SHA-256
  checksum before unpacking and stop on a mismatch. (Cheap protection for a downloaded
  executable; recorded here because the spec left integrity open.)
- **Alternatives considered**: musl static builds (RocksDB and libstdc++ static linking adds
  build complexity); cross-compiling every target from Linux (macOS and Windows SDK licensing and
  C++ toolchains make native runners simpler).

## R8. Install scripts

- **Decision**: `scripts/install.sh` (POSIX `sh`, checked with `shellcheck -s sh` and run under
  `dash` in CI) and `scripts/install.ps1` (PowerShell 5.1 compatible). Both accept
  `--plugins-dir`/`-PluginsDir`, `--version`/`-Version`, and `--from-file`/`-FromFile` (install a
  package already downloaded, for offline machines and tests).
- **Plugins folder discovery**, in order: the explicit argument; `plugins_path` read from the
  Stash config file (`./config.yml` then `~/.stash/config.yml`, Stash's own search order in
  `internal/manager/config/init.go`; a relative `plugins_path` is resolved against the config
  file's directory); then `<config dir>/plugins`, Stash's default
  (`GetDefaultPluginsPath`). If none exists the script stops and asks for `--plugins-dir`.
- **Install layout**: `<plugins>/semantic-engine/` containing `semantic-engine.yml`, the
  executable, and `vocabulary/`. Re-running replaces those files only; Stash's configuration
  (including `serverId`) is never touched.
- **Downloads**: only from the project's release page; nothing else is contacted (FR-020).

## R9. API document and its check

- **Decision**: `docs/api.md` is the human document; `contracts/operations.json` (in the repo at
  `api/operations.json` after implementation) is the machine list of operations, error codes, and
  the contract version. A test asserts the plugin's operation registry, the JSON list, and the
  headings in `docs/api.md` agree (FR-012).

## R10. Fixture capture tool

- **Decision**: A `fixture-capture` binary in the adapter crate that runs only Cynic query
  operations (no mutation types are compiled into it), takes the server URL and API key from
  environment variables, writes JSON responses under a chosen directory, and scrubs any header
  or cookie value from the files. It reports empty sections.
