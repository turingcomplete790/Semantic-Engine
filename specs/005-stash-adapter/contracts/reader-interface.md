# Contract: the `LibraryReader` interface

This is the Rust library interface the graph builder, the change applier, and the plugin's
operations and tasks use to read Stash (spec FR-001 to FR-023). It is an internal contract
between crates, not part of the operation protocol, so the operations contract stays **1.1.0**.

The record types are in [data-model.md](../data-model.md). The design decisions behind them are
in [research.md](../research.md), referred to below as R1 to R8.

## Module

`stash_adapter::reader`, which exports:
- **The interface:** `LibraryReader`.
- **The two implementations:** `MemoryReader`, and `Client`, which implements the trait.
- **The record types:** the types of data-model.md.
- **The key table:** `TAG_KEYS`, `PERFORMER_KEYS`, `MEDIA_KEYS`.
- **Two constants:** `RECORD_BATCH` (250) and `MINIMUM_STASH_VERSION` (`"v0.31.1"`).

## Trait

```rust
pub trait LibraryReader {
    /// FR-001 to FR-008: the whole semantic library.
    fn full_read(
        &self,
        should_stop: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<LibraryRead, Error>;

    /// FR-010: records of named ids, in the full read's shape.
    fn tag_records(&self, ids: &[String]) -> Result<RecordBatch<SemanticTag>, Error>;
    fn performer_records(&self, ids: &[String]) -> Result<RecordBatch<SemanticPerformer>, Error>;
    fn media_records(&self, kind: MediaKind, ids: &[String]) -> Result<RecordBatch<MediaRecord>, Error>;

    /// FR-009: ids of `kind` with updated_at after `threshold` (server-local text).
    fn changed_since(&self, kind: Kind, threshold: &str) -> Result<Vec<String>, Error>;

    /// FR-011: every id of `kind`, for the deletion check.
    fn all_ids(&self, kind: Kind) -> Result<Vec<String>, Error>;

    /// FR-012: the newest updated_at of `kind`, exactly as Stash stores it.
    fn latest_updated_at(&self, kind: Kind) -> Result<Option<String>, Error>;
}
```

`Kind` is one of `Scene`, `Image`, `Performer` or `Tag`. `MediaKind` (from 004) is one of `Scene`
or `Image`, and converts into `Kind`.

## Behaviour every implementation keeps

| # | Rule | Requirement |
|---|---|---|
| B1 | **`full_read` either completes or fails.** A stop gives `Err(Error::Stopped)`, and a failure gives its `Error`. It never returns a partial `LibraryRead`. | FR-006 |
| B2 | **`should_stop` is checked after each discovery query and after each record batch.** A stop takes effect before the next request. | FR-006, SC-009 |
| B3 | **`progress` is called after each discovery query and each record batch.** `share()` never decreases, and the last call has `phase: Done`. | FR-006 |
| B4 | **`start` is taken before the first discovery query.** | FR-007 |
| B5 | **Every list is sorted by numeric id, without duplicates.** That covers the lists in `LibraryRead`, `RecordBatch`, `changed_since` and `all_ids`. | FR-008 |
| B6 | **An id that no longer exists goes into `missing`.** It is never an error. | FR-010 |
| B7 | **Field parsing follows data-model "Field".** A bad field gives `Field::Unreadable`, and the record and the read continue. | FR-013, FR-014 |
| B8 | **`changed_since` compares `updated_at > threshold` as Stash does.** Callers pass `one_second_earlier(watermark)` to include saves in the same second. | FR-009 |
| B9 | **No method writes.** | FR-022 |

## `Client` (network)

- **Version check:**
  - **When:** on the first call of any reader method, then cached. An old or unparseable version
    gives `Error::UnsupportedVersion { found, minimum }` (R6).
  - **Methods kept:** `Client::stash_version()` (002) keeps working without the check, so
    `status` can still report an old server.
- **Requests:** typed Cynic operations against `schemas/stash-v0.31.1.graphql`, selecting only the
  fields of R1. Discovery uses one query per key (R3). Records are read in batches of
  `RECORD_BATCH` through `ids` (R4). There is no per-item request.
