# Semantic Engine

A [Stash](https://github.com/stashapp/stash) plugin, written in Rust, that holds the semantic,
RDF-style data for a Stash server and answers SPARQL-style queries over it.

The [Semantic Stash Viewer](../../Semantic-Stash-Viewer/) and the
[semantic-tagging](../semantic-tagging/) plugin (and its semantic-search plugin) each planned
their own engine. This project is the one shared engine they can both use, so the semantic model
is built, indexed, and queried in one place with one set of answers.

> **Status: early.** The plugin installs into Stash and answers `ping`, `status`, and
> `getServerId`, and gives each server a permanent identifier. The semantic graph and queries are
> not built yet. See the [roadmap](ROADMAP.md) for what exists and what comes next, and the
> [API document](docs/api.md) for what clients can call today.

## How it fits

- **Stash stays the system of record.** The engine's graph and index are a derived cache. Delete
  them and they are rebuilt from Stash; nothing in them is authoritative.
- **Reached only through Stash.** Clients call the engine with Stash's plugin operations and
  tasks. The engine opens no listening port and contacts no host except the Stash server that
  started it.
- **One vocabulary.** The terms the engine reads and writes are defined once, in Turtle, in
  [`vocabulary/`](vocabulary/), derived from the semantic-tagging design documents.
- **Governed by a constitution.** The rules every change is checked against are in
  [`.specify/memory/constitution.md`](.specify/memory/constitution.md).

## Requirements

| Requirement | Why |
|---|---|
| Stash **v0.31.1** or newer | `custom_fields` on tags, scenes, images, and performers |
| A platform with a prebuilt package, or a Rust toolchain to build from source | see [Installing](#installing) |

### Long-term cache

The engine keeps its derived index in an on-disk store so it is not rebuilt every time Stash
starts the plugin. The store uses RocksDB, which is **bundled**: it is compiled into the plugin,
so there is nothing extra to install. The store lives in the plugin's cache folder, one per Stash
server, and can be deleted at any time without losing data.

## Installing

Packages are self-contained (the RocksDB store is compiled in). Each release publishes one
package per target and a `SHA256SUMS` file:

| Platform | Target | Package |
|---|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Linux aarch64 | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.gz` |
| macOS Apple silicon | `aarch64-apple-darwin` | `.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `.zip` |

BSD and 32-bit ARM have no package yet; the install script says so and explains how to build
from source.

Two scripts install the plugin into Stash's plugins folder. They find the folder from Stash's
`config.yml` (`plugins_path`), or take it as an argument; they **verify the package's SHA-256
checksum** before changing anything; and they only ever touch `<plugins>/semantic-engine/`.

```sh
# BSD, macOS, Linux (POSIX sh: works with dash, busybox sh, and the BSD and macOS /bin/sh)
sh install.sh --from-file semantic-engine-0.1.0-x86_64-unknown-linux-gnu.tar.gz
sh install.sh --plugins-dir ~/.stash/plugins --version 0.1.0     # download a release
```

```powershell
# Windows (Windows PowerShell 5.1 or PowerShell 7)
.\install.ps1 -FromFile semantic-engine-0.1.0-x86_64-pc-windows-msvc.zip
.\install.ps1 -PluginsDir $HOME\.stash\plugins -Version 0.1.0
```

With `--from-file`/`-FromFile`, the `.sha256` file from the release must sit next to the package.
Downloads come from this project's
[GitHub releases](https://github.com/turingcomplete790/Semantic-Engine/releases);
`SEMANTIC_ENGINE_RELEASE_URL` overrides that, for example for a mirror.

After installing, reload plugins in Stash (Settings > Plugins > Reload plugins), then run the
**Generate server identifier** task once (Settings > Tasks > Plugin Tasks).

Exit codes: 0 installed, 2 bad arguments, 3 no package for this platform, 4 plugins folder not
found or not writable, 5 download or checksum failure, 6 unpack or copy failure.

## Building from source

You need a stable Rust toolchain, a C++17 compiler, and clang. RocksDB is compiled from source
as part of the build (no CMake); the first release build takes a few minutes.

```sh
cargo build --release
cargo test --workspace
sh scripts/package.sh            # dist/semantic-engine-<version>-<target>.tar.gz + .sha256
pwsh -File scripts/package.ps1   # Windows: dist\semantic-engine-<version>-<target>.zip + .sha256
```

On Windows, bindgen needs to find LLVM's `libclang` (set `LIBCLANG_PATH`, for example to
`C:\Program Files\LLVM\bin`).

## Continuous integration and releases

- `.github/workflows/ci.yml` runs on every push and pull request: formatting, clippy, the full
  test suite, the vocabulary checks, shellcheck, the POSIX install-script tests under `dash` and
  `busybox sh`, and the PowerShell tests; it also builds and tests on macOS and Windows.
- `.github/workflows/release.yml` runs on `v*` tags (the tag must equal the plugin version),
  builds every package above, and publishes them with `SHA256SUMS` as a GitHub release.

## Checking the vocabulary

The vocabulary is a set of RDFS-level Turtle files with a checker that enforces provenance,
scope, status, and compatibility rules.

```sh
cargo run -p semantic-vocabulary --bin vocab-check          # all rules
cargo run -p semantic-vocabulary --bin vocab-check -- --write-terms     # regenerate TERMS.md
cargo run -p semantic-vocabulary --bin vocab-check -- --write-snapshot  # at release
scripts/validate-ttl.sh vocabulary   # second parser (Raptor's rapper), skipped if absent
```

The vocabulary change log is [`vocabulary/CHANGELOG.md`](vocabulary/CHANGELOG.md) and the
generated term list is [`vocabulary/TERMS.md`](vocabulary/TERMS.md).

## Repository layout

```text
api/operations.json    machine-readable list of operations, tasks, and error codes
docs/api.md            API document for clients
plugin/                Stash plugin manifest (semantic-engine.yml)
schemas/               pinned Stash GraphQL schema used by the adapter
vocabulary/            Turtle vocabulary files, change log, generated terms, release snapshots
crates/
  semantic-vocabulary/ vocabulary loader, item IRI scheme, and the vocabulary checker
  stash-adapter/       the only code that talks to Stash (Cynic, pinned schema)
  engine/              engine logic: versions, server identifier
  plugin/              the `semantic-engine` executable Stash runs
scripts/               install, package, schema, and check scripts
specs/                 feature specifications, plans, and tasks (Spec Kit)
ROADMAP.md             phases and what is done
.specify/              Spec Kit scaffolding and the constitution
```

## Working on this project

Work follows [Spec Kit](https://github.com/github/spec-kit): specify, clarify, plan, tasks,
analyze, implement. Every plan is checked against the constitution. Stash is accessed only
through a single adapter built with the Cynic GraphQL library, with typed operations generated
against a pinned copy of Stash's schema.

## License

Not yet chosen.
