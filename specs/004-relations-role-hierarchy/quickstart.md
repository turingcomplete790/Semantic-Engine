# Quickstart: Relations, Role Hierarchy, and Auto-Apply

This guide checks the feature end to end on the local test server. Operations, tasks and the hook
are defined in [contracts/operations-1.1.md](contracts/operations-1.1.md); stored data is
described in [data-model.md](data-model.md).

Last full run: 2026-10-23, against the Stash v0.31.1 test instance. Every step passed. Steps 1 to
5 are automated as opt-in live checks:

```sh
cargo test -p plugin --test live_relations -- --ignored --test-threads=1 --nocapture
```

Each check creates its own uniquely named tags, performers and scenes (prefix `SE-004`) and
deletes them afterwards. The checks run one at a time, because they share the test server and
the plugin's on/off switch.

## Prerequisites

- **Test server:** Stash v0.31.1 at `http://localhost:9998`. Its credentials are in
  `Test-Instance/creds.txt`; export them into the environment and never paste them.
- **Plugin:** this branch's plugin, installed into the test instance with `scripts/install.sh`.
  Then reload plugins in Stash.
- **Writes:** the test library only (constitution; the same rule as 003).

```sh
export STASH_TEST_URL=http://localhost:9998
export STASH_TEST_API_KEY=...        # from creds.txt
cargo test --workspace               # offline tests: hierarchy, evaluation, diff, vocabulary checks
cargo run -p semantic-vocabulary --bin vocab-check
```

**Expected:** all tests pass, and `vocab-check` reports vocabulary **1.2.0** with no violations.

Result: 206 offline tests pass; strict clippy (`--all-features -D warnings`) is clean; `vocab-check`
reports `all rules pass` on 1.2.0.

## 1. Define Example 1 (US1)

In Stash, create these tags:

| Tag | Flag | Stash parent | Relation fields |
|---|---|---|---|
| Customer | `isRole` | — | — |
| Patient | `isRole` | Customer | — |
| Professional | `isRole` | — | — |
| Nurse | `isRole` | Professional | — |
| Doctor | `isRole` | Professional | — |
| Full Service | `isRelation` | — | `relationEnds` = Customer, Professional |
| Extra Care | `isRelation` | Full Service | `relationEnds` = Patient, Nurse |

Then ask the engine for the definitions:

```graphql
mutation { runPluginOperation(plugin_id: "semantic-engine", args: { op: "relations" }) }
```

**Expected:**
- Patient's `broader` is Customer.
- Extra Care's `broader` is Full Service, and it is `valid`.
- `problems` is empty.
- Making Customer a child of Patient adds a `loop` problem, and every other entry stays the same.

Result (`relations_definitions`): as expected on the test server. Patient lists Customer as
broader; Extra Care is valid under Full Service with ends Patient and Nurse; the role lookups
narrow correctly. The loop case is covered offline (`relations_hierarchy`, `ops_relations`).

## 2. Auto-apply Example 2 (US2)

1. **Define Size Difference:** create Roles Smaller and Larger, and Labels Short and Tall. Create
   the Relation Size Difference with ends Smaller and Larger, and the rule
   `{"id": "r1", "a": {"label": Short}, "b": {"label": Tall}}`.
2. **Set up the performers:** give performer P1 the Label Short and P2 the Label Tall.
3. **Save a test scene:** in one test scene, give P1 and P2 the Action Kissing, linked by an
   action pairing, and save the scene in Stash's web UI.

