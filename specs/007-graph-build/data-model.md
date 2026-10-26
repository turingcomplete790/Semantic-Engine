# Data model: The Semantic Graph from the Documents

The records come from spec 005 (`stash_adapter::reader`). The facts and the store come from spec
006 (`engine::graph`). This feature adds the types below, in `engine::build`.

## Inputs (existing)

| Type | From | Used for |
|---|---|---|
| `LibraryRead` | spec 005 | the whole library: `tags`, `performers`, `scenes`, `images` |
| `SemanticTag`, `SemanticPerformer`, `MediaRecord` | spec 005 | one record each; JSON keys as `Field::Value` or `Field::Unreadable` |
| `ItemIris` | spec 001 | every item and instance IRI, for one server id |
| `Fact`, `Term` | spec 006 | the output. `Ord` is derived on both (research R9) |

## Mapping

### `Mapped`

The result of mapping one library, or one record.

| Field | Type | Meaning |
|---|---|---|
| `facts` | `BTreeSet<Fact>` | the facts, with duplicates removed |
| `notes` | `Vec<Note>` | skipped and mismatched entries, in the order found |

`map_library(read, iris) -> Mapped` maps everything. The build uses `map_record` one record at a
time instead (see [Build](#build)).

**Rules:**
- **Pure:** it is a function of the records and the server id only (FR-010, SC-007).
- **What it writes:** every fact follows
  [contracts/mapping-rules.md](contracts/mapping-rules.md).

### `TagIndex`

A lookup built from `LibraryRead.tags` before anything else is mapped.

| Field | Type | Meaning |
|---|---|---|
| `flags` | `HashMap<String, BTreeSet<TagFlag>>` | each tag's flags, by tag id |

**Used for:**
- broader links;
- a performer's Trait, Label and Variation links;
- the `tagMissing` and `tagWrongType` checks.

### `Note`

| Field | Type | Meaning |
|---|---|---|
| `kind` | `Kind` (spec 005) | `scene`, `image`, `performer` or `tag` |
| `id` | `String` | the item's Stash id |
| `key` | `String` | the Stash key, for example `roleTagAssignments` or `timeline` |
| `entry` | `Option<String>` | the entry: a performer id, a list position, an entry id or a rule id |
| `reason` | `NoteReason` | see below |

**Validation:**
- A note never holds a field's value (research R8).
- Notes with equal (kind, id, key, entry, reason) are kept once.

### `NoteReason`

| Group | Values |
|---|---|
| Skipped | `Unreadable`, `BadShape`, `InvalidId`, `UnknownKind`, `Retired`, `DanglingRef` |
| Mismatch | `TagMissing`, `TagWrongType`, `PerformerNotInMedia`, `NotRecordedRole` |

`NoteReason::is_skip()` tells the two groups apart for the summary. The camelCase names
(`badShape`) are the log and contract form.

## Build

### `BuildOptions`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `batch_facts` | `usize` | 50,000 | facts per `Writer::add` |
| `note_log_cap` | `usize` | 1,000 | notes written to the log |

### `BuildReport`

What one build did. It is the envelope's `result` and the summary line.

| Field | Type | Meaning |
|---|---|---|
| `generation` | `u64` | the generation published |
| `facts` | `u64` | the generation's fact count (from `publish()`) |
| `records` | `RecordCounts` | tags, performers, scenes, images read |
| `cleared` | `Option<RebuildReason>` | why the store was cleared first (006 reasons) |
| `skipped` | `BTreeMap<NoteReason, usize>` | skip notes, by reason |
| `mismatches` | `BTreeMap<NoteReason, usize>` | mismatch notes, by reason |
| `phases` | `PhaseTimes` | milliseconds for read, map, write and publish |
| `seconds` | `f64` | the whole build |

### `BuildError`

A build that ends without publishing.

| Variant | Cause | Contract code |
|---|---|---|
| `NoServerId` | no `serverId` setting | `SERVER_ID_NOT_GENERATED` |
| `BadServerId` | the setting is not a UUID | `SERVER_ID_INVALID` |
| `Busy` | `GraphError::Busy` | `BUSY` |
| `Stash(stash_adapter::Error)` | the read failed | as `stash_error` (004) |
| `Store(GraphError)` | any other store failure | `INTERNAL` |

### Build states

```text
start ─► server id? ──no──► NoServerId
          │yes
          ▼
        health ──NeedsRebuild──► clear_for_rebuild ─┐
          │Ready / NotBuilt / Missing               │
          ▼◄────────────────────────────────────────┘
        rebuild_writer ──Busy──► Busy
          │
          ▼
        read ──error──► Stash(e)          (work dirty; nothing published)
          │
          ▼
        map + add (batches) ──error──► Store(e)
          │
          ▼
        vocabulary facts ► publish ──► BuildReport
```

A kill at any point before `publish()` switches `current` leaves the previous generation
current (research R2).

## Store (spec 006 addition)

`GraphStore::rebuild_writer()`: lock, then dirty marker, then an empty `work/`. Otherwise it is
the same as `writer()` (research R3).

## Fixtures

| File | Content |
|---|---|
| `crates/engine/tests/fixtures/build/library.json` | a hand-written `LibraryRead` (spec 005 JSON) |
| `crates/engine/tests/fixtures/build/expected.nt` | one fact per line in a short form (prefixed names, grouped by record), expanded to N-Triples and compared as a sorted set; without the vocabulary facts. The README lists the short forms |
| `crates/engine/tests/fixtures/build/expected-notes.json` | the expected `Note` list, in the camelCase contract form |
| `crates/engine/tests/fixtures/build/README.md` | what each record exercises, by mapping entry and note reason |

The test-instance fixture is read in place, from
`crates/stash-adapter/tests/fixtures/test-instance/library-read.json`.
