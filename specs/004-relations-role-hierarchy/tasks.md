# Tasks: Relations, Role Hierarchy, and Auto-Apply

**Input**: Design documents from `/specs/004-relations-role-hierarchy/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R13), data-model.md,
contracts/vocabulary-terms.md, contracts/operations-1.1.md, quickstart.md

**Tests**: included. Constitution Principle VII requires tests first, run without a live Stash
against recorded fixtures. Each test task is written to fail before the code that makes it pass.
Live checks against the test server (`http://localhost:9998`) are opt-in (`--ignored`).

**Organization**: tasks are grouped by user story:
- **US1 (P1):** define role hierarchies and relations;
- **US2 (P1):** auto-apply on save;
- **US3 (P2):** recursive search expansion;
- **US4 (P2):** recommendations.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1–US4

---

## Phase 1: Setup

- [X] T001 Create the module skeleton:
  - `crates/engine/src/relations/mod.rs` declaring `defs`, `hierarchy`, `evaluate`, `diff` and
    `explain` (empty modules);
  - `crates/engine/src/recommend.rs`;
  - `pub mod relations; pub mod recommend;` in `crates/engine/src/lib.rs`;
  - `crates/stash-adapter/src/relations.rs`, registered in `crates/stash-adapter/src/lib.rs`.

  `cargo build --workspace` passes.
- [X] T002 [P] Write offline fixtures in `crates/engine/tests/fixtures/relations/`, as JSON shaped
  like the adapter's types:
  - **`example1.json`:** tags for Customer, Patient (parent Customer), Professional, Nurse and
    Doctor (parent Professional), all `isRole`. Full Service (`isRelation`, ends Customer and
    Professional) and Extra Care (`isRelation`, parent Full Service, ends Patient and Nurse).
  - **`example2.json`:** Roles Smaller and Larger; Labels Short and Tall; Action Kissing, plus a
    variant tag whose `variantOfTagId` is Kissing. Relation Size Difference with ends Smaller
    and Larger and rule `{"id":"r1","a":{"label":Short},"b":{"label":Tall}}`. Performers P1
    (Short) and P2 (Tall).
  - **`scenes.json`:** scenes with `roleTagAssignments`, `actionAssignments` and
    `actionPairings` in the fixture shapes (`{"7":["19"]}`, pairings with
    `a`/`b`/`direction`/`pairingId`). Cover:
    - paired on Kissing;
    - reversed direction;
    - both directions;
    - three performers;
    - the same Action without a pairing;
    - paired on the variant tag;
    - a hand-applied relation;
    - a hand-held end role;
    - a scene already up to date.
  - **`invalid.json`:** a role loop, a relation with a missing end, an end that is not a Role, a
    narrower relation whose end is not narrower, and a rule with both `label` and `role`.
- [X] T003 [P] Write recorded GraphQL response fixtures for the new adapter operations in
  `crates/stash-adapter/tests/fixtures/relations/`:
  - flag-filtered tags;
  - scenes and images by ids;
  - performers by ids;
  - item ids by `updated_at`;
  - a scene's native tags;
  - variant tags.

  They are captured read-only from the test server where possible, or hand-written in the same
  shape, with no credentials.

---

## Phase 2: Foundational (blocks every story)

- [X] T004 Add the vocabulary 1.2.0 classes and properties from
  `specs/004-relations-role-hierarchy/contracts/vocabulary-terms.md` to `vocabulary/core.ttl`:
  - the terms: `Relation`, `RelationInstance`, `RelationRule`, `broaderRole`, `broaderRelation`,
    `endARole`, `endBRole`, `hasRelationRule`, `relationRuleId`, `endALabel`,
    `endARoleCondition`, `endBLabel`, `endBRoleCondition`, `linkingAction`, `ofRelation`,
    `atEndA`, `atEndB`, `appliedAutomatically`, `appliedByRule`;
  - the required annotations on each: `sem:source "specs/004-relations-role-hierarchy/spec.md
    (owner, 2026-10-21)"`, `sem:sourceRevision`, `sem:status sem:Active`, `sem:since "1.2.0"`;
  - one domain and one range per property, and a `sem:scope` on every property and assignment
    class;
  - `sem:version` set to `"1.2.0"`.
