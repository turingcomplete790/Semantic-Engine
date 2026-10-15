# Implementation Plan: Engine Plugin Foundation

**Branch**: `002-engine-plugin-foundation` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-engine-plugin-foundation/spec.md`

## Summary

Turn the repository into an installable Stash plugin with no semantic behaviour yet. A Rust
executable speaks Stash's raw plugin interface (JSON on stdin and stdout, logs on stderr),
answers `ping`, `status`, and `getServerId` operations inside a structured envelope, and runs a
`generateServerId` task that stores a UUID as a plugin setting in Stash. A Stash adapter crate
built with Cynic against a pinned v0.31.1 schema is the only code that talks to Stash. The engine
crate links Oxigraph with its bundled RocksDB so the status operation can report versions. The
feature also delivers the API document with an automated agreement check, a read-only fixture
capture tool, POSIX shell and PowerShell install scripts with checksum verification, and CI that
builds release packages for Linux, macOS, and Windows. Decisions and sources are in
[research.md](research.md).

## Technical Context

**Language/Version**: Rust stable (edition 2021); POSIX `sh` and PowerShell 5.1+ for install
scripts

**Primary Dependencies**: `cynic` (+ `cynic-codegen`), `reqwest` (blocking, `rustls`),
`serde`/`serde_json`, `uuid`, `thiserror`, `oxigraph` 0.4 (bundled RocksDB), the existing
`semantic-vocabulary` crate

**Storage**: Stash configuration (`plugins.settings.semantic-engine.serverId`); no engine store is
written in this feature (Oxigraph is linked and its version reported only)

**Testing**: `cargo test` with recorded GraphQL fixtures and a fake Stash HTTP endpoint in tests;
`shellcheck -s sh` and `dash` for the POSIX script; Pester-free PowerShell smoke tests run with
`pwsh`; an opt-in integration test against the local test instance (`STASH_TEST_URL`)

**Target Platform**: Stash v0.31.1+ on Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows
(x86_64); BSD supported by the install script's clear refusal and build-from-source instructions

**Project Type**: Stash plugin (CLI executable) plus library crates

**Performance Goals**: `status` and `ping` answered within 1 s of invocation on a local server
(SC-005), measured as a recorded baseline for the Phase 1 spike

**Constraints**: no listening socket, no host but the invoking Stash, no credential in logs or
files (Principle IV); plain Cargo build, no CMake (constitution v2.0.0); only Cynic typed
operations (constitution v1.1.0)

**Scale/Scope**: 3 operations, 1 task, 4 new crates or binaries, 2 install scripts, 1 CI pipeline
with 5 package targets

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Stash is the system of record | Pass | Only the server identifier is written, into Stash's own configuration; no engine data stored |
| II. Plugin documents are the contract | N/A | No semantic data read or written in this feature |
| III. One engine, one set of answers | Pass | Defines the single operation contract both clients use |
| IV. Reached only through Stash | Pass | Raw interface via Stash; connects only to `server_connection`; session cookie never logged (research R1, R2, R5) |
| V. Rust, embedded, headless | Pass | Library crates with no UI; plugin binary is a thin shell; adapter is the only Stash I/O |
| VI. Versioned, explicit contracts | Pass | Contract version in every status response; API document plus agreement check (FR-010 to FR-012) |
| VII. Fast, bounded, test-first | Pass | Tests first with recorded fixtures; status latency budget recorded (SC-005) |
| Technology: Cynic | Pass | Adapter uses Cynic with the pinned v0.31.1 schema (R3) |
| Technology: bundled RocksDB | Pass | `oxigraph` default features; no system RocksDB, no CMake (R6) |

No violations; Complexity Tracking not required.

**Re-check after Phase 1 design**: Pass. The envelope keeps structured errors inside `output`
(R1); the fixture tool compiles only query operations (R10); install scripts touch only the
plugin folder (R8).

## Project Structure

### Documentation (this feature)

```text
specs/002-engine-plugin-foundation/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── operations.md          # envelope, operations, task, error codes (source for docs/api.md)
│   ├── manifest.md            # semantic-engine.yml
│   └── install-scripts.md     # install.sh / install.ps1 interface and exit codes
├── checklists/requirements.md
└── tasks.md                   # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                         # workspace (exists)
schemas/
└── stash-v0.31.1.graphql          # pinned Stash schema (concatenated SDL)
crates/
├── semantic-vocabulary/           # exists (feature 001)
├── stash-adapter/                 # only code that talks to Stash (Cynic)
│   ├── build.rs                   # cynic-codegen schema registration
│   ├── src/lib.rs                 # client from server_connection, typed operations
│   ├── src/plugin_config.rs       # read/merge/write plugin settings
│   └── src/bin/fixture-capture.rs # read-only fixture tool
├── engine/                        # engine library: versions, server identity logic
│   ├── build.rs                   # embeds bundled RocksDB version
│   └── src/lib.rs
└── plugin/                        # the Stash plugin executable `semantic-engine`
    ├── src/main.rs                # stdin/stdout envelope, dispatch
    ├── src/ops.rs                 # operation and task registry
    └── src/log.rs                 # Stash log-level prefixes
plugin/
└── semantic-engine.yml            # manifest shipped in packages
api/
└── operations.json                # machine list of operations and error codes
docs/
└── api.md                         # API document for clients
scripts/
├── install.sh                     # POSIX sh: BSD, macOS, Linux
├── install.ps1                    # Windows PowerShell
├── package.sh                     # build a release package locally
└── validate-ttl.sh                # exists
.github/workflows/
├── vocabulary.yml                 # exists; folded into ci.yml
├── ci.yml                         # fmt, clippy, tests, checks, script linting
└── release.yml                    # packages + SHA256SUMS on tags
```

**Structure Decision**: Three new crates follow Principle V: `stash-adapter` (all Stash I/O),
`engine` (logic, no I/O of its own), and `plugin` (thin executable). The manifest and API list
live outside the crates because packages and clients use them directly.

## Complexity Tracking

No constitution violations to justify.
