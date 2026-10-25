# Research: Stash Adapter, the Library Reader

Each decision below resolves an open point from the plan's Technical Context. Probes against the
test instance (localhost:9998, Stash v0.31.1) were read-only and ran on 2026-10-24.

## R1. Which keys the reader reads

**Decision:** the reader reads exactly the keys in the table below. Each is parsed into a
structured value and checked only for its top-level JSON kind. Deeper checks belong to the graph
builder.

| Record | Key | Top-level kind | Source |
|---|---|---|---|
| Tag | `plugtag`, `isAction`, `isRole`, `isTrait`, `isLabel`, `isMiseEnScene`, `isVariation`, `isRelation` | flag | `stash-mapping.ttl` |
| Tag | `variantOfTagId` | tag id (string or number) | `stash-mapping.ttl` |
| Tag | `categories`, `variations` | list | `stash-mapping.ttl` |
| Tag | `labelFormula`, `variationData`, `relationEnds` | object | `stash-mapping.ttl` |
| Tag | `relationRules` | list | `stash-mapping.ttl` (004) |
| Tag | `fieldMapping` | object `{base?, variations?}` | plugin `src/ui/field-mapping.js` at `2c04141` |
| Tag | Stash `parents` | ids | `stash-mapping.ttl` |
| Performer | `isArchetype` | flag | `stash-mapping.ttl` |
| Performer | `archetypeParents` | list of ids | `stash-mapping.ttl` |
| Performer | `archetypeGraphPosition` | object `{x, y}`, display data | plugin `src/archetypes/index.js` at `2c04141` |
| Performer | Stash `tags` | ids | `stash-mapping.ttl` |
| Performer | Stash `measurements` | text, plus four derived numbers (R2) | plugin `src/ui/matrix.js` at `2c04141`; deliberate addition |
| Scene, image | `roleTagAssignments`, `actionAssignments`, `pluggableAssignments`, `autoRoleAssignments` | object | `stash-mapping.ttl` |
| Scene, image | `actionPairings`, `roleTagPairings`, `relationAssignments`, `timeline` | list | `stash-mapping.ttl` |
| Scene, image | Stash `performers`, `tags`, `updated_at` | ids, time | Stash schema |

**Not read:**
- **Retired keys:** `isGeneric` and `looks` (vocabulary 1.0.0, spec 001 FR-016).
- **Excluded keys:** `archetypeRecipe` and the plugin's caches `directTraitVariations`,
  `inheritedVariations` and `inheritedTraitVariations`. The vocabulary changelog excludes these
  on purpose.

