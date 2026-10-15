# Semantic Engine Constitution

Semantic Engine is a Rust Stash plugin that holds the semantic, RDF-style data for a Stash
server and answers SPARQL-style queries over it. It exists so that the
[Semantic Stash Viewer](../../Semantic-Stash-Viewer/) and the
[semantic-tagging](../semantic-tagging/) plugin (and its future semantic-search plugin) share one
engine instead of each building, polling, and maintaining their own. It is the shared-engine
helper described in the viewer's ROADMAP Phase 6 and in the plugin's Milestone 8 ("Optional
Extraction"), delivered as a Stash plugin.

## Core Principles

### I. Stash Is the System of Record; the Engine Is a Cache

- Stash owns all library data. The engine MUST NOT hold authoritative data: every fact in its
  graph is derived from Stash and MUST be rebuildable from Stash alone.
- Every user-visible write (tag assignments, flags, `custom_fields`, derived tags) MUST be
  persisted to Stash through its GraphQL API. The engine's store is a derived index, one per
  Stash server, safe to delete at any time.
- The store MUST be rebuilt automatically when missing, damaged, or from an older format, and
  MUST never be shared across Stash servers.
- A stock Stash instance MUST keep working for every client without this plugin installed; the
  engine is an optimisation, never a requirement.

**Rationale**: Users trust Stash with their library. A second authoritative store creates sync
bugs and lock-in; a disposable cache costs time at worst, never data.

### II. The Plugin Design Documents Are the Contract (NON-NEGOTIABLE)

- The semantic model (vocabulary, scopes, flags, Variations, Labels, Archetypes, query features)
  is defined by the semantic-tagging design documents (`README.md`,
  `semantic-query-engine-roadmap.md`, `archetype-system-plan.md`). The engine builds what they
  describe and MUST NOT port the plugin's JavaScript implementation.
- The engine MUST read and write the same tag flags and `custom_fields` the documents define
  (`plugtag`, `isAction`, `isRole`, `isTrait`, `isLabel`, `isMiseEnScene`, `isVariation`,
  `variantOfTagId`, and the rest of their current list) so the plugin, the viewer, and the engine
  can work on one library concurrently without corrupting each other's data.
- Each feature spec touching semantic data MUST cite the document sections it builds on and the
  plugin commit (or date) it read them at. Where documents and code disagree, the documents win
  and the disagreement is noted for the plugin.
- Any divergence from the data contract MUST be deliberate, documented in the spec, and ship with
  a migration or compatibility shim.

**Rationale**: The viewer and the plugin are already converging on these documents. The engine is
only useful if both can trust it to mean the same thing by the same data.

### III. One Engine, One Set of Answers

- The engine MUST be the single implementation of semantic query semantics (SELECT first;
  CONSTRUCT and rules later, per the documents). Clients MUST NOT need their own matching path to
  get a correct answer.
- Semantic features (search, Matrix, Quick Pairings, recommendations, Label evaluation,
  archetype inheritance, explanations) MUST be expressed as queries or rules on the engine, not as
  hand-written filters. A missing fact is added to the engine, not worked around in a client.
- The same library state MUST give the same answers whether the caller is the viewer, a plugin's
  JavaScript, or a test.
- The engine MUST NOT contain UI or client-specific behaviour; client needs are met through
  general operations and queries.

**Rationale**: Duplicated engines were the reason this project exists. A second matching path
reintroduces divergent answers and wasted effort.

### IV. Reached Only Through Stash

- The engine is a Stash plugin and MUST integrate through Stash's plugin system: its plugin
  interface for operations and tasks, Stash's plugin hooks for change notification, and Stash's
  GraphQL for data. Clients reach it only via Stash (`runPluginOperation`, `runPluginTask`, or
  data Stash serves), so the network destination stays Stash.
- The engine MUST NOT open its own listening socket, connect to any host other than the Stash
  server that invoked it, or phone home.
- Stash credentials and the API key supplied by Stash MUST NOT be written to logs or stored
  outside what Stash already holds.
- Because plugin invocations may be short-lived processes, the design MUST state, per spec, how
  state persists between calls (on-disk store), how concurrent invocations are serialised (single
  writer, read-only readers), and how freshness is maintained from hooks. These are open design
  questions resolved by a spike and recorded in the spec before implementation.
- Failure MUST be graceful: if the engine is absent, stale, or errors, callers get a clear error
  or can fall back to their own build; the engine MUST NOT return silently wrong results.

**Rationale**: Stash has no change subscription and no side channel. Staying inside its plugin
system keeps the trust boundary and the deployment story unchanged.

### V. Rust, Embedded, and Headless

- The engine is written in Rust. The graph and SPARQL evaluation use **Oxigraph** embedded
  in-process (Redland only as a fallback, by amendment recording why), behind an engine-owned
  interface so the backend can be replaced without touching callers.
- Domain logic MUST live in library crates with no dependency on any UI toolkit, windowing, or
  network stack beyond the Stash adapter. The plugin executable is a thin shell over those
  libraries so the viewer MAY link the same crate in-process.
- Stash I/O MUST go through a single adapter module; the graph builder and query layer consume it
  and never call the network themselves.
- `unsafe` code and FFI MUST be isolated, justified in the plan's Constitution Check, and covered
  by tests.

**Rationale**: One crate shared by the plugin and the viewer is the cheapest way to guarantee one
set of semantics, and headless crates are testable without Stash or a GPU.

### VI. Versioned, Explicit Contracts

- The operation protocol (names, request and response JSON, error shapes) and any snapshot or
  store format MUST each carry an explicit version. Breaking changes MUST bump the version and
  clients MUST be able to detect a mismatch and degrade (rebuild, or fall back to their own
  engine) rather than misread.
- Operations MUST be documented in the spec's contracts, including result-size limits and
  paging, since plugin responses pass through Stash.
- Errors MUST be structured (code, message, offending query position where applicable), never
  free text only.

**Rationale**: Three independently released clients and one engine will drift unless the
boundary is explicit and checkable.

### VII. Fast, Bounded, and Test-First

- Data fetched from Stash MUST be bounded and flag-filtered: semantic vocabulary is found with
  "flag is not null" filters evaluated by Stash, never by loading the whole tag universe. No
  per-item request patterns (N+1) in builds or updates.
- Builds and updates MUST be incremental after the first build, report progress, and be
  cancellable; query evaluation MUST NOT block other operations longer than a stated budget.
  Spec-level budgets (process start, first build, incremental update, query latency on a
  representative library) MUST be written down and measured by a repeatable command.
- Tests come first for behaviour: contract tests for each operation and compatibility fixtures
  captured read-only from a library tagged by the plugin, so the engine's graph matches the
  documents' expectations. Tests MUST run without a live Stash using recorded GraphQL fixtures.
- Derived facts and rule outputs MUST be explainable ("why does this performer have this
  Label?") by the engine, with provenance recorded when applied through Stash.

**Rationale**: The engine is only worth its cost if it is faster than each client building the
index itself, and its answers are only trusted if they are checkable and reproducible.

## Technology & Platform Constraints

- **Language and toolchain**: Rust (stable), a Cargo workspace, `rustfmt` and `clippy` clean.
- **Engine**: Oxigraph (SPARQL 1.1 Query and Update) with an on-disk store in the plugin's
  cache location, one per Stash server profile.
- **Stash interface**: GraphQL via a single typed adapter; minimum supported Stash version
  follows the viewer (v0.31.1 or newer) unless a spec states otherwise.
- **Plugin packaging**: a Stash plugin manifest and a prebuilt executable per supported
  platform; Linux is primary, Windows and macOS where the toolchain allows.
- **Dependencies**: each new dependency MUST be justified in the plan; no network-calling
  dependencies beyond the Stash adapter.

## Development Workflow & Quality Gates

- Work follows Spec Kit: specify → plan → tasks → implement. Every plan MUST include a
  Constitution Check against Principles I–VII.
- Before a feature spec begins, open design questions (Principle IV) MUST be settled by a
  documented spike with measurements.
- Changes to the semantic contract MUST be coordinated with the plugin and viewer: the spec names
  the affected clients and the document sections changed.
- Pull requests MUST pass formatting, lint, and the full fixture test suite, and MUST state
  which principles they touch.
- Cross-project changes (viewer or plugin adopting the engine) are tracked in those projects'
  roadmaps; this repository MUST NOT depend on their internals.

## Governance

This constitution supersedes other practices in this repository. Amendments require a written
rationale, an update to this file with a Sync Impact Report, a version bump, and, where a
principle changes meaning, a migration note for dependent specs and the two client projects.

- **Versioning**: semantic. MAJOR for removing or redefining a principle in a backward
  incompatible way (for example changing the embedded engine or abandoning the Stash-only
  boundary); MINOR for a new principle or materially expanded guidance; PATCH for wording and
  clarification.
- **Compliance review**: every plan's Constitution Check and every pull request review MUST
  verify compliance. Violations MUST be justified in the plan's complexity table or the change
  is rejected.
- **Cross-project alignment**: amendments to Principles II, IV, or VI MUST be checked against
  the viewer's constitution (Principles I, II, VII, XII) and the plugin's documents for conflict,
  and the viewer's pending Phase 6 amendment wording is the reference for the shared-cache
  relationship.

**Version**: 1.0.0 | **Ratified**: 2026-10-14 | **Last Amended**: 2026-10-14