- [X] T005 Add the five mappings to `vocabulary/stash-mapping.ttl`: `map_isRelation`,
  `map_relationEnds`, `map_relationRules`, `map_relationAssignments`, `map_autoRoleAssignments`.
  Use the allowed values exactly as in the contract, and add the `rdfs:comment` on
  `map_tagParents` about same-type parents producing `broaderRole` and `broaderRelation`.
- [X] T006 [P] Write failing tests in `crates/semantic-vocabulary/tests/iri_scheme.rs` for
  `ItemIris::relation_instance(media, relation, a, b)` (`{media}/relation:{relation}:{a}:{b}`)
  and `ItemIris::relation_rule(relation, rule_id)` (`{relation}/rule:{ruleId}`). Then implement
  both in `crates/semantic-vocabulary/src/iri.rs`, following the existing derived-IRI scheme.
- [X] T007 Release the vocabulary:
  1. Add the `## 1.2.0 (2026-10-21)` entry to `vocabulary/CHANGELOG.md`, under "Deliberate
     additions (constitution Principle II)", noting that the plugin documents at `3c82c85` have
     no Relations.
  2. Run `cargo run -p semantic-vocabulary --bin vocab-check -- --write-terms --write-snapshot`
     to regenerate `vocabulary/TERMS.md` and `vocabulary/released/1.2.0.json`.
  3. Run `vocab-check`. It must pass every rule from V001 to V014, and
     `cargo test -p semantic-vocabulary` must pass.
- [X] T008 [P] Add `ErrorCode::NotFound` ("NOT_FOUND") to `crates/engine/src/errors.rs`, with a
  test in `crates/engine/tests/versions.rs` or a new `crates/engine/tests/errors.rs`. Set the
  operations contract version to `"1.1.0"` in `crates/engine/src/versions.rs`, in
  `api/operations.json` (`contractVersion`, `errors`) and in `docs/api.md` (version line and
  error table). `cargo test -p plugin --test api_agreement` must still pass.
- [X] T009 [P] Write failing tests in `crates/stash-adapter/tests/relations.rs`, using the T003
  fixtures, for `Client::relation_definitions()`. It makes one flag-filtered `findTags` read per flag (7 in
  all, because custom-field criteria combine only with AND) of tags flagged `isRole`, `isRelation`, `isLabel`, `isAction`, `plugtag` or `isMiseEnScene`, or having
  `variantOfTagId`,
  returning id, name, parent ids and custom fields. Implement it as a typed Cynic operation in
  `crates/stash-adapter/src/relations.rs`. No per-tag requests (Principle VII).
- [X] T010 [P] Write failing tests in `crates/engine/tests/relations_defs.rs`, using
  `example1.json`, `example2.json` and `invalid.json`, for `relations::defs::Definitions::from_tags`.
  It parses Roles, Relations (from `relationEnds` `{"a": roleId, "b": roleId}`) and rules (from
  `relationRules`). Each rule's `a`/`b` has "exactly one of `label` … or `role`", and its `id`
  is "required; unique per relation". The function reports problems of kind `missingEnd`,
  `endNotRole` and `badRule`. Then implement it in `crates/engine/src/relations/defs.rs`.
- [X] T011 Write failing tests in `crates/engine/tests/relations_hierarchy.rs` for
  `relations::hierarchy`. Then implement it in `crates/engine/src/relations/hierarchy.rs`.
  - **Broader terms:** only a parent "also flagged `isRole`" (or `isRelation`) is a broader term,
    and a tag may have several.
  - **Narrower closure:** `narrower(tag, recursive)` and `broader_closure(tag)` work at any
    depth.
  - **Loops:** found by a depth-first walk; every link on a loop is ignored and reported as kind
    `loop`.
  - **Relation validity:** a relation is `invalidEnds` unless "each end must be the same role as,
    or narrower than, the matching end of every broader relation".
  - **Example 1:** Patient's broader is Customer, Extra Care is valid under Full Service, and
    `invalid.json` reports one loop with the other tags unaffected.

