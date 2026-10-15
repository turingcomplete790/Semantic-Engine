# Semantic Engine

A [Stash](https://github.com/stashapp/stash) plugin, written in Rust, that holds the semantic,
RDF-style data for a Stash server and answers SPARQL-style queries over it.

The [Semantic Stash Viewer](../../Semantic-Stash-Viewer/) and the
[semantic-tagging](../semantic-tagging/) plugin (and its semantic-search plugin) each planned
their own engine. This project is the one shared engine they can both use, so the semantic model
is built, indexed, and queried in one place with one set of answers.

> **Status: early.** The semantic vocabulary and its checker are done. The plugin itself (the
> manifest, operations, install scripts) and the graph engine are not built yet. See the
> [roadmap](ROADMAP.md) for what exists and what comes next.

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
| **RocksDB** installed as a system package | the engine's long-term on-disk cache (see below) |
| A platform with a prebuilt package, or a Rust toolchain to build from source | see [Installing](#installing) |

### RocksDB (required)

The engine keeps its derived index in an on-disk store so it is not rebuilt every time Stash
starts the plugin. That store is built on RocksDB, which the engine **expects to find as an
installed package**; it is not bundled with the plugin. Install it before installing or building
the plugin.

| Platform | Install |
|---|---|
| Debian, Ubuntu | `sudo apt install librocksdb-dev` |
| Fedora | `sudo dnf install rocksdb rocksdb-devel` |
| Arch Linux, Manjaro | `sudo pacman -S rocksdb` |
| macOS (Homebrew) | `brew install rocksdb` |
| FreeBSD | `sudo pkg install rocksdb` |
| Windows (vcpkg) | `vcpkg install rocksdb:x64-windows` |

Package names and versions differ between distributions; use your platform's package manager to
find the RocksDB library and, if you build from source, its development files.

The store is a cache: it lives in the plugin's cache folder, one per Stash server, and can be
deleted at any time without losing data.

## Installing

> Planned. The install scripts below are specified
> ([`specs/002-engine-plugin-foundation`](specs/002-engine-plugin-foundation/spec.md)) but not
> written yet.

Two scripts will install the plugin into Stash's plugins folder:

- a **POSIX shell** script for BSD, macOS, and Linux, and
- a **PowerShell** script for Windows.

Each detects the platform and architecture, places the plugin files in the plugins folder, and is
safe to run again. If there is no prebuilt package for your platform it stops without changing
anything and tells you how to build from source.

## Building from source

You need a stable Rust toolchain and the RocksDB package above.

```sh
cargo build --release
cargo test -p semantic-vocabulary
```

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
vocabulary/            Turtle vocabulary files, change log, generated terms, release snapshots
crates/
  semantic-vocabulary/ loader, item IRI scheme, and the vocabulary checker
scripts/               helper scripts (second-parser check)
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
