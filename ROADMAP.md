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

Done 2026-10-20: [specs/003-engine-design-spike/decision-record.md](specs/003-engine-design-spike/decision-record.md) (outcome `proceed-with-changes`).

- [x] Process model: one process per call, reopening the published snapshot read-only. Query p95
      88.7 ms at 250k scenes; the long-lived helper is dropped, since it saves about 10 ms
      (decision record §1)
- [x] Persistence: on-disk store with published snapshot generations and a single writer. Readers
      of published snapshots never saw a partial write; read-only readers of the live store did
      in 72% of reads (§2)
- [x] Freshness: hooks fire for every save from every client, adding about 5 ms per saved item. A
      hook records the change and the next engine call applies it (§3)
- [x] Result limits: pages of 1 MiB, about 8,600 bindings; the hard ceiling is 2 MiB. Time grows
      by about 75 ms per MiB, and no call failed up to 16 MiB (§4)
- [x] Snapshot delivery: not needed. Clients query the engine and hold no snapshot
      (constitution v3.0.0, Principle III). Measured for reference: 186 s and 74 MB at 250k (§5)
- [x] Build time and size: within the viewer's 009 budgets. First build estimated at 30–40 s at
      Production size; release archives 1.8–2.2 MB (§6)
- [x] Decision: proceed with the shared engine, with constitution v3.0.0, the reworded Phase 4,
      and the Phase 2 rules in the record (§7)

## Phase 2: Semantic graph (engine core)

**Goal:** the graph the design documents define, built from Stash, headless and testable.

- [x] Relations, role and relation hierarchies, and auto-apply on save, with vocabulary 1.2.0
      and contract 1.1.0 (`specs/004-relations-role-hierarchy`): the save hook records changes and
      a background job applies them; "Re-apply relations" for definition changes
- [x] Stash adapter crate: the one place that talks to Stash, using the Cynic GraphQL library
      with typed operations generated against a pinned copy of Stash's schema (constitution
      v1.1.0, Technology & Platform Constraints). Done as the library reader
      (`specs/005-stash-adapter`):
      - a full read with progress and stopping, change reads, and id listings;
      - six distinct failure kinds, and a `MemoryReader` for tests;
      - vocabulary 1.3.0 (Field Mapping, derived measurements).
      Production full read: 177 ms for its 1,215 semantic records, in 8 MB (2026-10-25).
