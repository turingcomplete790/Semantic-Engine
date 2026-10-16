# Semantic Engine Roadmap

A Rust Stash plugin that holds the semantic, RDF-style graph for a Stash server and answers
SPARQL-style queries over it, shared by the
[Semantic Stash Viewer](../../Semantic-Stash-Viewer/) and the
[semantic-tagging](../semantic-tagging/) plugin (and its semantic-search plugin). Both clients have
their own engine on their roadmaps today; this project replaces that duplicated effort with one.

Governing rules live in the [constitution](.specify/memory/constitution.md). Sources:

- Viewer: `ROADMAP.md` Phase 6 (shared engine cache) and `specs/009-semantic-engine`.
- Plugin: `semantic-query-engine-roadmap.md` (Milestones 1D, 2–8), `README.md`,
  `archetype-system-plan.md`.

Each phase is a Spec Kit feature (or a few). Tick items off when their spec is implemented and
link the spec folder (`specs/NNN-name/`) next to the item.

## Phase 0: Foundations

**Goal:** a project that builds, tests, and installs as an empty Stash plugin.

- [x] Constitution (now v2.0.0)
- [x] Cargo workspace: `semantic-vocabulary`, `stash-adapter` (the only Stash I/O, Cynic with the
      pinned v0.31.1 schema), `engine`, and `plugin` (the `semantic-engine` executable)
      (`specs/002-engine-plugin-foundation/`)
- [x] Server identity task: "Generate server identifier" stores a UUID once as the plugin setting
      `serverId`, read by every client as the server part of item identifiers
      (`specs/001-semantic-vocabulary/` FR-014, `specs/002-engine-plugin-foundation/` US2)
- [x] Plugin manifest and operations Stash can invoke (`runPluginOperation`): `ping`, `status`,
      `getServerId` (`specs/002-engine-plugin-foundation/` US1, US2)
- [x] RocksDB bundled through Oxigraph's default build (constitution v2.0.0): plain Cargo build,
      no CMake, no system RocksDB; packages are self-contained and the status operation reports
      the bundled RocksDB version
- [x] Install scripts: POSIX shell for BSD, macOS, and Linux (tested under dash and busybox sh),
      and PowerShell for Windows; both verify the package's SHA-256 checksum
- [x] CI and releases (`.github/workflows/ci.yml`, `release.yml`): format, clippy, tests, script
      tests, builds on Linux, macOS, and Windows; v0.1.0 published with five packages and
      `SHA256SUMS`
- [x] Fixture tooling: `fixture-capture` records read-only, credential-free fixtures; fixtures
      from the test instance in `crates/stash-adapter/tests/fixtures/test-instance/`
- [x] Semantic vocabulary as a set of RDF documents in Turtle (`.ttl`), derived from the
      semantic-tagging design documents (`README.md`, `semantic-query-engine-roadmap.md`,
      `archetype-system-plan.md`): classes, properties, tag-type flags, scopes, Variations,
      Labels, and archetypes, each term citing the document section and plugin commit (or date)
      it was read at (constitution Principle II). The vocabulary is the contract the engine
      builds its graph from (Phase 2). Done: `vocabulary/` and `specs/001-semantic-vocabulary/`
- [x] API document for clients (the viewer and the plugin): the operations, request and response
      shapes, errors, and versions (constitution Principle VI), kept in step with the plugin as
      it is built and updated whenever an operation or the vocabulary changes. It refers to
      terms in `vocabulary/` by IRI (`urn:semantic-stash:vocab:`), never by redefining them.
      Done: `docs/api.md`, `api/operations.json`, and the agreement test (contract 1.0.0)

## Phase 1: Spike (settle Principle IV's open questions)

**Goal:** measured answers before any feature spec. Output is a written decision record.

- [ ] Process model: per-invocation process start cost vs a long-lived process Stash keeps alive
      (baseline from 002, Stash v0.31.1, local: process start ~4 ms, `ping` ~9 ms and `status`
      ~21 ms through `runPluginOperation`, first call after a plugin reload ~1.3 s)
- [ ] Persistence: on-disk Oxigraph store reopened per call; single-writer lock, read-only
      readers, or snapshot-only