- **Retries:** reads are retried at most twice, after 250 ms and then 1 s, and only for
  `Unreachable` and `TimedOut` (R6).
- **Credentials:** no `Error` message contains the API key or the session cookie (FR-019).

## `MemoryReader` (no network)

- **Building one:**
  - `MemoryReader::new(LibraryRead)`, or `MemoryReader::from_file(path)` for a recorded
    `LibraryRead` JSON file;
  - `.with_ids(kind, ids)` adds ids that exist but carry no semantic data, for `all_ids`;
  - `.fail_with(error)` makes every call fail, for testing callers' failure handling.
- **Behaviour:** it answers every method from its records and keeps B1 to B9.
  - `changed_since` compares the records' `updated_at` texts.
  - `full_read` reports progress over its records and honours `should_stop` between batches of
    `RECORD_BATCH`.

## Spec 004 adapters (kept)

These public functions keep their signatures and results (FR-021, SC-008). FR-021 allows two
ways: served by the reader, or kept to the same rules.
- **Served by the reader:** reads whose request is identical either way.
- **Kept, to the same rules:** reads that would otherwise change 004's recorded requests. These
  are 004's flag-filtered full-row definition read, and its item read with its own field
  selection. They use the same typed scalars, the same id ordering, and the same flag rule.

| 004 function | How |
|---|---|
| `Client::changed_since(kind, threshold)` | served by the reader's change read (same request) |
| `Client::latest_updated_at(kind)` | served by the reader's newest-`updated_at` read (same request) |
| `Client::ids_with_relation_data(kind)` | served by the reader's media discovery for `actionPairings` and `relationAssignments` |
| `Client::relation_definitions()` | kept: one full-row query per `DEFINITION_FLAGS` flag, as 004 recorded; ids ordered by the reader's rule |
| `Client::items_by_ids(kind, ids)` | kept: 004's selection (no native tags), batches of 100 |
| `Client::performer_tags(ids)` | kept: all of each performer's tag ids, with no semantic membership rule, because relation rules read their Labels |
| `Client::item_native_tags`, `update_item_custom_fields`, `run_plugin_task` | unchanged |

**Shared code:**
- `MediaKind` and `StashTime` now live in `reader`; `relations` re-exports them at their old
  paths.
- `engine::relations::defs::flag_set` delegates to `reader::field::flag_set`.

**Name clash:** `Client` has inherent methods `changed_since` and `latest_updated_at` (004, taking
a `MediaKind`) with the same names as `LibraryReader`'s (taking a `Kind`). Method syntax picks
004's. Callers that want the reader's call it through the trait:
`LibraryReader::changed_since(&client, Kind::Tag, threshold)`.

## Error mapping in the plugin

`plugin::relations::stash_error` becomes:

| Adapter error | Contract code |
|---|---|
| `Unreachable`, `TimedOut` | `STASH_UNREACHABLE` |
| `UnsupportedVersion` | `STASH_TOO_OLD` |
| `NotAuthorized`, `Graphql`, `UnexpectedResponse`, `Setup`, `Stopped` | `INTERNAL`, with the kind named in the message |

No code is added, so `api/operations.json` and `docs/api.md` are unchanged.

## Additions (spec 008)

From `specs/008-incremental-graph-update/contracts/reader-additions.md`:

- **The semantic rule, in one place:** `SemanticTag::is_semantic()`,
  `SemanticPerformer::is_semantic(&performer_tags)`, `MediaRecord::is_semantic()` and
  `performer_tag_set(&tags)`.
  - `full_read` (the Client and `MemoryReader`) filters with them.
  - Records read by id stay unfiltered; callers apply the rule.
- **`semantic_tags()`:** the tag half of `full_read`, sorted by id.
- **`performers_with_tags(tag_ids)`:** ids of performers carrying any of the tags, sorted,
  queried in chunks of `PERFORMER_TAG_CHUNK`.

Rules B1–B9 hold for both methods.