**Checkpoint**: vocabulary 1.2.0 passes the checks. The engine can read definitions and resolve
both hierarchies offline.

---

## Phase 3: User Story 1 - Define role hierarchies and relations (Priority: P1) 🎯 MVP

**Goal**: The owner defines hierarchical roles and relations in Stash, and the engine reports
them, with every problem found.

**Independent Test**: Quickstart step 1. With Example 1 defined on the test server, `relations`
reports Patient → Customer, Extra Care → Full Service (valid), the ends in order, and no
problems. Adding a loop adds exactly one `loop` problem.

### Tests for User Story 1

- [X] T012 [P] [US1] Write the contract test `crates/plugin/tests/ops_relations.rs`, offline
  through a fixture-backed client, for the `relations` operation.
  - **Envelope:** the 1.0.0 envelope with `contractVersion` 1.1.0.
  - **Result:** `roles` (`id`, `name`, `broader`, `narrower`), `relations` (`ends`,
    `broader`, `narrower`, `valid`, `rules`) and `problems` (`kind` among `loop`,
    `missingEnd`, `endNotRole`, `invalidEnds`, `badRule`).
  - **`tag` argument:** narrows the result, and an unknown tag is `INVALID_INPUT`.
  - **`recursive` argument:** switches `narrower` between direct children and every depth.

### Implementation for User Story 1

- [X] T013 [US1] Implement the `relations` operation in `crates/plugin/src/ops.rs` and add it to
  `REGISTRY`. It reads definitions once through `Client::relation_definitions()` and builds the
  result with `defs` and `hierarchy`. Unreachable Stash gives `STASH_UNREACHABLE`.
- [X] T014 [US1] Add `relations` to `api/operations.json` and `docs/api.md`, with its arguments,
  result shape, problem kinds and errors, as in `contracts/operations-1.1.md`.
  `cargo test -p plugin --test api_agreement` must pass.
- [X] T015 [US1] Live check, opt-in: add `relations_definitions` to
  `crates/plugin/tests/live_relations.rs`. Behind `--ignored` and the `STASH_TEST_*` variables,
  it creates the Example 1 tags on the test server, calls `relations`, asserts the result,
  then deletes the tags. Writes go to the test server only.

**Checkpoint**: US1 is complete and usable alone. Definitions are visible and checked.

---

## Phase 4: User Story 2 - Apply relations automatically when a scene is saved (Priority: P1)

**Goal**: Saving a scene or image starts one background job per burst. The job writes the
relations, and their end roles, that the rules call for, and removes the ones that no longer
hold.

**Independent Test**: Quickstart steps 2–4. Example 2 is applied within 30 s with P1 at Smaller
and P2 at Larger. Removing the pairing removes both. Saving again changes nothing. The save cost
is at most 7 ms per item, and a 1,000-scene bulk edit finishes within 5 minutes.

### Tests for User Story 2

- [X] T016 [P] [US2] Write failing tests in `crates/engine/tests/relations_evaluate.rs`, using
  `scenes.json`, for `relations::evaluate::matches(item, defs, hierarchy, labels)`. It checks
  every ordered pair p ≠ q.
  - **Example 2:** Size Difference with a = P1, b = P2, rule r1.
  - **Reversed direction:** P2 at Smaller.
  - **Both directions:** one match per direction; a performer is never paired with themselves.
  - **Three performers:** every qualifying pair.
  - **Same Action, no pairing:** no match, because "Holding the same Action without a pairing
    does not link them".
  - **Variant:** paired on a variant tag counts as Kissing.
  - **`actions` filter:** a non-empty list restricts the linking Actions.
  - **Role conditions:** Patient meets a Customer condition.
  - **Repeated passes:** re-evaluation stops once nothing changes, capped at 4 passes. A
    still-changing chain is reported, not written.
