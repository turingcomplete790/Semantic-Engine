# Contract: graph store additions (spec 006, format 2)

These are additions to `engine::graph`, recorded in
`specs/006-graph-store/contracts/graph-store-interface.md`. Every 006 rule (G1–G9) still holds.
In particular, no Oxigraph type appears in the interface.

## Format

- **`STORE_FORMAT = 2`:** facts are kept in named graphs, one per owner, which is an item IRI
  or the vocabulary's namespace (research R1).
- **Format-1 stores** give `NeedsRebuild(OtherFormat { found: 1, expected: 2 })`.
- **Queries:** `Reader::select` evaluates every query with the default graph as the union of
  all graphs, so queries written for format 1 give the same answers.

## Writer

```rust
/// Facts owned by one item.
pub struct Owned {
    pub owner: String,     // an absolute IRI
    pub facts: Vec<Fact>,
}

/// What `replace_owned` did.
pub enum Replaced {
    Unchanged,
    Changed { removed: u64, added: u64 },
}

impl Writer {
    /// Adds whole owners' facts, for builds: the owners' graphs must be empty. Uses the bulk
    /// loader under the same rule as `add` (006 R3).
    pub fn add_owned(&mut self, batch: &[Owned]) -> Result<(), GraphError>;

    /// Makes the owner's graph hold exactly `facts`. Writes nothing when it already does.
    pub fn replace_owned(&mut self, owner: &str, facts: &[Fact]) -> Result<Replaced, GraphError>;

    /// Removes the owner's graph; returns how many facts it held (0 if none).
    pub fn remove_owned(&mut self, owner: &str) -> Result<u64, GraphError>;

    /// Every owner in the working store, sorted.
    pub fn owners(&self) -> Result<Vec<String>, GraphError>;

    /// Owners whose graphs hold a fact with `object` as its object, sorted.
    pub fn owners_referencing(&self, object: &str) -> Result<Vec<String>, GraphError>;

    /// `publish`, also recording `build` in the generation file (research R7).
    pub fn publish_with(self, build: serde_json::Value) -> Result<Generation, GraphError>;
}
```

`Generation` gains `build: Option<serde_json::Value>`, which is absent for generations
published with plain `publish()`.

## Rules

| Rule | Statement |
|---|---|
| G10 | **The fact count is kept, not recounted.** `publish` records the writer's kept count when it is known; only after a default-graph `add`, `remove`, `remove_matching` or `clear` does it count the store (research R2). In debug builds, tests assert the kept count equals the store's. |
| G11 | **`replace_owned` is a no-op for identical facts.** It does not mark the store dirty, and does not change the count. |
| G12 | **An owner's facts are only its graph.** `remove_owned` and `replace_owned` never touch another owner's graph or the default graph. |
| G13 | **`owners()` and `owners_referencing()` read the working store.** They see the writer's own unpublished changes. |
