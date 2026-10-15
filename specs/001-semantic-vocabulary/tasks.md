---

description: "Task list for the Semantic Vocabulary feature"
---

# Tasks: Semantic Vocabulary

**Input**: Design documents from `/specs/001-semantic-vocabulary/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. Constitution Principle VII requires tests before behaviour; each test task
precedes the work it covers and MUST fail first.

**Organization**: Grouped by user story. Paths follow plan.md (Cargo workspace at the repository
root, crate `crates/semantic-vocabulary/`, data in `vocabulary/`).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1 to US5, matching spec.md user stories

Plugin source for provenance: `~/Projects/Stash-Plugins/semantic-tagging/` (documents
`README.md`, `semantic-query-engine-roadmap.md`, `archetype-system-plan.md`, read at commit
`3c82c85` plus working copy, 2026-10-14). Record that exact revision string in every term's
`sem:sourceRevision`.

---

## Phase 1: Setup

**Purpose**: Workspace and crate skeleton

- [X] T001 Create the Cargo workspace root `Cargo.toml` (members: `crates/*`), `rustfmt.toml`, and `.gitignore` entry for `target/`
- [X] T002 Create crate `crates/semantic-vocabulary/Cargo.toml` (edition 2021; dependencies `oxttl`, `oxrdf`, `clap`, `serde`, `serde_json`, `thiserror`) with `src/lib.rs` and `src/bin/vocab-check.rs` stubs
- [X] T003 [P] Create empty vocabulary files with the shared prefix header from `contracts/vocabulary-files.md`: `vocabulary/core.ttl`, `vocabulary/labels.ttl`, `vocabulary/timeline.ttl`, `vocabulary/stash-mapping.ttl`
- [X] T004 [P] Create `vocabulary/CHANGELOG.md` with a `## 1.0.0 (2026-10-14)` entry stub and `vocabulary/TERMS.md` placeholder

---

## Phase 2: Foundational (blocking)

**Purpose**: Loader, rule framework, and IRI scheme that every story depends on

**CRITICAL**: No user story work begins until this phase completes.

- [X] T005 [P] Write failing test `crates/semantic-vocabulary/tests/parse.rs`: loading a directory returns all four files' triples; a deliberate syntax error in a temp file yields exit-code-2 style error (V001)
- [X] T006 [P] Write failing test `crates/semantic-vocabulary/tests/iri_scheme.rs`: golden IRIs for scene, image, performer, tag, marker, and each derived node pattern in `data-model.md` "Identity"; building twice yields identical strings (FR-014, SC-006)
- [X] T007 Implement loader in `crates/semantic-vocabulary/src/lib.rs`: `load(dir)`, `embedded()` using `include_str!` over `vocabulary/*.ttl`, term index (class, property, annotations) built from parsed triples
- [X] T008 [P] Implement item IRI builders in `crates/semantic-vocabulary/src/iri.rs` for every pattern in `data-model.md` "Identity" (server id is a lower-case hyphenated UUID read from Stash by the caller; rejects any other form, and `{id}` values containing `:` or `/`; tests use fixed example UUID `00000000-0000-4000-8000-000000000001`)
- [X] T009 Implement the rule framework in `crates/semantic-vocabulary/src/check.rs`: `Violation {rule, term, file, line, message}`, `Vocabulary::check()`, and rule V001 (parse) with exit codes per `contracts/vocabulary-check.md`
- [X] T010 Implement `crates/semantic-vocabulary/src/bin/vocab-check.rs` with `--dir`, `--format text|json`, `--write-terms` and the JSON output shape from `contracts/vocabulary-check.md`
- [X] T011 Add CI workflow `.github/workflows/vocabulary.yml` running `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test -p semantic-vocabulary`, and `cargo run -p semantic-vocabulary --bin vocab-check` (V014 passes with a notice until the first snapshot exists)

**Checkpoint**: loader, IRI scheme, and check runner work on empty vocabulary files.

---

## Phase 3: User Story 1 - One authoritative list of semantic terms (Priority: P1) MVP

**Goal**: Every class and property from `data-model.md` exists in Turtle with a definition and
scope, so a reader can answer scoping questions from the vocabulary alone.

**Independent Test**: Run `vocab-check`; then answer the SC-004 questions from `TERMS.md`
without opening the design documents.

### Tests for US1

- [X] T012 [P] [US1] Write failing test `crates/semantic-vocabulary/tests/meta_terms.rs`: every `sem:` annotation and structural term used in `vocabulary/*.ttl` (`source`, `sourceRevision`, `scope`, `status`, `since`, `replacedBy`, `retiredNote`, `references`, `version`, `Mapping`, `fromEntity`, `fromField`, `allowedValues`, `produces`, `SceneScoped`, `Global`, `Active`, `Deprecated`, `Planned`, `Derived`) is defined in the vocabulary with the required annotations (V003, V005, FR-017)
- [X] T013 [P] [US1] Write failing test `crates/semantic-vocabulary/tests/defined_terms.rs`: every class in `data-model.md` (core, labels, timeline tables) is defined in the file the table names (SC-001, V003, V012)
- [X] T014 [P] [US1] Write failing test `crates/semantic-vocabulary/tests/domain_range.rs`: every property has exactly one `rdfs:domain` and one `rdfs:range`; no `owl:` term appears (V002, V004, FR-013)
- [X] T015 [P] [US1] Write failing test in `crates/semantic-vocabulary/tests/domain_range.rs`: each property listed under "Participation and assignment" has the `sem:scope` stated in `data-model.md`; each assignment class has `sem:scope sem:SceneScoped` (V006, FR-004)

### Implementation for US1

- [X] T016 [US1] Define the vocabulary's own annotation and structural terms in `vocabulary/core.ttl` under a "Vocabulary metadata" heading, with the same required annotations as semantic terms; make T012 pass
- [X] T017 [US1] Write classes in `vocabulary/core.ttl`: `MediaEntity`, `Scene`, `Image` (with `sem:references`), `Performer`, `Tag`, the six flagged types, `Variation`, `Archetype`, `TagReference`, `Placeable`, `TagAssignment`, `RoleInstance`, `ActionInstance`, `PluggableInstance`, `MiseEnSceneInstance`, `Pairing`, `RolePairing`, `ActionPairing`, with `rdfs:label`, `rdfs:comment`, `sem:source`, `sem:sourceRevision`, `sem:scope`, `sem:status`, `sem:since` per `contracts/vocabulary-files.md`; the `MiseEnSceneInstance` comment states the derivation rule from Stage picks (FR-015)
- [X] T018 [US1] Write participation and assignment properties in `vocabulary/core.ttl`: `hasPerformer`, `hasAssignment`, `hasRole`, `hasAction`, `hasPluggable`, `hasMiseEnScene`, `fromPick`, `inMedia`, `forPerformer`, `ofTag`, `hasVariation`, `atTime`, `pairs`, `pairingDirection`, `pairingId`, each with exactly one domain and range from `data-model.md`; `hasRole`, `hasAction`, `hasPluggable` as `rdfs:subPropertyOf sem:hasAssignment`
- [X] T019 [US1] Write performer-global and tag-metadata properties in `vocabulary/core.ttl`: `hasTrait`, `hasLabel`, `hasPerformerVariation`, `hasVariationPluggable`, `hasVariationLabel`, `variationOf`, `parentArchetype`, `hasCategory`, `hasTagVariation`, `parentTag`, `variantOfTag`; state in `rdfs:comment` which tag types carry categories and variations
- [X] T020 [P] [US1] Write Label classes and properties in `vocabulary/labels.ttl`: `LabelRule`, `TraitRule`, `FieldRule`, `LabelDependencyRule`, `PluggableRule`, `hasThreshold` (comment: default 100 when unset), `hasRule`, `hasWeight`, `isRecursive`, `hasField`, `hasModifier`, `hasValue`, `hasValue2`; `FieldRule` fields reference Stash filter terms, not redefine them (FR-009)
- [X] T021 [P] [US1] Write Timeline classes and properties in `vocabulary/timeline.ttl`: `Timeline`, `TimelineEntry`, `Slot`, `Stage`, `StagePick`, `hasTimeline`, `hasEntry`, `position` (xsd:nonNegativeInteger, zero-based), `entryId`, `places`, `slotSeconds`, `linkedMarker`, `hasPick`, `inStage` (status `sem:Derived`), per FR-015 and `research.md` R8
- [X] T022 [US1] Declare the vocabulary version once on `<urn:semantic-stash:vocab:>` in `vocabulary/core.ttl` (`sem:version "1.0.0"`)
- [X] T023 [US1] Implement rules V003, V004, V006, V012, V002, and V005 for meta terms in `crates/semantic-vocabulary/src/check.rs`; make T012 to T015 pass
- [X] T024 [US1] Implement `TERMS.md` generator in `crates/semantic-vocabulary/src/terms_md.rs` (term, kind, definition, scope, status, source) and rule V011; generate `vocabulary/TERMS.md` with `--write-terms` (FR-011)

**Checkpoint**: US1 is complete and independently verifiable; this is the MVP.

---

## Phase 4: User Story 2 - Machine-readable and valid (Priority: P1)

**Goal**: Any standard parser loads the vocabulary; the automated check passes on every
revision.

**Independent Test**: `cargo run -p semantic-vocabulary --bin vocab-check` exits 0, and an
independent Turtle validator parses all four files with no warnings.

### Tests for US2

- [X] T025 [P] [US2] Write failing test `crates/semantic-vocabulary/tests/check_exit_codes.rs`: clean directory exits 0; a directory with a missing annotation exits 1 with rule id; a directory with a syntax error exits 2
- [X] T026 [P] [US2] Write failing test `crates/semantic-vocabulary/tests/embedded.rs`: `embedded()` equals the on-disk `vocabulary/` load (same triples), so the engine can link the crate without file access

### Implementation for US2

- [X] T027 [US2] Implement the `--format json` writer and exit-code mapping in `crates/semantic-vocabulary/src/bin/vocab-check.rs`; make T025 pass
- [X] T028 [US2] Make `embedded()` in `crates/semantic-vocabulary/src/lib.rs` pass T026 (build script or `include_str!` paths relative to the workspace)
- [X] T029 [P] [US2] Add a second-parser check `scripts/validate-ttl.sh` using Raptor's `rapper -i turtle -c` on each `vocabulary/*.ttl`, failing on any error or warning, and call it from `.github/workflows/vocabulary.yml` (CI installs `raptor2-utils`); also add a rapper-parsed triple-count comparison against the Rust loader in `crates/semantic-vocabulary/tests/second_parser.rs`, skipped with a notice when `rapper` is not installed
- [X] T030 [US2] Run the full check; fix any violations of the rules implemented so far (V001 to V012; V013 and V014 arrive later) in `vocabulary/*.ttl` until exit code 0

---

## Phase 5: User Story 3 - Traceable to the design documents (Priority: P2)

**Goal**: Every term shows where it came from and which revision; superseded and planned terms
are marked.

**Independent Test**: Pick ten terms at random in `TERMS.md`; each shows a source section and
revision; `hasLook` shows its replacement.

### Tests for US3

- [X] T031 [P] [US3] Write failing test `crates/semantic-vocabulary/tests/provenance.rs`: every term has exactly one `rdfs:label` (`@en`), one `rdfs:comment`, at least one `sem:source`, exactly one `sem:sourceRevision`, one `sem:status`, one `sem:since` (V005, SC-003)
- [X] T032 [P] [US3] Write failing test in `crates/semantic-vocabulary/tests/provenance.rs`: every `sem:Deprecated` term has `sem:replacedBy` or `sem:retiredNote`, the replacement is defined and active; every `sem:Planned` term's comment starts "Planned:" (V007, V008)

### Implementation for US3

- [X] T033 [US3] Implement rules V005, V007, V008 in `crates/semantic-vocabulary/src/check.rs`; make T031 and T032 pass
- [X] T034 [US3] Add deprecated terms to `vocabulary/core.ttl`: `Generic` (replacedBy `Archetype`), `Look` (replacedBy `Variation`), `hasLook` (replacedBy `hasPerformerVariation`), `hasArchetype` (retiredNote: removed in Milestone 1D); none has a mapping entry (FR-016)
- [X] T035 [US3] Add planned terms to `vocabulary/core.ttl`: `hasTag` (comment "Planned: derived tags from CONSTRUCT, Milestone 5") and any Timeline details left open per `research.md` R8
- [X] T036 [US3] Audit pass: for each term in `vocabulary/*.ttl`, confirm `sem:source` names the exact document section; correct vague entries; record any document-versus-code disagreement in `vocabulary/CHANGELOG.md` (Principle II)

---

## Phase 6: User Story 4 - Stash data mapping (Priority: P2)

**Goal**: A builder knows which Stash field produces which term, with allowed values.

**Independent Test**: For each tag-type flag, the mapping gives field, allowed values, and
produced term; every key in the captured fixture library is mapped.

### Tests for US4

- [X] T037 [P] [US4] (live keys captured from the test instance 2026-10-15; the remaining keys are derived from plugin source because the test library has no assignments, pairings, timelines, or formulas) Capture a read-only fixture of a plugin-tagged library's `custom_fields` keys into `crates/semantic-vocabulary/tests/fixtures/library-keys.json` (flags, `variantOfTagId`, `labelFormula`, `archetypeParents`, assignments, pairings, timeline)
- [X] T038 [P] [US4] Write failing test `crates/semantic-vocabulary/tests/mapping.rs`: every key in the fixture has a mapping entry or is on the explicit not-mapped list (`isGeneric`, Looks JSON, legacy `mise-en-scene` refs); each mapping names a defined term and an allowed-value list (V010, SC-005, FR-016)

### Implementation for US4

- [X] T039 [US4] Write mapping entries in `vocabulary/stash-mapping.ttl` for every row of `data-model.md` "Stash mapping" using the `sem:Mapping` shape in `contracts/vocabulary-files.md`, including the planned entry for the server identifier setting (`sem:status sem:Planned`, comment beginning "Planned:")
- [X] T040 [US4] Implement rule V010 in `crates/semantic-vocabulary/src/check.rs`; make T038 pass
- [X] T041 [US4] Confirm each mapped field name against the plugin source and documents; where they differ, follow the documents and list the difference in `vocabulary/CHANGELOG.md`
- [X] T042 [US4] Add a "Not mapped (retired)" section to the generated `vocabulary/TERMS.md` listing `isGeneric`, Looks JSON, and legacy slot refs, with the reason

---

## Phase 7: User Story 5 - Stable and versioned (Priority: P3)

**Goal**: Version and change log let clients detect and adapt to change.

**Independent Test**: Rename a property in a scratch copy; the check fails V009 until the
version and change log are updated.

### Tests for US5

- [X] T043 [P] [US5] Write failing test `crates/semantic-vocabulary/tests/release_compat.rs` for V014: a removed or renamed term, or an incompatible domain or range, without a higher MAJOR fails and names the term; an added term without a higher MINOR fails; matching version and change log passes; no snapshot present passes with a notice
- [X] T044 [P] [US5] Write failing test `crates/semantic-vocabulary/tests/version.rs`: version is declared exactly once, and equals the top heading of `vocabulary/CHANGELOG.md` (V009)

### Implementation for US5

- [X] T045 [US5] Implement V014 and `--write-snapshot` in `crates/semantic-vocabulary/src/check.rs` and `src/bin/vocab-check.rs`; generate `vocabulary/released/1.0.0.json`; document the release procedure (bump version, update `CHANGELOG.md`, write snapshot) at the top of `vocabulary/CHANGELOG.md`; make T043 pass
- [X] T046 [US5] Implement rule V009 in `crates/semantic-vocabulary/src/check.rs`; make T044 pass
- [X] T047 [US5] Write `vocabulary/CHANGELOG.md` 1.0.0 entry: initial release, the list of divergences from the documents (reified Pluggable and Mise-en-scène assignments), deprecated terms with replacements, and migration notes for the viewer and the plugin

---

## Phase 8: Polish and cross-cutting

- [X] T048 [P] Implement rule V013 in `crates/semantic-vocabulary/src/check.rs` (no redefinition of Stash native entities; `sem:references` present) with a test in `crates/semantic-vocabulary/tests/defined_terms.rs`
- [ ] T049 [P] (pending: needs an independent reader) Run the SC-004 spot check from `specs/001-semantic-vocabulary/quickstart.md` with a reader who has not read the documents; record results in `specs/001-semantic-vocabulary/checklists/requirements.md` notes
- [X] T050 Run all steps of `specs/001-semantic-vocabulary/quickstart.md` and fix any drift between it and the implementation
- [X] T051 [P] Update `ROADMAP.md`: tick the Phase 0 vocabulary item and link `specs/001-semantic-vocabulary/`
- [X] T052 Open a follow-up note for the API document item in `ROADMAP.md` Phase 0 stating that it references `vocabulary/` terms by IRI

---

## Dependencies and execution order

- Phase 1 → Phase 2 → user stories. Phase 2 blocks everything.
- US1 and US2 are both P1; US2's check rules mostly extend work done in US1, so run US1 first.
- US3 depends on US1 (terms exist to annotate and to deprecate). US4 depends on US1 (terms
  exist to produce). US5 depends on US1 (version declared).
- Polish depends on the stories it touches.

### Parallel opportunities

- T003 and T004; T005 and T006; T012 to T015; T020 and T021; T025 and T026; T031 and T032;
  T037 and T038; T043 and T044; T048, T049, T051.
- After US1, US3, US4, and US5 can proceed in parallel (different rules and files, except
  shared `check.rs` which needs coordination).

### Implementation strategy

1. Complete Phases 1 and 2.
2. Deliver US1 as the MVP: all terms in Turtle, `TERMS.md` generated.
3. Add US2 (hardened check), then US3, US4, US5 in priority order.
4. Polish; commit after each phase.