- [ ] Freshness: hook delivery when a tag, scene, image, or performer is saved from the web UI,
      the plugin, or the viewer
- [ ] Result limits: the largest result `runPluginOperation` can return, and paging when it is
      exceeded
- [ ] Snapshot delivery: operation result vs the plugin's asset folder Stash serves; how a
      client catches up from it
- [ ] Build time and size against the viewer's 009 budgets (SC-001)
- [ ] Decision: proceed, or fall back to clients keeping in-process engines only

## Phase 2: Semantic graph (engine core)

**Goal:** the graph the design documents define, built from Stash, headless and testable.

- [ ] Stash adapter crate: the one place that talks to Stash, using the Cynic GraphQL library
      with typed operations generated against a pinned copy of Stash's schema (constitution
      v1.1.0, Technology & Platform Constraints)
- [ ] Oxigraph embedded behind an engine-owned interface, its on-disk store on the bundled
      RocksDB (Oxigraph's default build; never a system RocksDB)
- [ ] Vocabulary from the documents ("The semantic graph is the abstraction boundary" and its
      vocabulary corrections; Milestone 1D): Scene and Image media entities, performers, the six
      flagged tag types plus Variations, reified Role and Action instances, tag categories and
      variations, parent tags, Label formulas, archetype parents
- [ ] Data contract read exactly as documented: per-type flags (`plugtag`, `isAction`, `isRole`,
      `isTrait`, `isLabel`, `isMiseEnScene`, `isVariation`), `variantOfTagId`, scene- and
      image-scoped assignments, performer-global Traits and Labels
- [ ] Bounded, flag-filtered discovery (constitution Principle VII)
- [ ] Incremental build with progress and cancellation; store rebuilt when missing, damaged, or
      from an older format
- [ ] Compatibility fixtures: a plugin-tagged library produces the documented graph

## Phase 3: Query operations (Milestone 2)

**Goal:** clients can query the graph through Stash.

- [ ] Versioned operation protocol (Principle VI): request, response, structured errors
- [ ] SPARQL `SELECT`: triple patterns, variables, joins, projection, basic filters
- [ ] Scoping rules enforced (scene-scoped predicates only join within one scene)
- [ ] Paging and result-size limits per Phase 1 findings
- [ ] Hook-driven incremental updates, with a manual rebuild task
- [ ] Status operation: engine version, store format, last update, graph counts

## Phase 4: Client adoption

**Goal:** the viewer and the plugin use the engine, each still working without it.

- [ ] Viewer: the engine as an optional companion, falling back to its in-process engine
      (viewer Phase 6; needs the viewer's constitution amendment first)
- [ ] Plugin: the semantic-search plugin calls the engine through `runPluginOperation`
- [ ] Cross-client equivalence tests: the same library state gives the same answers in each
- [ ] Remove or retire duplicated engine work in the clients' roadmaps

## Phase 5: Richer queries and rules (Milestones 3–6)

**Goal:** the documents' query language beyond `SELECT`. Each item extends the one engine.

- [ ] Boolean logic and more SPARQL constructs ("Query Features: Phase 2 and 3")
- [ ] Label evaluation and "why does this performer have this Label?" explanations
- [ ] Archetype inheritance as queries
- [ ] `CONSTRUCT`: preview derived facts, apply derived tags through Stash GraphQL, record
      provenance
- [ ] Saved queries and rules, manual and scheduled execution, explaining derived tags

## Phase 6: Optimisation (Milestone 7)

- [ ] Profile evaluation; join ordering; query caching where it pays
- [ ] Reduce store size and memory footprint
- [ ] Snapshot export so a client can start from the engine's state instead of building its own

## Out of scope

- Any UI. The viewer and Stash's web UI own presentation.
- A listening network service; the engine is reached only through Stash.
- Scrapers and ordinary GraphQL response caching (Stash is already fast; only the semantic
  index is costly to build).
- Replacing or becoming the source of truth for any Stash data.

## Maintaining this roadmap

- Reordering phases is fine. Changes that conflict with the constitution need an amendment first.
- When the plugin's design documents change a feature or data structure, update the affected
  phase and note the plugin commit read (constitution Principle II).
- Keep this roadmap and the viewer's Phase 6 in step; when one changes, update the other.