- [X] T017 [P] [US2] Write failing tests in `crates/engine/tests/relations_diff.rs` for
  `relations::diff::plan(stored, desired)`.
  - **Adding:** new automatic instances get `"auto": true` and `ruleId`, and end roles are added
    to both `roleTagAssignments` and `autoRoleAssignments`.
  - **Removing:** instances no longer desired are removed, with their automatic roles.
  - **Hand-applied relations:** untouched, never duplicated, and an automatic entry for the same
    (relation, a, b) gives way to them.
  - **Hand-held roles:** never recorded in `autoRoleAssignments` and never removed.
  - **Uniqueness:** "at most one entry per (relationTagId, a, b)".
  - **No change:** an up-to-date item gives an empty patch (SC-006).
- [X] T018 [P] [US2] Write failing tests in `crates/engine/tests/relations_explain.rs` for
  `relations::explain::explain(item, instance, …)`. An automatic instance returns `ruleId`, each
  end's `met` (`label`, or `role` with `via` the narrower role held), `link` (`action`, `by:
  "pairing"`, `pairingId`) and `addedRoles`. A hand-applied one returns `applied: "hand"`; a
  missing one returns `applied: "none"`.
- [X] T019 [P] [US2] Write failing tests in `crates/plugin/tests/journal.rs` for
  `crates/plugin/src/journal.rs`, in a temporary folder.
  - **Appending:** `append` adds one `{type, id}` line to `journal.jsonl`.
  - **The marker:** `mark_pending` creates `apply.pending` and reports whether it was absent.
  - **Draining:** `drain` takes the journal by atomic rename.
  - **Finishing:** `finish` removes the marker, then re-checks the journal and reports whether
    entries arrived. A hook that appended during a drain is never lost.
  - **Watermark:** `apply-watermark` is read and written in Stash's local-time format.
- [X] T020 [P] [US2] Write failing tests in `crates/stash-adapter/tests/relations.rs`, using the
  T003 fixtures, for the typed Cynic operations below. Then implement them in
  `crates/stash-adapter/src/relations.rs`.
  - **`items_by_ids(kind, ids)`:** scenes or images in batches of 100, with `custom_fields`,
    performer ids and `updated_at`.
  - **`performer_tags(ids)`:** each performer's tag ids, for their Labels.
  - **`changed_since(kind, threshold)`:** item ids with `updated_at` greater than the threshold.
    The threshold is sent in the server's local-time format, one second early, and never with
    `ids`.
  - **`update_item_custom_fields(kind, id, partial)`:** a partial update of
    `relationAssignments`, `autoRoleAssignments` and `roleTagAssignments` only.
  - **`run_plugin_task(plugin_id, task_name)`.**

### Implementation for User Story 2

- [X] T021 [P] [US2] Implement `crates/engine/src/relations/evaluate.rs` until T016 passes. A link
  exists only through `actionPairings` (research R3); a Label condition reads the performer's
  tags; a Role condition reads `roleTagAssignments` with narrower roles.
- [X] T022 [P] [US2] Implement `crates/engine/src/relations/diff.rs` until T017 passes. The output
  is the minimal partial `custom_fields` patch, or nothing.
- [X] T023 [P] [US2] Implement `crates/engine/src/relations/explain.rs` until T018 passes, reusing
  `evaluate`.
- [X] T024 [P] [US2] Implement `crates/plugin/src/journal.rs` until T019 passes.
- [X] T025 [US2] Implement the `recordChange` hook operation in `crates/plugin/src/ops.rs`, and
  add it to `REGISTRY`. It reads the hook context (`type`, `id`) from Stash's input and appends
  to the journal. If `mark_pending` reports the marker was absent, it calls `run_plugin_task`
  for "Apply relations" on the invoking Stash only. It does no evaluation and no store work.
  Test it in `crates/plugin/tests/ops_record_change.rs`, offline: one append per call, and a
  task request only for the first call of a burst.
- [X] T026 [US2] Implement the "Apply relations" task in `crates/plugin/src/apply.rs`.
  1. Read definitions once.
  2. Drain the journal, and add items `changed_since` the watermark (no watermark means no
     catch-up).
  3. Process items in batches of 100. For each item, read performers' tags, then evaluate and
     diff.
  4. Before each write, re-read the item. If `updated_at` moved, re-evaluate instead of
     writing.
  5. Write only non-empty patches.
  6. Log progress lines with `Level::Progress`, and one summary line (`apply: N items, M
     changed, K skipped, T s`).
  7. Set the watermark to the run's start time.
  8. Call `finish`, and loop while entries arrived.
- [X] T027 [US2] Implement the "Re-apply relations" task in `crates/plugin/src/apply.rs`. It takes
  `scope` (`all`, `scenes` or `images`) and optional `ids`, walks the items in batches of 100
  with the same evaluation and writes, and reports progress. A stopped run is safe to rerun.
  Add an offline test in `crates/plugin/tests/apply.rs` with a fixture-backed client, checking
  that a rerun after a partial run gives the same final state and no duplicates.
- [X] T028 [US2] Implement the `explainRelation` operation in `crates/plugin/src/ops.rs`
  (`media`, `id`, `relationTagId`, `a` and `b`, all required). A missing item gives `NOT_FOUND`.
  Add `crates/plugin/tests/ops_explain.rs`.
- [X] T029 [US2] Update `plugin/semantic-engine.yml`:
  - **Tasks:** "Apply relations" (`op: applyRelations`) and "Re-apply relations"
    (`op: reapplyRelations`).
  - **Hook:** "Apply relations after a save", triggered by `Scene.Create.Post`,
    `Scene.Update.Post`, `Image.Create.Post` and `Image.Update.Post`, with `defaultArgs: { op:
    recordChange }`.

  Add both tasks, the hook, `explainRelation` and `recordChange` to `api/operations.json` and
  `docs/api.md`. `cargo test -p plugin --test api_agreement` must pass.
- [X] T030 [US2] Add a note under rule 6 in
  `specs/003-engine-design-spike/decision-record.md`: "Superseded 2026-10-21 by
  `specs/004-relations-role-hierarchy/research.md` R6: the hook records the change and starts a
  Stash background job; no client call pays for applying."
- [X] T031 [US2] Live checks, opt-in, in `crates/plugin/tests/live_relations.rs`. Install the
  plugin into the test instance with `scripts/install.sh` first. Clean up after each check.
  - **`auto_apply_example2`:** the relation and end roles appear within 30 s with no client
    call (SC-002).
  - **Removal:** removing the pairing removes them.
  - **`idempotent_saves`:** ten unchanged saves make no writes after the first pass (SC-006).
  - **`save_cost`:** median save time with the plugin enabled minus disabled, for a single save
    and for the first save of a burst, must be at most 7 ms (SC-003).
  - **`bulk_1000`:** a bulk edit of 1,000 test scenes is fully applied within 5 minutes by one
    job (SC-005).
- [X] T032 [US2] If T031's first-save-of-a-burst cost is over 7 ms, use the research R6
  fallback: the hook only appends, and the job is started by the next run's catch-up or by the
  owner. Record the measured figure and the choice in
  `specs/004-relations-role-hierarchy/research.md` R6. If it is within budget, record the
  measured figure there instead.

**Checkpoint**: US2 is complete. Auto-apply works on the test server within its budgets.

---

## Phase 5: User Story 3 - Search relations and roles recursively (Priority: P2)

**Goal**: The expansion that recursive search uses is in the engine, tested, and ready for Phase
3's search operation (research R1).

**Independent Test**: offline. Expanding Full Service recursively gives {Full Service, Extra
Care}; non-recursive gives {Full Service}; Role Customer recursive includes Patient.

- [X] T033 [P] [US3] Write failing tests in `crates/engine/tests/relations_expand.rs`, using
  `example1.json`, for `relations::hierarchy::expand_criterion(tag, recursive)`. It is the set
  of tag ids a search criterion matches; recursive is off by default. Then implement it in
  `crates/engine/src/relations/hierarchy.rs`.
- [X] T034 [US3] Document `expand_criterion` as the function Phase 3's search must call for
  Relation and Role criteria, in `crates/engine/src/relations/mod.rs` and in
  `ROADMAP.md`. Add to Phase 3's "SPARQL `SELECT`" items: "Relation and Role criteria accept
  `recursive` and expand through the engine's hierarchy (004 US3); verify 004 SC-004 and SC-007
  there."

**Checkpoint**: US3's engine part is done. Search itself ships with Phase 3.

---

## Phase 6: User Story 4 - Recommend tags from the engine, including Relations (Priority: P2)

**Goal**: The plugin's tag recommendations come from the engine, with Relations added. The
engine only reads native tags.

**Independent Test**: Quickstart step 5. On fixtures, the results for Actions, Roles, Pluggables
and Mise-en-scène equal the plugin's current recommendations, and a Relation in the native tags
is recommended.

### Tests for User Story 4

- [X] T035 [P] [US4] Write parity fixtures in `crates/engine/tests/fixtures/recommend/`. Read the
  plugin's `computeRecommendedTagIds` and `groupVariantsByCanonicalId` in
  `~/Projects/Stash-Plugins/semantic-tagging/src/ui/shared.js`, and their call sites in
  `actions.js`, `roles.js`, `pluggables.js` and `timeline.js`, and record the plugin commit read.
  For a set of items with native tags, record the plugin's expected recommended ids per category,
  plus Relation cases.
- [X] T036 [P] [US4] Write failing tests in `crates/engine/tests/recommend.rs`, using T035, for
  `recommend::recommend(native_tag_ids, vocabulary_tags, variants)`.
  - **Categories:** returns `actions`, `roles`, `pluggables`, `miseEnScene` and `relations`,
    each a list of `{tag, from}`.
  - **Variants:** a tag is recommended when it is native, or when a native tag's
    `variantOfTagId` points at it.
  - **Parity:** the four existing categories equal the plugin's (SC-008).
  - **Empty case:** no vocabulary tags means every list is empty.
- [X] T037 [P] [US4] Write failing tests in `crates/stash-adapter/tests/relations.rs` for
  `Client::item_native_tags(kind, id)`, a missing item being `NOT_FOUND`. Then implement it in
  `crates/stash-adapter/src/relations.rs`. Variant tags come from `relation_definitions()`
  (T009), so no extra request is needed.

### Implementation for User Story 4

- [X] T038 [US4] Implement `crates/engine/src/recommend.rs` until T036 passes.
- [X] T039 [US4] Implement the `recommendations` operation in `crates/plugin/src/ops.rs` and add it
  to `REGISTRY` (`media`, `id`). It makes no writes (FR-020). Add the contract test
  `crates/plugin/tests/ops_recommend.rs`, and add the operation to `api/operations.json` and
  `docs/api.md`.
- [X] T040 [US4] Live check, opt-in: add `recommendations_latency` to
  `crates/plugin/tests/live_relations.rs`. It runs 100 calls on a test scene, asserts p95 under
  200 ms (SC-008), and checks the scene's native tags are unchanged afterwards.

**Checkpoint**: US4 is complete. The plugin can switch its pickers to the engine.

---

## Phase 7: Polish & cross-cutting

- [X] T041 [P] Write `specs/004-relations-role-hierarchy/plugin-notes.md` for the semantic-tagging
  plugin (Principle II coordination):
  - the new tag flag and custom-field keys, with their JSON shapes;
  - that automatic end roles also appear in `roleTagAssignments`;
  - that recommendations are now available from the engine's `recommendations` operation, so the
    plugin can retire `computeRecommendedTagIds`.
- [X] T042 [P] Update `ROADMAP.md`:
  - **Phase 2:** "Relations and role hierarchy (004)".
  - **Phase 3:** the T034 note.
  - **Phase 4:** "Plugin: tag recommendations from the engine's `recommendations` operation
    (004 US4)".
- [X] T043 Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features --
  -D warnings`, `cargo test --workspace` and `vocab-check`. All must pass.
