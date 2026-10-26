# Graph build fixtures (spec 007)

| File | Content |
|---|---|
| `library.json` | a hand-written `LibraryRead` (spec 005's JSON form) |
| `expected.nt` | its full expected graph, written by hand from `specs/007-graph-build/contracts/mapping-rules.md`, without the vocabulary's own facts |
| `expected-notes.json` | the notes its bad fields give (user story 3) |

The captured test-instance library is read in place, from
`crates/stash-adapter/tests/fixtures/test-instance/library-read.json`.

## Short forms

`expected.nt` has one fact per line, ending in ` .`. It is grouped by record for review, and
the tests compare it as a sorted set. The helper (`tests/common/build.rs`) expands the short
forms:

| Short form | Expands to |
|---|---|
| `a` | `rdf:type` |
| `rdfs:x` | `<http://www.w3.org/2000/01/rdf-schema#x>` |
| `sem:x` | `<urn:semantic-stash:vocab:x>` |
| `i:x` | `<urn:semantic-stash:00000000-0000-4000-8000-000000000001:x>` |
| `"v"^^xsd:t` | `"v"^^<http://www.w3.org/2001/XMLSchema#t>` |
| `"v"` | a plain string |

## Blessing

With `BUILD_FIXTURE_BLESS=1`, `build_fixture` writes the mapped graph as `expected.nt.new`, in
full N-Triples, instead of comparing. Review the difference by hand, then edit `expected.nt`. The
first version of `expected.nt` was written by hand, not blessed.

## What each record exercises

| Mapping entry or rule | Record |
|---|---|
| each tag flag (`map_plugtag` … `map_isRelation`) | tags 1–4, 6–14 |
| two flags on one tag | tag 13 (Trait and Label) |
| `map_tagParents`, broader role, broader relation, parent not read | tags 2, 12 |
| `map_categories`, `map_variations` | tags 2, 3, 4, 7, 9 |
| `map_variantOfTagId`, a tag with no flag | tag 5 |
| `map_labelFormula_*`: all four rule kinds, `__any__`, `__derived_bust`, threshold 0 | tag 8 |
| a formula with only a threshold | tag 13 |
| `map_variationData` | tag 11 |
| `map_fieldMapping`: base and two variation slots | tag 7 |
| `map_relationEnds`, `map_relationRules`: label and role conditions, actions | tag 12 |
| `map_performerTags`: Trait, Label, Variation, and a non-semantic tag | performers 7, 8 |
| `map_isArchetype`, `map_archetypeParents` | performers 20–22 |
| `map_measurements`: all four values, and bust only | performers 7, 8 |
| `map_roleTagAssignments`, with a repeated entry | scene 100 |
| `map_autoRoleAssignments` | scene 100, role 8:3 |
| `map_actionAssignments`: variations merged, none | scene 100 |
| `map_pluggableAssignments` | scene 100 |
| `map_actionPairings`: default direction, `give` | scene 100, p1 and p2 |
| `map_roleTagPairings`: no direction | scene 100, p3 |
| `map_relationAssignments`: automatic with rule, manual | scene 100 |
| `map_timeline`: two Stages, the same tag on both, a pick with no variation, all four slot kinds, seconds, a marker | scene 100 |
| a slot with no seconds | image 200 |
| the same performer with different roles in two scenes (US1-2) | scenes 100, 101 |
| a performer named only by an assignment | scene 101, performer 9 |

## Records with bad fields (user story 3)

Each sits in a record that also has good fields, so the tests show the rest is still mapped.
`expected-notes.json` lists the 22 notes they give.

| Note reason | Record |
|---|---|
| `unreadable` | tag 16 `variations`; performer 31 `archetypeParents`; scene 300 `roleTagAssignments`; scene 302 `actionAssignments` and `timeline` (all of its fields) |
| `badShape` | tag 15, a trait rule with no id; scene 301, a pairing direction `sideways` and a timeline entry with no id |
| `invalidId` | tag 16 `variantOfTagId` `abc`; scene 301, performer key `x7`, tag id `7 a`, pairing id `a/b` |
| `unknownKind` | scene 301, slot `t1` (`camera-angle`) |
| `retired` | scene 301, slot `t2` (the old `mise-en-scene` ref) |
| `danglingRef` | scene 301, pairing `q1` (no such action instances) and slot `t5` (names `q1`) |
| `tagMissing` | scene 301, a role naming tag 999 |
| `tagWrongType` | tag 15, categories on a Label; performer 30, archetype parents on a non-Archetype; scene 301, a role naming Action tag 4 |
| `performerNotInMedia` | scene 301, an action for performer 9 |
| `notRecordedRole` | scene 301, an automatic role 8:1 not in `roleTagAssignments` |

The retired keys `isGeneric` and `looks` cannot appear: the reader never returns them (spec 005
`NOT_READ`), so the reader's record form has no place for them.

## Failure check (T035, 2026-10-26)

Rules were broken on purpose, then restored:
1. **Action variations dropped** in `map.rs`: `build_fixture` failed and listed the two missing
   lines (`scene:100/action:7:4 sem:hasVariation "making out"` and `"peck"`). `build_coverage`
   still passed, as expected, because Stage picks and trait rules also use `sem:hasVariation`.
2. **`variantOfTagId` skipped:** `build_coverage` failed, naming
   `sem:map_variantOfTagId (sem:variantOfTag)`.

After both were reverted, the tests passed again.
