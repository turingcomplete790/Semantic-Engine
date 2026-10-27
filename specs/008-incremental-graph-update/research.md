# Research: Incremental Graph Update

Each decision resolves an open point of the plan's Technical Context. These sources are taken as
settled:
- the 003 decision record, rules 7, 8 and 13, and § 3 Freshness;
- spec 005's reader (`changed_since`, `all_ids`, records by id, `MemoryReader`);
- spec 006's store (single writer, generations, the dirty marker);
- spec 007's mapping (`engine::build`), its fixtures, and its results.

## R1. Knowing which facts belong to which item: one named graph per item

**Decision:** every record's facts are kept in a named graph whose name is the record's item IRI:
- each tag, performer, scene and image owns its graph;
- the vocabulary's facts live in the graph `urn:semantic-stash:vocab:`;
- queries see the union of every graph as their default graph, so SPARQL `SELECT` answers do not
  change.

Removing an item is then clearing its graph, and replacing it is clearing and refilling it.

**Why the graphs never overlap:** spec 007's mapping gives each record facts whose subjects
belong to that record only:
- a media item's own IRI and the IRIs under it (instances, pairings, timeline);
- performer links to those instances (`hasRole` and the rest), which name the media item's
  instances;
- a tag's own IRI and the IRIs under it (rules, Field Mapping slots);
- a performer's own IRI.

So no fact appears in two graphs, and the union holds no duplicates. A test checks this over
the fixture and the synthetic library, and the equivalence tests (R8) would also catch a
breach.

**Rationale:**
- **Cheap removal:** Oxigraph keeps a graph-first index, so clearing a graph costs the size of
  that graph. Deleting a scene does not scan the store.
- **No guessing:** the alternative, finding an item's facts by IRI prefix, needs a scan, or a
  query per item, and misses performer links whose subject is not under the item's IRI.

**Alternatives considered:**
- **Re-deriving the old facts from the old record:** rejected. The old record is gone once Stash
  has changed it, and keeping every record would double the store.
- **Prefix queries (`STRSTARTS`):** rejected for the scan cost at 250,000 scenes, and for the
  performer links above.
- **A side table of item to facts:** rejected. It duplicates the store's own index, and must be
  kept in step with it.

**Consequence: store format 2.** Spec 006's `STORE_FORMAT` becomes 2. A format-1 store (the
test instance and Production today) gives `NeedsRebuild(OtherFormat)`, so the first update after
installing rebuilds in full, as FR-010 already requires.

## R2. Publishing without counting every fact

**Finding:** measured on a 250,000-scene store on disk (9.79 million facts), publishing is
almost all counting:

| Step | Time |
|---|---|
| `Store::len` (count every fact) | 2.39–3.08 s |
| `flush` | 27–31 ms |
| `backup` (checkpoint) | 4–9 ms |

This confirms 006's guess about its 3.8 s publish.

**Decision:** the writer keeps the fact count as it goes:
- `rebuild_writer()` starts at 0;
- `writer()` starts at the current generation's recorded count;
- adding a graph adds its size, and clearing one subtracts the size it had;
- `publish()` uses the kept count, and calls `Store::len` only when the count is unknown. That
  happens after the older default-graph methods (`add`, `remove`, `remove_matching`, `clear`),
  which tests still use.

A debug assertion in the tests checks the kept count against `Store::len`.

**Rationale:** publishing drops from about 3 s to about 40 ms at 250k, which makes SC-002's
5-second budget for one changed scene reachable.

## R3. The same "semantic" rule for full reads and updates

**Finding:** the full read keeps a record only when it is semantic (spec 005, `read_all`):
- **A tag:** a flag is set, or `variantOfTagId` is present.
- **A performer:** it is an Archetype, has archetype parents or measurements, or carries a tag
  with a Trait, Label or Variation flag.
- **A scene or image:** at least one semantic field is present.

Records read by id are not filtered, so an update could keep an item that a full rebuild drops.

**Decision:** the rule moves into the reader as public functions, used by both:
- `SemanticTag::is_semantic()`;
- `SemanticPerformer::is_semantic(performer_tags)`, where `performer_tags` is the set of tags
  with a Trait, Label or Variation flag;
- `MediaRecord::is_semantic()`;
- `performer_tag_set(&[SemanticTag])`.

`read_all` keeps its behaviour, now through these functions. The update maps a re-read record
only when it is semantic, and otherwise removes the item's graph.

**Rationale:** one definition of "semantic", so an update and a full rebuild cannot disagree on
which items exist. This is required by FR-005 and FR-007.

## R4. Tags are read whole on every update