- [X] T044 Run `specs/004-relations-role-hierarchy/quickstart.md` steps 1–5 and 7 on the test
  server. Record each step's result in that file, as 002 and 003 did, and leave the test library
  as found.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (T001–T003):** no dependencies.
- **Foundational (T004–T011):**
  - needs Setup;
  - T004 → T005 → T007;
  - T006, T008, T009 and T010 are independent;
  - T011 needs T010.
  - It blocks every story.
- **US1 (T012–T015):** needs Foundational.
- **US2 (T016–T032):** needs Foundational.
  - It does not need US1's operation, but uses the same definitions.
  - Order: tests T016–T020 first, then T021–T024 in parallel, then T025 → T026 → T027, T028,
    T029, then T030, then live checks T031 → T032.
- **US3 (T033–T034):** needs T011.
- **US4 (T035–T040):** needs T009. It is independent of US1–US3.
- **Polish (T041–T044):** after the stories it covers. T044 is last.

### User Story Dependencies

- **US1 (P1), US2 (P1):** both need only Foundational. US2's live checks use tags like US1's, but
  create their own.
- **US3 (P2):** needs only the hierarchy (T011).
- **US4 (P2):** needs only the adapter's definition read (T009).

### Within Each User Story

- Tests are written and fail before the implementation.
- Engine library code comes before plugin operations; plugin operations come before manifest
  and API document updates.
