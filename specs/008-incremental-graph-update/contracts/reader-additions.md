# Contract: library reader additions (spec 005)

These are additions to `stash_adapter::reader`, recorded in
`specs/005-stash-adapter/contracts/reader-interface.md`. Rules B1–B9 still hold, and every
method only reads.

## The semantic rule, in one place (research R3)

```rust
impl SemanticTag {
    /// A flag is set, or `variantOfTagId` is present.
    pub fn is_semantic(&self) -> bool;
}
impl SemanticPerformer {
    /// An Archetype, archetype parents or measurements, or a tag in `performer_tags`.
    pub fn is_semantic(&self, performer_tags: &BTreeSet<String>) -> bool;
}
impl MediaRecord {
    /// At least one semantic field is present.
    pub fn is_semantic(&self) -> bool;
}
/// The ids of tags with a Trait, Label or Variation flag.
pub fn performer_tag_set(tags: &[SemanticTag]) -> BTreeSet<String>;
```

`Client::full_read` and `MemoryReader::full_read` filter with these functions, and their results
are unchanged. Records read by id stay unfiltered; callers apply the rule.

## New trait methods

```rust
pub trait LibraryReader {
    // … the 005 methods …

    /// Every semantic tag: the tag half of `full_read` (discovery by key, records in batches,
    /// filtered by `is_semantic`), sorted by id (B5).
    fn semantic_tags(&self) -> Result<Vec<SemanticTag>, Error>;

    /// Ids of performers carrying any of `tag_ids`, sorted. Queried in chunks of
    /// `PERFORMER_TAG_CHUNK` (500), as discovery does.
    fn performers_with_tags(&self, tag_ids: &[String]) -> Result<Vec<String>, Error>;
}
```

- **`MemoryReader`** answers both from its records.
- **`build-bench`'s `OnceReader`** returns `Setup` errors for them, as for every method other
  than `full_read`.

## Name clash

Spec 005 found that `Client` has inherent methods named like trait methods. The new names do not
clash with any inherent `Client` method today. If one appears, callers use
`LibraryReader::method(&client, …)`, as 005 documents.