**Expected, within 30 seconds (SC-002), with no client open:**
- The scene's `relationAssignments` holds `{relationTagId: Size Difference, a: P1, b: P2, auto:
  true, ruleId: "r1"}`.
- `roleTagAssignments` gives P1 Smaller and P2 Larger.
- `autoRoleAssignments` lists those two roles.
- Stash's job list shows one finished "Apply relations" job.

Then:
- **Remove the action and save again:** the automatic relation and both automatic roles are
  gone. Any roles set by hand stay.
- **Save unchanged 10 times:** no new entries and no writes after the first pass (SC-006).
- **`explainRelation` on the instance:** returns rule `r1`, the two Labels, and the pairing.

Result (`auto_apply_example2`, `idempotent_saves`):
- **Applied:** the relation, both end roles and the automatic role record appeared 0.1–0.26 s
  after the save, with no client open.
- **Removal:** removing the pairing removed them within 0.1–0.26 s.
- **Unchanged saves:** ten saves left the scene's `updated_at` at the last save's value, so the
  engine wrote nothing.
- **`explainRelation`:** covered offline by the contract test (`ops_explain`) and the engine
  tests (`relations_explain`). It was not run against the live server.

## 3. Bulk edit and save cost (SC-003, SC-005)

```sh
cargo test -p plugin --test live_relations -- --ignored --test-threads=1 --nocapture save_cost bulk_1000
```

**Expected:**
- **Save cost:** the extra time per saved item with the plugin enabled is at most 7 ms. This is
  measured for single saves and for the first save of a burst, which starts the job.
- **Bulk edit:** after a 1,000-scene bulk edit that meets a rule, every auto-apply is written
  within 5 minutes, by one "Apply relations" job.

Result (`save_cost`, `bulk_1000`), median save time:

| Save | Median | Extra from the plugin |
|---|---|---|
| Plugin disabled | 1.3–1.5 ms | baseline |
| Hook only records the change | 3.0–3.2 ms | +1.5 to +1.8 ms |
| First save of a burst (also starts the job) | 6.5–6.8 ms | +5.1 to +5.3 ms |

- **Save cost:** within the 7 ms budget. This needed a fix: the first measurement of a burst's
  first save was +14.2 ms, because building the HTTP client loaded TLS certificates for a plain
  `http://` address (research R6).
- **Bulk edit:** 1,000 scenes were saved in 2.0–3.1 s and fully applied 6.0–7.9 s after the bulk
  edit.

## 4. Re-apply after a definition change (FR-011)

1. Add a second rule to Size Difference. Nothing changes in existing scenes.
2. Run the task "Re-apply relations" with `scope: scenes`.

**Expected:**
- The job reports progress.
- Scenes matching the new rule gain the relation.
- Stopping the job part-way and running it again finishes the rest, with no duplicates.

Result (`reapply_after_a_rule_change`):
- **No automatic re-check:** after the rule was removed, the scene kept its now-stale automatic
  relation, as expected.
- **Re-apply:** running "Re-apply relations" with `scope: scenes` and the scene's id removed the
  relation and its roles 0.26 s later.
- **Stop and rerun:** a stopped and re-run job is covered offline (`apply`:
  `a_stopped_run_completed_later_gives_the_same_result`).

## 5. Recommendations (US4)

1. Give a test scene, as native Stash tags: the Action tag Cowgirl, a tag marked as a common
   variant of Cowgirl, and the Relation tag Size Difference.
2. Ask for recommendations:

```graphql
mutation { runPluginOperation(plugin_id: "semantic-engine", args: { op: "recommendations", media: "scene", id: "<id>" }) }
```

**Expected:**
- Cowgirl is under `actions`, once for each native tag that suggests it.
- Size Difference is under `relations`.
- The scene's native tags are unchanged.
- On the fixture library, the four existing categories match the plugin's recommendations
  (SC-008, offline test).

Result (`recommendations_latency`, `recommend`, `ops_recommend`):
- **Live:** with an Action and a Relation as native tags, both were recommended. 100 calls had a
  median of 23.6 ms and a p95 of 26.1 ms, and the scene's native tags were unchanged.
- **Offline parity:** all 9 parity cases match the plugin's rule at `3c82c85`. That rule resolves
  variants for Actions and Mise-en-scène only.

## 6. Recursive search (US3), with Phase 3

Recursive search is checked when Phase 3's search operation exists (research R1). Until then:
- `relations` with `tag: Full Service, recursive: true` lists Extra Care among `narrower`;
- the offline tests check the expansion against the Example 1 fixtures.

## 7. Clean up

Delete the test tags, or run "Re-apply relations" after removing their rules. The test
library's item counts are unchanged apart from the test tags.

Result: after every live run, no tags, performers or scenes named `SE-004` remain. The test
library still has 1,021 scenes and 17 tags. Its performer count rose by 1,100 during the same
period; those performers come from another extension's own tests of semantic tags, not from this
feature.