- Live checks run last, one at a time: they share the test server.

### Parallel Opportunities

- **Setup:** T002 and T003.
- **Foundational:** T006, T008, T009 and T010.
- **US2 tests:** T016–T020 (five different files).
- **US2 implementation:** T021–T024 (four different files).
- **US3 and US4:** can run alongside US2 once Foundational is done.
- **Polish:** T041 and T042.

---

## Parallel Example: User Story 2

```text
# Tests together (different files):
T016 crates/engine/tests/relations_evaluate.rs
T017 crates/engine/tests/relations_diff.rs
T018 crates/engine/tests/relations_explain.rs
T019 crates/plugin/tests/journal.rs
T020 crates/stash-adapter/tests/relations.rs

# Then implementations together:
T021 crates/engine/src/relations/evaluate.rs
T022 crates/engine/src/relations/diff.rs
T023 crates/engine/src/relations/explain.rs
T024 crates/plugin/src/journal.rs
```

---

## Implementation Strategy

### MVP first (US1, then US2)

1. Setup and Foundational give vocabulary 1.2.0, definitions, and the hierarchy.
2. US1: the `relations` operation. **Stop and check:** the owner defines Example 1 in Stash and
   sees it reported correctly.
3. US2: auto-apply. **Stop and check:** Example 2 applies on save within its budgets. This is the
   feature's main value.

### Incremental delivery

1. **US3** hands the expansion to Phase 3, with no client-visible change yet.
2. **US4** lets the plugin switch its recommendations to the engine.
3. **Polish:** coordinate with the plugin, update the roadmap, and record the quickstart results.

### Notes

- **Writes:** writes to Stash happen only in the "Apply relations" and "Re-apply relations" tasks,
  through typed Cynic operations (constitution Principle I and the Cynic constraint).
- **Credentials:** never print or commit them. Live tests read them from `STASH_TEST_*`.