**Values:** a value may be stored as JSON text (the plugin's habit) or as a JSON value. Both are
accepted. A text value that does not parse, or parses to the wrong kind, becomes
`Unreadable { raw, reason }` (FR-013).

**Rationale:** one table drives discovery, parsing, fixtures and the vocabulary check, so the
contract cannot drift between them.

**Alternatives considered:**
- Returning the raw `custom_fields` map, as 004 does: rejected. Every caller would then parse and
  validate on its own.
- Full schema validation in the adapter: rejected. It duplicates the builder's job.

## R2. Native performer fields: only `measurements`

**Decision:** the only native performer field read is `measurements`. From it the reader derives
Bust Size, Waist, Hip and Waist-to-Hip Ratio (clarification 2026-10-24, spec FR-002b).

**Other fields:** at `2c04141`, `labels.js` resolves each Label field rule
(`resolveFieldRuleMatchSets`) with a Stash performer filter query built from the rule. "Does this
performer match" is "whatever Stash's own filter resolution says", so those fields are never read.
Field Mapping (`field-mapping.js`) only *writes* performer fields. Resolving those rules is Label
evaluation (ROADMAP Phase 5), which adds a bounded "performer ids matching this criterion" read.

**The exception:** `matrix.js` defines four derived fields (`DERIVED_PERFORMER_FIELDS`:
`__derived_bust`, `__derived_waist`, `__derived_hip`, `__derived_whr`). Stash cannot filter on
them, so the plugin fetches every performer's `measurements` and computes them itself
(`resolveDerivedFieldMatches`). Label field rules can name them, so the engine must resolve them.

**Derivation:** it is copied from `parseMeasurements` at `2c04141`:
- **Pattern:** `^(\d+)\s*([A-Za-z]*)\s*(½|\.5|1/2)?\s*(?:-\s*(\d+)\s*-\s*(\d+))?` on the
  trimmed text. If there is no match, or the band is 0, nothing is derived.
- **Cup:** the letters are upper-cased. DD, DDD and DDDD become E, F and G. A half marker adds
  `½`.
- **Cup steps:** AA 0, A 1, A½ 2, B 3, B½ 4, C 5, C½ 6, D 7, D½ 8, E 9, E½ 10, F 11, G 12, H 13,
  I 14, J 15, K 16, L 17. Any other cup has no step.
- **Bust Size:** band + step × 2.5 (when the cup has a step) + (28 − waist) × 2 (when the waist is
  given and under 28). It is a decimal number.
- **Waist** and **Hip:** integers, when present.
- **Waist-to-Hip Ratio:** waist ÷ hip, rounded to 2 places with JavaScript's `Math.round`
  semantics (half rounds up), when both are present and neither is 0. The plugin's test is
  `waist && hip`. A waist of 0 still counts as given for the Bust Size bonus, as in the plugin
  (`waist !== null`).

**Where the code lives:** it is a pure function in `reader/measurements.rs`, like the JSON field
parsing. It turns a Stash text field into structured values and does no I/O.

**Parity:** a fixture of at least 30 `measurements` strings with the plugin's results (SC-010).
The strings come from the 791 test-instance performers that have measurements, plus the edge
cases. The expected values are produced by running the plugin's own `parseMeasurements` under
Node once, and they are committed with the plugin commit recorded.

**Alternatives considered:**
- **Reading every native performer field:** rejected. It is unbounded, and it would hand-roll
  comparisons that Stash already does.
- **Reading the raw text only and deriving later:** rejected by the owner. Labels use the derived
  fields, so the engine resolves them now.

## R3. Discovery: one query per key

**Decision:** discovery runs one "key is not null" query per key and kind, and merges the results
by id. In numbers:
- **Tags:** 9 queries (8 flags plus `variantOfTagId`).
- **Performers:** 3 queries (`isArchetype`, `archetypeParents`, and `measurements` not null, a
  native string criterion), plus one query for performers whose tags include any Trait, Label or
  Variation tag. That query is chunked at 500 tag ids. On the test instance 791 of 1,107
  performers have `measurements`.
- **Scenes and images:** 8 queries per kind.

The count is fixed by the key table, never by library size.

**Probe:** custom-field criteria cannot be OR-ed. `id = 3 OR roleTagAssignments not null`
returned only scene 2, though scene 3 exists. Stash applies `custom_fields` to the whole filter,
outside the `OR` tree. Plain criteria do OR correctly (`id = 4 OR id = 3` returned both). This
confirms 004's note that custom-field criteria combine only with AND.

**Alternatives considered:**
- A nested `OR` chain: rejected, because it gives wrong results (probe above).
- Reading everything and filtering locally: rejected by Principle VII.

## R4. Paging: ids first, then records by id

**Decision:** a full read has two phases.
1. **Id discovery:** the queries of R3, selecting `id` only, with `per_page: -1`. Ids cost about
   13 bytes each.
2. **Records:** the merged, sorted ids fetched through the `ids` argument, in batches of
   `RECORD_BATCH` = 250, with only the fields of R1.

**Rationale:**
- **Stable under edits (FR-008):** the record set is fixed after phase 1. Deletions and inserts
  during phase 2 cannot shift pages, so nothing is skipped or repeated. An id deleted meanwhile is
  returned as missing.
- **Progress has a known total (FR-006):** after phase 1 the reader knows how many records
  follow, so progress is a true share.
- **Bounded memory (SC-005):** the reader holds the id lists (about 1 MB at 79,000 items) and one
  batch of raw response. The records it returns are the caller's.
- **One shape:** batched reads by id are the same operation as FR-010's records for given ids.

**Probes** (test instance, 1,021 scenes):
- **All scene ids:** 17 ms (0.016 ms per id). At Production size, all ids of every kind take
  about 1.5 s, under SC-003's 5 s.
- **A page of 500 scenes with custom fields, performers and tags:** 72 ms (0.14 ms per item).
  About 79,000 media items then take about 11 s, against SC-002's 2 minutes. 003 measured
  0.38 ms per item warm for the full builder path.
- **Batch size:** 250 halves the raw response held at once versus 500, for about 10% more
  requests. The size is measured again in the quickstart, and tuned if SC-002 needs it.

**Alternatives considered:**
- **Offset paging (`page`/`per_page`):** rejected, because pages shift under deletes.
- **Keyset paging (`id > last`):** works for scenes and images (they have an `id` criterion), but
  not for performers or tags (their filters have none). It also cannot be combined with
  one-query-per-key discovery without repeating each query per page.
- **`per_page: -1` for records:** rejected. It holds the whole library's raw response in memory
  at once.

## R5. Where a read starts

**Decision:** before phase 1, the reader takes the newest `updated_at` of each kind (scenes,
images, performers, tags). It uses one query per kind, sorted by `updated_at` descending with
`per_page: 1`, exactly as 004's `latest_updated_at` does for scenes and images. The result is
`ReadStart { kind → server-local time text }`.

**Change reads:** they take a threshold in that text form. Stash compares it as text, to the
second, so callers pass 004's `one_second_earlier` (003 decision record). The threshold is never
sent together with `ids`, because Stash then ignores the filter (003 findings).

**Rationale:**
- **No clock query:** Stash's schema has no query for the server's current time.
- **Equivalent for catch-up:** an item saved after the read began has a newer `updated_at`, so the
  next change read finds it.

**Alternatives considered:**
- **The engine's own clock:** rejected. It can differ from the server's time zone and clock, and
  a UTC threshold matched 0 of 1,000 changed scenes (003).

## R6. Failure kinds and how each is detected

**Decision:** `stash_adapter::Error` gains distinct variants. The existing `Unreachable`,
`Graphql` and `Setup` keep their meaning, so callers of 002 and 004 compile unchanged except for
new match arms.

| Kind | Variant | Detected by | Retried |
|---|---|---|---|
| unreachable | `Unreachable(msg)` | connection refused, DNS or reset; HTTP 502, 503 or 504 | yes |
| timed out | `TimedOut(msg)` | reqwest timeout (5 s to connect, 60 s per request) | yes |
| not authorized | `NotAuthorized` | HTTP 401 or 403. The probe gave 401 with an empty body for a wrong key and for no key | no |
| unsupported version | `UnsupportedVersion { found, minimum }` | `version` query below v0.31.1, or unparseable | no |
| error reported by Stash | `Graphql(msg)` | a GraphQL `errors` array | no |
| unexpected response | `UnexpectedResponse(msg)` | a body that does not decode to the typed operation, missing `data`, or another non-2xx status | no |

**Version check:** it runs once per `Client`, before the first library read. Its result is
cached for the life of the process. The parsing in `engine::versions` (`parse_stash_version`,
`STASH_MINIMUM_VERSION`) moves into the adapter, and `engine` re-exports it, so there is still
one copy.

**Retries:** at most 2, waiting 250 ms and then 1 s. They apply only to reads; writes (004) are
never retried, since a write may have landed.

**Credentials:**
- The API key and the cookie travel only in headers marked sensitive.
- Messages are built from the error kind, the HTTP status and reqwest's message, which carries
  the URL but no header.
- A test sends a known secret through every failure path and asserts that no message contains it
  (SC-007).

**Contract codes:** this feature adds no operation, so the contract stays 1.1.0. The plugin's
`stash_error` maps the new kinds as follows:
- `TimedOut` becomes `STASH_UNREACHABLE`.
- `UnsupportedVersion` becomes `STASH_TOO_OLD`.
- Every other kind becomes `INTERNAL`, with the kind named in the message.

A dedicated `STASH_UNAUTHORIZED` code is left to the feature that first exposes builds to
clients.

**Alternatives considered:**
- One `Error::Stash(String)`: rejected by FR-016.
- Retrying everything: rejected, because it retries rejected credentials and real errors.

## R7. Cancellation and progress

**Decision:** the full read takes `&dyn Fn() -> bool` (`should_stop`) and
`&mut dyn FnMut(Progress)`. Both are checked or reported after each phase-1 query and each
phase-2 batch.
- **Progress:** `{ phase, done, total }`. Its `share()` is the share of phase 2's records done,
  with phase 1 counted as the first 5%.
- **Stopping:** a stop returns `Err(Error::Stopped)`, never a partial `LibraryRead` (FR-006).

The plugin's task later wires Stash's job stop to `should_stop`. Nothing is wired in this
feature.

**Alternatives considered:**
- An iterator or stream of records: rejected for now. The builder of the next feature needs
  everything before it writes a generation. A streaming form can be added later without changing
  the record types.

## R8. One reader interface, one stand-in

**Decision:** a trait `LibraryReader` in `stash_adapter::reader` with the reads of FR-001 to
FR-012:
- `full_read`
- `records(kind, ids)`
- `changed_since(kind, threshold)`
- `all_ids(kind)`
- `latest_updated_at(kind)`

It has two implementations:
- **`Client`:** the network implementation.
- **`MemoryReader`:** built from records, or from a recorded `LibraryRead` JSON file, with no
  network. It lets the builder and the plugin test against it (FR-023). It is always compiled,
  since it has no I/O beyond reading a file.

**Changes to 004 (FR-021):**
- **Store reads:** `apply::Store`'s read methods (`items`, `performer_tags`, `changed_since`,
  `latest_updated_at`, `ids_with_relation_data`) call the reader's functions.