**Decision:** each update reads every semantic tag again, through a new reader method,
`semantic_tags()`. It is the tag half of the full read: one discovery query per tag key, then
the records in batches.

**Rationale:**
- **Tags drive other items' facts:**
  - the tag index (flags) decides performers' Trait, Label and Variation links (spec 007);
  - spec 004's relation definitions validate relation rules against other tags' types;
  - a performer's "semantic" test needs the set of semantic performer tags (R3).

  An update must have the current flags of every tag, not only the changed ones.
- **Tags are few:** Production has 49 semantic tags, and the synthetic library 48. The read is
  9 discovery queries plus one batch per 250 tags, so a few tens of milliseconds.
- **No cache to go stale:** keeping a copy of the tag table with each generation would work too,
  but could silently disagree with Stash.

**Consequence for FR-019:** tags are the one kind an update reads in full. Their count is the
number of semantic tags, not the library's size, and the budgets measure it.

## R5. Which items an update re-maps

**Decision:** an update works out, in order:
1. **Catch-up point:** the new point, `latest_updated_at` per kind, taken before any other read
   (FR-009).
2. **Tags:**
   - read every semantic tag (R4);
   - remove the graphs of tags in the store that are not among them (deleted, merged away, or
     no longer semantic);
   - replace every tag's graph with its new facts (R6 skips the unchanged ones).
3. **Tags that matter to performers:** the tags whose graphs were removed or actually changed.
4. **Performers:**
   - those changed since the old point (`changed_since`, from one second earlier);
   - the dependents of step 3: performers Stash says carry those tags (new reader method
     `performers_with_tags`), and performers whose graphs hold a link to them (new store method
     `owners_referencing`);
   - read them by id, then replace or remove each graph by the semantic rule (R3);
   - remove the graphs of performers no longer in Stash (`all_ids` compared with the store's
     owners).
5. **Scenes and images:** those changed since the old point, read by id, then replaced or
   removed; deletions as for performers.
6. **Publish** once, with the new point in the generation's build stamp (R7), but only if a
   fact changed (FR-008).

**Why media have no dependents:** spec 007's media facts name tags and performers only by IRI.
No fact of a media item depends on a tag's flags or a performer's data. Mismatch notes depend on
flags, but notes are not facts.

**Fallback (FR-010):** before any writing, if the performers, scenes and images to re-map exceed
a third of the items in the store, the update rebuilds in full instead. The plan sets the third
as the default; the bench measures where an update stops being cheaper (R10), and the constant
is set from that.

## R6. Replacing a graph only when its facts changed

**Decision:** the store method `replace_owned(owner, facts)` compares the new facts with the
graph's current facts, and writes only if they differ. It returns what happened: unchanged, or
the facts removed and added.

**Rationale:**
- **No empty generations:** Stash bumps `updated_at` on any save, including saves of fields the
  graph does not hold. Without the comparison, such a save would publish a new, identical
  generation (FR-008, SC-003).
- **Cheap:** the comparison reads one item's graph, the same cost as the clear it replaces.

**The catch-up point when nothing changed:** no generation is published, so the point stays
where it was. The next update re-reads the same unchanged items, and again publishes nothing.
That cost is bounded by the items saved since the last publish, and the next real change moves
the point.

## R7. The build stamp

**Decision:** each generation file (`snapshots/<n>.json`) gains a `build` object, written by
`publish_with(meta)`:

```json
"build": {
  "mappingVersion": 1,
  "vocabularyVersion": "1.4.0",
  "catchUp": { "tag": "…", "performer": "…", "scene": "…", "image": "…" },
  "how": "build" | "update"
}
```

- **`mappingVersion`:** a new constant, `engine::build::MAPPING_VERSION = 1`. It is bumped by any
  change to the mapping rules that changes what some record gives. A test fixes it to the
  fixture's expected graph: changing `expected.nt` without bumping it fails.
- **`catchUp`:** Stash's text, never converted (003: a UTC threshold matched nothing). A kind
  with no items has no entry, and then every item of that kind counts as changed.
- **The store module treats `build` as opaque JSON,** so `engine::graph` does not learn about
  mapping.

The update rebuilds in full when the stamp is absent, unreadable, or its versions differ from
the running engine's (FR-010, FR-011).

## R8. Proving equivalence with a full rebuild (SC-001)

**Decision:** a test, `update_equivalence`, runs scripted change sequences on a `MemoryReader`
library:
1. build;
2. apply one change, with `updated_at` moved forward;
3. update;
4. compare every fact in the published generation with a full rebuild of the changed library in
   a second store;
