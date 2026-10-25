# Data Model: Stash Adapter, the Library Reader

These are the records the reader returns (`stash_adapter::reader`). Stash stays the system of
record: nothing here is stored by the engine. Ids are Stash's ids as strings. Times are Stash's
server-local text (`2026-10-24T10:27:55-06:00`).

## Field

The parsed value of one semantic `custom_fields` key (research R1).

| Variant | Holds | When |
|---|---|---|
| `Value(serde_json::Value)` | the parsed value | stored as JSON text that parses, or as a JSON value, of the expected top-level kind |
| `Unreadable { raw, reason }` | the stored text or value as found, and why it was rejected | invalid JSON, or the wrong top-level kind |

**Rules:**
- An absent key is simply not in the record's map. It is never a `Field`.
- Flags are not `Field`s. Each flag becomes a member of the record's `flags` set when its value is
  `1`, `"1"`, `true` or `"true"` (FR-014). Any other value leaves the flag out, with no error.
- **Kinds:** each key has one expected top-level kind, `List`, `Object` or `Id`, from R1's table.
  `Id` accepts a string or an integer, and keeps it as a string.

## SemanticTag

| Field | Type | Notes |
|---|---|---|
| `id` | string | |
| `name` | string | |
| `flags` | set of `TagFlag` | `Pluggable`, `Action`, `Role`, `Trait`, `Label`, `MiseEnScene`, `Variation`, `Relation` |
| `parents` | list of tag ids | Stash parents, direct only, sorted by id |
| `fields` | map from key to `Field` | only the tag keys of R1 that are present |
| `updated_at` | time | |

**Rules:**
- **Membership:** a tag is in the read if `flags` is non-empty or `fields` has
  `variantOfTagId` (FR-001).
- **Key spellings:** keys keep Stash's spelling (`labelFormula`, `fieldMapping`, and so on).

## SemanticPerformer

| Field | Type | Notes |
|---|---|---|
| `id` | string | |
| `tags` | list of tag ids | all of the performer's tags, sorted by id. The builder picks the semantic ones |
| `archetype` | bool | `isArchetype` is set |
| `archetype_parents` | `Field`, optional | `archetypeParents` |
| `measurements` | `Measurements`, optional | present when `measurements` is set (FR-002b) |
| `display` | `PerformerDisplay`, optional | display data, never a graph fact (FR-002a) |
| `updated_at` | time | |

**`Measurements`:** derived from the text (research R2).

| Field | Type | Notes |
|---|---|---|
| `raw` | string | as stored in Stash |
| `bust_size` | f64, optional | absent if the text does not parse |
| `waist`, `hip` | integer, optional | absent for bust-only text |
| `waist_to_hip_ratio` | f64, optional | rounded to 2 places; absent unless waist and hip are both present |

Text that does not parse gives `raw` with every derived value absent. That is never an error.

**`PerformerDisplay`:** `graph_position: Option<{x: f64, y: f64}>`. A missing or unreadable
position is `None`, never an error. The plugin treats one the same way: "no saved position".

**Membership:** a performer is in the read if it has a Trait, Label or Variation tag,
`isArchetype`, `archetypeParents`, or a non-empty `measurements` field (FR-002). A performer who is only referenced by a scene is
not fetched.

## MediaRecord

| Field | Type | Notes |
|---|---|---|
| `kind` | `MediaKind` | `Scene` or `Image` (004's type, moved to `reader`) |
| `id` | string | |
| `performers` | list of performer ids | sorted by id |
| `tags` | list of tag ids | native tags, sorted by id |
| `fields` | map from key to `Field` | only the media keys of R1 that are present |
| `updated_at` | time | |

**Membership:** an item is in the read if at least one media key of R1 is present (FR-003).

## LibraryRead

| Field | Type | Notes |
|---|---|---|
| `start` | `ReadStart` | the newest `updated_at` per kind, taken before discovery (FR-007) |
| `tags` | list of `SemanticTag` | sorted by numeric id |
| `performers` | list of `SemanticPerformer` | sorted by numeric id |
| `scenes`, `images` | list of `MediaRecord` | sorted by numeric id |
| `missing` | count per kind | ids found in discovery but gone by the time their records were read |
| `requests` | integer | requests sent, for SC checks and the bench |

**Rules:**
- **No duplicates:** each id appears at most once per kind (FR-008).
- **Only complete reads:** a stopped read never produces a `LibraryRead`; it returns
  `Error::Stopped` (R7).

## ReadStart

A map from kind (`Scene`, `Image`, `Performer`, `Tag`) to time. A kind with no items is absent.

## ChangeSet

The ids of one kind with `updated_at` greater than a threshold (FR-009), sorted and de-duplicated.

## RecordBatch

The answer to `records(kind, ids)` (FR-010):
- `found`: the records, in the same shape as a full read;
- `missing`: the requested ids that no longer exist.

## IdList

Every id of one kind, sorted by numeric id (FR-011). Tags here means every tag, not only semantic
ones, so a merged-away semantic tag shows as absent.

## Progress

| Field | Type |
|---|---|
| `phase` | `Discover`, `Records` or `Done` |
| `done` | integer |
| `total` | integer (known for `Records`) |

`share()` gives a number from 0 to 1, with discovery counted as the first 5% (R7).

## Error

See research R6. The variants are:
- `Unreachable(msg)` and `TimedOut(msg)`;
- `NotAuthorized`;
- `UnsupportedVersion { found, minimum }`;
- `Graphql(msg)` and `UnexpectedResponse(msg)`;
- `Setup(msg)` (existing);
- `Stopped`.

`is_transient()` is true only for `Unreachable` and `TimedOut`. No message contains a
credential.

## Existing types (spec 004), kept

- `ItemRecord`, `PerformerRecord` and `TagRecord` (in `relations`): these are built from the
  reader's records. Their names, fields and order are unchanged, so `engine::relations` and the
  plugin compile and behave as before (FR-021).