- [x] Oxigraph embedded behind an engine-owned interface, its on-disk store on the bundled
      RocksDB (Oxigraph's default build; never a system RocksDB). Done as `engine::graph`
      (`specs/006-graph-store`): at 250k scenes the open median is 6.3 ms and the query call
      p95 71.7 ms
- [x] Vocabulary from the documents ("The semantic graph is the abstraction boundary" and its
      vocabulary corrections; Milestone 1D): Scene and Image media entities, performers, the six
      flagged tag types plus Variations, reified Role and Action instances, tag categories and
      variations, parent tags, Label formulas, archetype parents. Done as `engine::build` and the
      task "Build semantic graph" (`specs/007-graph-build`, contract 1.2.0, vocabulary 1.4.0):
      a synthetic library of Production size (50,000 scenes, 29,000 images, 4.3 million facts)
      builds in 40 s with a 1.6 GB peak
- [x] Data contract read exactly as documented: per-type flags (`plugtag`, `isAction`, `isRole`,
      `isTrait`, `isLabel`, `isMiseEnScene`, `isVariation`), `variantOfTagId`, scene- and
      image-scoped assignments, performer-global Traits and Labels. Done in `specs/007-graph-build`:
      every key's facts are in its mapping rules; bad fields are skipped or noted, never fatal
- [x] Bounded, flag-filtered discovery (constitution Principle VII). Done in specs 005 and 008:
      one filtered query per key, then records by id, 250 per request (40 requests for 905
      semantic records on the test instance); updates read in proportion to what changed
- [x] Incremental build with progress and cancellation; store rebuilt when missing, damaged, or
      from an older format. Done as "Update semantic graph" (`specs/008-incremental-graph-update`,
      store format 2, contract 1.3.0): every update equals a full rebuild. At 250k scenes, one
      changed scene takes 4.6 s and 1,000 take 17.9 s. Starting updates from save hooks stays
      with Phase 3's "Hook-driven incremental updates"
- [x] Store with published snapshot generations, a single writer, and readers of published
      generations only (constitution Principle IV; decision record rules 1–3). Done in
      `specs/006-graph-store`: 0 partial reads in 16,492 multi-process reads at 250k. A
      publish there takes about 3.8 s, to measure again for incremental updates
- [x] Compatibility fixtures: a plugin-tagged library produces the documented graph. Done in
      `specs/007-graph-build`: a hand-written library and its full expected graph, a check that
      every active mapping entry is exercised, and the captured test-instance library

## Phase 3: Query operations (Milestone 2)

**Goal:** clients can query the graph through Stash.

- [ ] Versioned operation protocol (Principle VI): request, response, structured errors
- [ ] SPARQL `SELECT`: triple patterns, variables, joins, projection, basic filters
- [ ] Relation and Role criteria accept `recursive` and expand through the engine's hierarchy
      (`Hierarchy::expand_criterion`, 004 US3); verify 004 SC-004 (exact matches) and SC-007
      (p95 under 200 ms at 250k scenes) here
- [ ] Scoping rules enforced (scene-scoped predicates only join within one scene)
- [ ] Paging and result-size limits per Phase 1 findings: 1 MiB pages, never over 2 MiB
      (decision record rule 9)
- [ ] Hook-driven incremental updates: hooks append to a journal and the next engine call
      applies it (003 decision record, rules 4–8), with a manual rebuild task
- [ ] Status operation: engine version, store format, last update, graph counts

## Phase 4: Client adoption

**Goal:** the viewer and the plugin are user interfaces over the engine. Their semantic features
query it; without it they say clearly that it is missing (constitution Principles I and III).

- [ ] Viewer: semantic features query the engine, with a clear state when it is absent; no
      in-process engine (viewer Phase 6; needs the viewer's constitution amendment first)
- [ ] Plugin: the semantic-search plugin calls the engine through `runPluginOperation`
- [ ] Plugin: tag pickers take recommendations from the engine's `recommendations` operation
      and retire `computeRecommendedTagIds` (004 US4; `specs/004-relations-role-hierarchy/plugin-notes.md`)
- [ ] Plugin: adopt Relations, role hierarchy, and auto-apply in its design documents (004
      plugin notes, constitution Principle II)
- [ ] Cross-client equivalence tests: the same library state gives the same answers in each
- [ ] Remove or retire duplicated engine work in the clients' roadmaps

## Phase 5: Richer queries and rules (Milestones 3–6)

**Goal:** the documents' query language beyond `SELECT`. Each item extends the one engine.

- [ ] Boolean logic and more SPARQL constructs ("Query Features: Phase 2 and 3")
- [ ] Label evaluation and "why does this performer have this Label?" explanations. The
      derived measurement fields that field rules can name are already read (005 FR-002b). Still
      to come: a bounded "performer ids matching this criterion" read for the other field rules,
      which Stash's own filter resolves (005 research R2).
- [ ] Archetype inheritance as queries
- [ ] `CONSTRUCT`: preview derived facts, apply derived tags through Stash GraphQL, record
      provenance
- [ ] Saved queries and rules, manual and scheduled execution, explaining derived tags

## Phase 6: Optimisation (Milestone 7)

- [ ] Profile evaluation; join ordering; query caching where it pays
- [ ] Reduce store size and memory footprint
- ~~Snapshot export so a client can start from the engine's state~~: dropped. Clients query the
  engine and hold no snapshot (constitution v3.0.0, Principle III; 003 decision record §5)

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