5. repeat over the sequence.

**The changes cover:**
- **Tags:** add, edit fields, set and clear a flag, delete, merge (delete the source and edit
  the destination), and make one non-semantic.
- **Performers:** add, edit tags, gain and lose semantic status, delete.
- **Scenes and images:** add, edit each key, clear every semantic field, delete.
- **Cross-item:** a tag gains Trait while a performer carries it; a parent Role loses its flag; a
  relation's Label condition loses its flag.

There are at least 20 sequences, some of them several changes long.

**The comparison reads every fact through the reader** (`SELECT ?s ?p ?o`, now over the union),
so it checks what clients will see.

## R9. Kill safety

**Decision:** unchanged from 007 R2. An update writes through `writer()`, which marks the store
dirty before its first change, and publishes once.
- **A kill:** nothing is published, and the next writer resets the working store from the
  current generation.
- **The catch-up point:** it lives only in the published generation, so the next update starts
  from the last published point (FR-013).

**Test:** `update_kill` follows 007's `build_kill`. The child runs an update of 300 changed
scenes on a synthetic library, the parent kills it at varied points, and after each kill:
- the store is whole;
- a fresh update gives the full rebuild's graph.

## R10. Measuring the budgets

**Decision:** `build-bench update --scenes N --images M --change K`:
1. builds a synthetic library;
2. changes K scenes (alternating edits, deletions and new scenes) with newer `updated_at`;
3. times one update through a `MemoryReader`, with every phase reported (reads, mapping,
   writes, publish).

Runs at 250,000 scenes with K = 0, 1, 100 and 1,000 give SC-002 and SC-003.

**The fallback point:** a sweep of K up to a full rebuild's time sets the fallback constant
(R5), if the default third is far off.

**The query budget:** spec 006's SC-003 (p95 under 200 ms at 250k) is measured again with
`graph-bench query`, because format 2 queries the union of graphs.

**Request counts (US4, SC-006):** a `CountingReader` wrapper in the tests counts each reader
call and the ids it asks for. On the test instance, the Client's own request count is logged.

## R11. The task and the contract

**Decision:**
- **The task:** the manifest gains "Update semantic graph" (operation `updateGraph`).
- **The contract:** it moves to 1.3.0, a MINOR change, recorded in
  [contracts/operations-1.3.md](contracts/operations-1.3.md).
- **"Build semantic graph" keeps its result.** Its generation now also carries the build stamp.

**Errors:** the same as `buildGraph`. There are no new codes.

## R12. The store keeps values, not the text it was given (found in implementation)

**Finding:** Oxigraph 0.4.11 stores typed literals as values (`storage/numeric_encoder.rs`):
- every integer-derived type (`nonNegativeInteger`, `int`, `long`, `positiveInteger` and the
  rest) is stored, and read back, as `xsd:integer`;
- decimals, booleans and dates are kept in canonical form.

Spec 007's `sem:position` is written as `xsd:nonNegativeInteger`, so it reads back as
`xsd:integer`. That is true of every build since spec 007. Queries see `xsd:integer`, with the
same numeric value.

**Consequence for updates:** comparing an item's new facts with its stored ones must compare
them in stored form. Otherwise every item with a timeline would look changed on every update,
and an update with no change would still publish.

**Decision:** `replace_owned` first writes the new facts to a small in-memory scratch store and
reads them back, so both sides are in the store's own form. This uses Oxigraph's own
conversion, so it stays right if a later version changes its rules.

**Follow-up for the mapping (not this feature):** spec 007's mapping rules could write
`position` as `xsd:integer`, the type the store keeps. That changes the mapping version. The
vocabulary's declared range stays `xsd:nonNegativeInteger`, which the value satisfies.

## R13. A crowded catch-up second (found on the test instance)

**Finding:** the test instance's items were seeded in bulk, so hundreds share the newest second of
their kind: 305 scenes, 262 images and about 31 performers. Rule 7 reads from one second before
the catch-up point, so every update re-reads all of them. Counting them as changes made the
second update on the test instance exceed the share limit (598 against 500) and rebuild in full,
although only three items had changed.

**Decision:** each kind gets a second change query, `changed_since(point)` (strictly after the
point's second), whose count is what the share limit uses. The re-read set stays rule 7's (from
one second early), so saves in the catch-up second are still caught.

**Cost:** one more query per kind (6 instead of 3), and the re-reads of the crowded second. Those
are batched (250 per request), and confirm unchanged facts without writing (R6).

A test, `a_crowded_catch_up_second_does_not_force_a_rebuild`, covers 700 scenes in one second.
