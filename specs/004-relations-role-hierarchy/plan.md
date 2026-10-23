# Implementation Plan: Relations, Role Hierarchy, and Auto-Apply

**Branch**: `004-relations-role-hierarchy` | **Date**: 2026-10-21 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/004-relations-role-hierarchy/spec.md`

## Summary

This feature adds Relations, role and relation hierarchies, and automatic application of
relations when a scene or image is saved.

- **Vocabulary 1.2.0:** new terms for Relations, broader roles and relations, auto-apply rules,
  and relation instances. Each is mapped to new Stash keys:
  - tag flag `isRelation`;
  - tag fields `relationEnds` and `relationRules`;
  - scene and image fields `relationAssignments` and `autoRoleAssignments`;
  - hierarchy through Stash's own tag parents.
- **Engine library:** hierarchy resolution, rule evaluation, and the minimal change to write.
  These are pure functions, tested first against fixtures.
- **Plugin:**
  - a hook that records each save and starts one background job per burst;
  - the tasks "Apply relations" and "Re-apply relations";
  - the operations `relations`, `explainRelation` and `recommendations`, which moves the plugin's
    tag recommendations into the engine and adds Relations (research R13).

  The operations contract goes from 1.0.0 to 1.1.0.

Evaluation needs one scene's or image's data only, not the full semantic graph (research R1), so
auto-apply is built now. Recursive search ships with Phase 3's search operation, which calls this
feature's hierarchy expansion.

**Change to the 003 decision record:** applying changes moves from "the next engine call"
(rule 6) to a Stash background job started by the hook (research R6). The owner judged that a
1,000-scene edit adding seconds to some later call is unacceptable.

## Technical Context

**Language/Version**: Rust stable (edition 2021), the existing workspace

**Primary Dependencies**:
- existing crates: `semantic-vocabulary`, `engine`, `stash-adapter` (Cynic, pinned schema
  v0.31.1), `plugin`;
- `serde_json`;
- no new external dependency.

**Storage**:
- **Stash:** the system of record (data-model.md).
- **Plugin folder:** the journal, pending marker and watermark, all disposable.
- **No graph store:** this feature uses none (research R1).

**Testing**:
- `cargo test` with recorded fixtures: hierarchy, evaluation, diff, explain, vocabulary checks,
  and operation contract tests (Principle VII);
- opt-in live tests against the test server, for save cost and bulk timing.

**Target Platform**: the five release targets from 002. Measurements are taken on Linux.

**Project Type**: Rust Stash plugin (library crates plus a thin plugin binary)

**Performance Goals**:
- relation visible within 30 s of a save (SC-002);
- 1,000-scene bulk edit applied within 5 minutes (SC-005);
- recursive search p95 under 200 ms at 250k scenes, verified with Phase 3 (SC-007).

**Constraints**:
- no more than 7 ms added per saved item (SC-003; 003 rule 11);
- no apply work in saves or client calls (FR-012);
- bounded, flag-filtered reads with no N+1 (Principle VII);
- only the invoking Stash is contacted (Principle IV);
- every write goes through typed Cynic operations.

**Scale/Scope**:
- libraries up to 250k scenes (Production: 49,683 and growing);
- 4 hook triggers, 2 tasks, 3 operations;
- about 16 new vocabulary terms and 5 mappings.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Checked against
constitution v3.0.0.

| Principle | Status | Notes |
|---|---|---|
| I. Stash is the system of record | Pass | Every definition and result is stored in Stash. The plugin-folder files are disposable. Writes are partial `custom_fields` updates through GraphQL. |
| II. Plugin documents are the contract | Pass, deliberate addition | Relations, hierarchy and auto-apply are not in the documents at `3c82c85`. They are added deliberately and documented (research R12). Existing keys keep their shape, and automatic roles are tracked in a new key, not by changing `roleTagAssignments` (R2). |
| III. One engine, one set of answers | Pass | Evaluation and explanation live only in the engine. Clients read the results from Stash, and later query them through Phase 3. |
| IV. Reached only through Stash | Pass | Hooks, tasks and operations all run through Stash, and only the invoking Stash is called. Freshness comes from hooks, with `updated_at` catch-up (R7). Hook cost is measured (R6). |
| V. Rust, embedded, headless | Pass | Logic is in the `engine` library and all I/O is in `stash-adapter`; the plugin binary stays a thin shell. |
| VI. Versioned, explicit contracts | Pass | Operations go to 1.1.0 and the vocabulary to 1.2.0, both additive (R11). `NOT_FOUND` is added. |
| VII. Fast, bounded, test-first | Pass | Reads are flag-filtered and batched. Both tasks report progress and can be cancelled. Tests come first with fixtures. Every automatic relation can be explained (FR-014). The hook and apply budgets are stated and measured. |
| Technology: Cynic, bundled RocksDB | Pass | The new reads and writes are typed Cynic operations. No RocksDB use in this feature. |

No violations. Complexity Tracking not required.

**Re-check after Phase 1 design**: Pass.
- The data model adds keys and changes none (FR-017).
- The contracts are additive.
- The one design risk, the cost of the first hook in a burst including `runPluginTask`, has a
  measurement task and a recorded fallback (R6).

## Project Structure

### Documentation (this feature)

```text
specs/004-relations-role-hierarchy/
├── plan.md
├── research.md                  # R1–R13
├── data-model.md                # Role, Relation, rule, instance, automatic roles, journal files
├── quickstart.md                # end-to-end checks on the test server
├── contracts/
│   ├── vocabulary-terms.md      # vocabulary 1.2.0 additions and mappings
│   └── operations-1.1.md        # relations, explainRelation, recommendations, two tasks, the hook
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks
```

### Source Code (repository root)

```text
vocabulary/
├── core.ttl                     # new classes and properties; version 1.2.0
├── stash-mapping.ttl            # five new mappings; tagParents comment
├── CHANGELOG.md, TERMS.md, released/1.2.0.json
crates/
├── semantic-vocabulary/src/iri.rs        # relation-instance and relation-rule IRIs
├── engine/src/relations/
│   ├── mod.rs                   # public surface
│   ├── defs.rs                  # roles, relations and rules parsed from tags; validity problems
│   ├── hierarchy.rs             # broader and narrower closure, loop detection (R4)
│   ├── evaluate.rs              # matches for one item: ordered pairs, conditions, links (R3, R5)
│   ├── diff.rs                  # desired vs stored automatic state, minimal patch (R5)
│   ├── explain.rs               # explanation of one instance (R10)
├── engine/src/recommend.rs      # recommendations by category: direct match or common variant (R13)
├── engine/src/errors.rs         # NOT_FOUND
├── stash-adapter/src/relations.rs        # typed reads (tags, items, performers, updated_at ids) and writes (item custom_fields, runPluginTask)
└── plugin/src/
    ├── ops.rs                   # the operation registry
    ├── relations.rs             # relations, explainRelation, recommendations, recordChange handlers
    ├── journal.rs               # journal, pending marker, watermark (R6, R7)
    └── apply.rs                 # Apply and Re-apply task bodies: batches, progress, writes
plugin/semantic-engine.yml       # two tasks, one hook (four triggers)
api/operations.json, docs/api.md # contract 1.1.0
specs/003-engine-design-spike/decision-record.md   # note on rule 6 (superseded by 004 R6)
```

**Structure Decision**:
- **Library and plugin split:** evaluation is pure library code in `engine`, so it can be tested
  without Stash and reused by Phase 3's query layer. All Stash I/O stays in `stash-adapter`, and
  the plugin wires them to hooks, tasks and operations.
- **Graph store:** none in this feature, by design (R1).

## Complexity Tracking

No constitution violations to justify.
