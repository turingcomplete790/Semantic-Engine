# Contract: the `engine::graph` interface

This is the Rust interface the rest of the engine uses for its graph (FR-001 to FR-021). It is an
internal contract between modules, not part of the operation protocol, so the operations
contract stays **1.1.0** and `status` is unchanged.

The types are in [data-model.md](../data-model.md), and the decisions behind them in
[research.md](../research.md) (R1 to R9).

## Entry points

```rust
pub struct GraphStore { /* the store folder and the expected server id */ }

impl GraphStore {
    /// `dir` is `{PluginDir}/graph`; `server_id` is the plugin setting `serverId` (spec 002).
    pub fn new(dir: impl Into<PathBuf>, server_id: &str) -> GraphStore;

    /// Is the store usable, and if not, why (R4)? Never writes. `Err` only for `NoServerId`
    /// and file-system failures; every store state is a `Health`.
    pub fn check(&self) -> Result<Health, GraphError>;

    /// Opens the current generation read-only, for one call (FR-010, FR-011).
    pub fn reader(&self) -> Result<Reader, GraphError>;

    /// Takes the writer lock without waiting (FR-004). If the last writer stopped before
    /// publishing, it first resets the working store to the current generation (R3).
    pub fn writer(&self) -> Result<Writer, GraphError>;

    /// Added by spec 007 (research R3): the same as `writer()`, but starting from an empty
    /// working store, for a full rebuild. Published generations stay current until `publish()`.
    pub fn rebuild_writer(&self) -> Result<Writer, GraphError>;

    /// Empties a store that needs a rebuild, or any store, and stamps it for this server and
    /// format (FR-016). It takes the writer lock, so it gives `Busy` while a writer works.
    pub fn clear_for_rebuild(&self) -> Result<(), GraphError>;

    /// The report of US4 (FR-018). Never waits for a writer.
    pub fn summary(&self) -> Result<Summary, GraphError>;
}

impl Reader {
    pub fn generation(&self) -> &Generation;
    /// SPARQL `SELECT` only (FR-012, R7).
    pub fn select(&self, query: &str) -> Result<Solutions, GraphError>;
}

impl Writer {
    pub fn add(&mut self, facts: &[Fact]) -> Result<(), GraphError>;
    pub fn remove(&mut self, facts: &[Fact]) -> Result<(), GraphError>;
    pub fn remove_matching(&mut self, pattern: &Pattern) -> Result<u64, GraphError>; // facts removed
    pub fn clear(&mut self) -> Result<(), GraphError>;
    /// Publishes the working store as the next generation, then cleans up (FR-006, FR-017).
    pub fn publish(self) -> Result<Generation, GraphError>;
}
// Dropping a Writer without publishing leaves `work.dirty`: the next writer resets (FR-008).
```

## Behaviour every caller can rely on

| # | Rule | Requirement |
|---|---|---|
| G1 | **Readers only ever open a published generation**, never `work/`. | FR-010 |
| G2 | **A `Reader` answers from one generation for its whole life.** | FR-011 |
| G3 | **A `Writer` exists only while its process holds `writer.lock`.** A second `writer()` gives `Busy` in under 50 ms. | FR-004, SC-004 |
| G4 | **`publish` is atomic for readers.** It makes generation n+1 current with one rename. | FR-006, FR-007 |
| G5 | **A writer dropped without `publish`, or a crashed process, changes nothing readers see.** The next `writer()` starts from the current generation. | FR-008, FR-009 |
| G6 | **`reader()` and `writer()` first run `check()`, and refuse a store that needs a rebuild.** Such a store is never read or written; `clear_for_rebuild` is the way out. `summary()` reports the same state in its `health` field instead of failing, so `status` can show it. | FR-014, FR-015 |
| G7 | **`publish` keeps the current generation, the previous one, and those younger than `GENERATION_GRACE` (15 minutes).** A removal that fails is retried at the next publish. | FR-017 |
| G8 | **No call contacts a host or writes a credential.** | FR-002, FR-020 |
| G9 | **No `oxigraph` type appears in any signature above.** | FR-001 |

## Constants

| Name | Value | Meaning |
|---|---|---|
| `STORE_FORMAT` | `1` | the store layout version (R2) |
| `GENERATION_GRACE` | 15 minutes | the minimum age before an old generation is removed (R5) |
| `STORE_DIR` | `"graph"` | the folder under `PluginDir` |

## Who calls what (this feature and the next)

- **Graph builder (spec 007):** a full build calls `rebuild_writer()`, then `add`, then
  `publish`. On `NeedsRebuild` it calls `clear_for_rebuild()` first. Incremental updates (the
  next item) use `writer()`, then `remove_matching`, then `add`, then `publish`.
- **Query operations (Phase 3):** `reader()`, then `select`.
- **`status` (Phase 3 item):** `summary()`. In this feature it is used only by `graph-bench` and
  by tests.

## Format 2 additions (spec 008)

From `specs/008-incremental-graph-update/contracts/graph-store-format-2.md`:
- **`STORE_FORMAT = 2`:** facts live in one named graph per owner (an item IRI, or the
  vocabulary's namespace).
- **Queries:** the default graph is the union of all graphs, so queries answer as in format 1.
  Format-1 stores need a rebuild.
- **`Writer` additions:**
  - `add_owned(&[Owned])`;
  - `replace_owned(owner, facts) -> Replaced`, which is a no-op for identical facts;
  - `remove_owned(owner) -> u64`;
  - `owners()`, `owners_referencing(iri)`;
  - `publish_with(build)`.
- **`Generation.build`:** an opaque JSON stamp written by the builder.

| Rule | Statement |
|---|---|
| G10 | The fact count is kept as the writer goes; `publish` counts the store only after a default-graph `add`, `remove`, `remove_matching` or `clear` |
| G11 | `replace_owned` with identical facts writes nothing and leaves the store clean |
| G12 | Owner methods touch only that owner's graph |
| G13 | `owners` and `owners_referencing` see the writer's unpublished changes |