- **`relation_definitions`:** it becomes the semantic-tag read, filtered to 004's
  `DEFINITION_FLAGS`. Its `TagRecord` is built from the new record, with the same fields and the
  same order.
- **Unchanged:** 004's writes and its public functions keep their signatures, and its tests must
  pass unchanged (SC-008).

**Fixture capture:** `fixtures.rs` moves to the reader as well: `fixture-capture` writes a
recorded `LibraryRead`, plus the raw request and response pairs the adapter tests replay.

**Alternatives considered:**
- Leaving 004's reads separate: rejected, because it keeps two ways to read the same records
  (FR-021).
- A mocking crate: rejected. The fake HTTP server in `tests/common` and `MemoryReader` cover
  both levels without a new dependency.

## R9. Vocabulary changes

**Decision:** vocabulary 1.3.0 (MINOR, additive).

- **`fieldMapping`:** mapped to a new class `sem:FieldMappingRule`, with:
  - `sem:hasFieldMapping` (Tag → FieldMappingRule);
  - `sem:targetField` (string);
  - `sem:hasPriority` (integer; lowest wins);
  - `sem:forVariation` (string, optional).

  It is like Label rules: the rule is described in the graph, and what it produces stays the
  plugin's write.
- **Derived measurements:** four Performer properties, all with `sem:status sem:Derived`:
  - `sem:bustSize` (decimal);
  - `sem:waist` (integer);
  - `sem:hip` (integer);
  - `sem:waistToHipRatio` (decimal).

  A mapping `sem:map_measurements` comes from `Performer.measurements`. The plugin's field-rule
  names `__derived_bust`, `__derived_waist`, `__derived_hip` and `__derived_whr` are recorded on
  each property, so a `sem:FieldRule` naming one of them resolves to the property.
- **`archetypeGraphPosition`:** moves in the changelog from "Excluded on purpose" to a new
  "Display data, passed through" list. It is not a graph term (FR-002a).

**Alternatives considered:**
- Leaving `fieldMapping` out: rejected by FR-015, since the spec reads it.

## R10. Measurement on Production size

**Decision:** the quickstart measures in two steps.
- **On the test instance:** the 1,021 scenes and 1,327 images are seeded with semantic data
  using the spike's `seed` and `cleanup` commands, which are allowed on the test instance. A full
  read is timed there, giving the cost per item.
- **On Production:** the owner runs the read-only `reader-bench` against Production with their
  own key. The reader sends no writes, so this is safe, and it is the real SC-002 figure.

**Rationale:** the test instance holds about 3% of Production's items, and the engine has no
Production credentials. Projecting from per-item cost was the 003 approach. The owner's run
replaces the projection.
