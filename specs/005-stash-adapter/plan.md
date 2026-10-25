# Implementation Plan: Stash Adapter, the Library Reader

**Branch**: `master` (spec directory `005-stash-adapter`) | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/005-stash-adapter/spec.md`

## Summary

This feature completes `stash-adapter` as the engine's one way to read a Stash library. It adds a
`reader` module with a `LibraryReader` trait (contracts/reader-interface.md):
- **Full read:** every semantic tag, performer, scene and image, with progress and stopping.
- **Change reads:** ids changed since a watermark, and records of named ids.
- **Id listings:** every id of each kind, for the deletion check.

**How a full read works:**
- **Discovery:** one "key is not null" query per key (research R3; custom-field criteria cannot
  be OR-ed in Stash).
- **Records:** the merged ids are then read in batches of 250 through `ids` (R4). Pages cannot
  shift under deletes, progress has a known total, and memory is bounded by one batch.
- **Parsing:** semantic fields are parsed into structured values from one key table (R1). A bad
  field is kept as `Unreadable` and does not fail the read.

**Failures:** they become six distinct error kinds (R6). The Stash version is checked once per
client. Transient failures are retried twice.

**Spec 004:** its reads are rebuilt on the reader, with the same signatures and results (FR-021).
The fixture tool records a full `LibraryRead`, and `MemoryReader` serves it to callers' tests
with no network (FR-023).

**Vocabulary:** 1.3.0 adds terms for `fieldMapping` (R9). `archetypeGraphPosition` is passed
through as display data, never a graph term (FR-002a).

**Derived measurements (clarification 2026-10-24):** the reader reads each performer's
`measurements` and derives Bust Size, Waist, Hip and Waist-to-Hip Ratio exactly as the plugin's
`parseMeasurements` does (R2). Vocabulary 1.3.0 gets four Derived terms for them. The gap in the
plugin's documents is recorded in `plugin-notes.md`.

**Corrections to the spec, made during research:**
- **Only one native performer field:** the plugin resolves every other Label field rule through
  Stash's own filter (R2), so `measurements` is the only native field read.
- **Retired keys:** `isGeneric` and `looks` are retired and not read.
- **Variant tags:** they are discovered by `variantOfTagId`.
- **Where a read starts:** it is the newest `updated_at` per kind, because Stash has no clock
  query (R5).

## Technical Context

**Language/Version**: Rust stable (edition 2021), the existing workspace

**Primary Dependencies**:
- existing crates: `stash-adapter` (Cynic 3.14 against `schemas/stash-v0.31.1.graphql`, reqwest
  0.13 blocking with rustls and ring), `engine`, `plugin`, `semantic-vocabulary`;
- `serde`, `serde_json` and `thiserror`, already in use;
- no new external dependency. Memory in `reader-bench` is read from `/proc/self/status` on Linux.

**Storage**: none. The reader holds nothing between calls. Stash stays the system of record.

**Testing**:
- `cargo test` against recorded fixtures through the fake Stash server in
  `crates/stash-adapter/tests/common`, plus `MemoryReader`;
- opt-in live checks against the test instance;
- `reader-bench` for the timing and memory figures (quickstart).

**Target Platform**: the five release targets from 002. Measurements are taken on Linux.

**Project Type**: Rust Stash plugin (library crates plus a thin plugin binary)

**Performance Goals**:
- full read at Production size (about 50,000 scenes and 29,000 images) under 2 minutes, never
  over 5 (SC-002);
- every id of every kind under 5 s (SC-003);
- change read after a save under 100 ms (SC-004).

**Constraints**:
- Stash-side "not null" filters only, with no N+1 (Principle VII);
- typed Cynic operations selecting only the fields in R1 (constitution, Technology & Platform
  Constraints);
- only the invoking Stash is contacted, and no credential reaches a message (Principle IV);
- no writes added (FR-022);
- 004's tests pass unchanged (SC-008).

**Scale/Scope**:
- discovery: 9 tag queries, 4 or more performer queries, and 16 media queries;
- one trait, two implementations, one bench binary;
- vocabulary: 1 class, 8 properties, 1 annotation and 2 mappings;
- a parity set of at least 30 `measurements` cases (SC-010).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Checked against
constitution v3.0.0.

| Principle | Status | Notes |
|---|---|---|
| I. Stash is the system of record | Pass | Read-only, and nothing is persisted. Every record is rebuilt from Stash on each read. |
| II. Plugin documents are the contract | Pass, deliberate addition | The derived measurement fields exist only in the plugin's code. They are added deliberately with vocabulary terms, the formula is recorded, and a note goes to the plugin (R2, R9, `plugin-notes.md`). Sources are cited at `2c04141`. The key table (R1) is the documented keys exactly. The keys the vocabulary already retires or excludes stay unread. The one new key read, `fieldMapping`, is added to the vocabulary (R9). The `fieldMapping` and `archetypeGraphPosition` differences from `3c82c85` are recorded. |
| III. One engine, one set of answers | Pass | The reader has no semantic logic. `archetypeGraphPosition` is display data, and the engine computes no layout (FR-002a). |
| IV. Reached only through Stash | Pass | Only the invoking Stash is called, with credentials in sensitive headers and a test that no message carries them (R6). No hook changes, so hook cost is unchanged. |
| V. Rust, embedded, headless | Pass | All Stash I/O stays in `stash-adapter`. The builder and the plugin consume the `LibraryReader` trait and never call the network themselves. No `unsafe`. |
| VI. Versioned, explicit contracts | Pass | The operations contract is unchanged at 1.1.0: no operation or code is added. The vocabulary goes to 1.3.0 (additive). The reader interface is documented in contracts/. |
| VII. Fast, bounded, test-first | Pass | Flag-filtered discovery, batched records, no N+1. The full read reports progress and can be stopped. Budgets are stated (SC-002 to SC-005) and measured by `reader-bench`. Tests come first, against recorded fixtures, with no live Stash. |
| Technology: Cynic, pinned schema | Pass | Every new request is a typed Cynic operation against v0.31.1. There are no hand-built query strings. |

No violations. Complexity Tracking not required.

**Re-check after Phase 1 design**: Pass.
- **Contracts:** the reader interface is internal, and the operations contract does not change.
- **Vocabulary:** additive.
- **Spec 004:** its public adapter functions keep their signatures (contracts/reader-interface.md,
  "Spec 004 adapters").
- **One design risk:** the batch size of 250 versus SC-002. It is measured in quickstart step 4,
  and the constant can be tuned without any interface change.

## Project Structure

### Documentation (this feature)

```text
specs/005-stash-adapter/
├── plan.md
├── research.md                  # R1–R10
├── data-model.md                # Field, SemanticTag, SemanticPerformer, MediaRecord, LibraryRead, Error
├── quickstart.md                # offline tests, fixture capture, reader-bench, Production run
├── contracts/
│   ├── reader-interface.md      # LibraryReader, Client, MemoryReader, 004 adapters, error mapping
│   └── vocabulary-terms.md      # vocabulary 1.3.0: FieldMappingRule
├── plugin-notes.md              # documentation gaps for the plugin (derived measurements, fieldMapping)
├── checklists/requirements.md
├── results/                     # reader-bench output (measurement data)
└── tasks.md                     # /speckit-tasks
```

### Source Code (repository root)

```text
crates/stash-adapter/
├── src/
│   ├── lib.rs                   # Error variants (R6), retry and status mapping in run(), version check cache
│   ├── version.rs               # parse_stash_version, MINIMUM_STASH_VERSION (moved from engine)
│   ├── reader/
│   │   ├── mod.rs               # LibraryReader trait, Kind, Progress, constants
│   │   ├── keys.rs              # key table: TAG_KEYS, PERFORMER_KEYS, MEDIA_KEYS, kinds (R1)
│   │   ├── field.rs             # Field parsing, flag rule (FR-013, FR-014)
│   │   ├── measurements.rs      # derived Bust Size, Waist, Hip, WHR (FR-002b, R2)
│   │   ├── records.rs           # SemanticTag, SemanticPerformer, MediaRecord, LibraryRead, RecordBatch
│   │   ├── queries.rs           # typed Cynic operations: id discovery, records by ids, latest, changed
│   │   ├── client.rs            # impl LibraryReader for Client: discovery, batches, stop, progress
│   │   └── memory.rs            # MemoryReader
│   ├── relations.rs             # 004 functions rebuilt on reader (signatures unchanged)
│   ├── fixtures.rs              # capture via reader: library-read.json plus request/response pairs
│   └── bin/
│       ├── fixture-capture.rs   # records the reader fixtures
│       └── reader-bench.rs      # read-only timings and memory (quickstart step 4)
└── tests/
    ├── reader_full.rs           # SC-001, FR-001 to FR-008
    ├── reader_changes.rs        # FR-009 to FR-012
    ├── reader_fields.rs         # FR-013, FR-014, display data (FR-002a)
    ├── reader_measurements.rs   # SC-010 parity against the plugin's parseMeasurements
    ├── reader_failures.rs       # SC-006, SC-007, retries
    ├── reader_stop.rs           # SC-009, progress
    ├── reader_memory.rs         # MemoryReader keeps B1 to B9
    └── fixtures/reader/         # recorded responses
crates/engine/src/versions.rs    # re-exports the adapter's version parsing
crates/plugin/src/relations.rs   # stash_error maps the new kinds (contract reader-interface.md)
vocabulary/
├── labels.ttl                   # FieldMappingRule and 4 properties
├── stash-mapping.ttl            # map_fieldMapping
├── core.ttl                     # version 1.3.0
├── CHANGELOG.md, TERMS.md, released/1.3.0.json
crates/semantic-vocabulary/src/iri.rs   # ItemIris::field_mapping
ROADMAP.md                       # tick the Phase 2 adapter item
```

**Structure Decision**:
- **One module for reading:** all reading goes in a new `reader` module of `stash-adapter`, the
  only crate that talks to Stash (Principle V).
- **004's code:** `relations.rs` keeps its public functions and its writes, built on the reader.
  `fixtures.rs` becomes a thin recorder over it.
- **Spike code:** `spike.rs` is untouched and stays behind the `spike` feature.
- **Graph builder:** none in this feature. It is the next Phase 2 item, and it consumes
  `LibraryReader`.

## Complexity Tracking

No constitution violations to justify.
