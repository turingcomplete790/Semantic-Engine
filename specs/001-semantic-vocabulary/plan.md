# Implementation Plan: Semantic Vocabulary

**Branch**: `001-semantic-vocabulary` | **Date**: 2026-10-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-semantic-vocabulary/spec.md`

## Summary

Produce the semantic vocabulary as a small set of Turtle (`.ttl`) documents at RDFS level
(constitution Principle II, spec FR-001, FR-013), with provenance, scope, and status on every
term, a Stash-data mapping, one deterministic IRI scheme (FR-014), and a human-readable term
list. A small Rust crate embeds the files, parses them, and provides an automated check
(FR-010) so the same files can later be linked by the engine and the viewer. The design is
derived from the plugin's design documents; the plugin's own `src/semantic/vocabulary.js` is
used only as a cross-check, not as the source.

## Technical Context

**Language/Version**: Rust (stable, edition 2021); Turtle 1.1 for the vocabulary documents

**Primary Dependencies**: `oxttl` and `oxrdf` (Oxigraph's Turtle parser and RDF model, so the
check uses the same parser the engine will); no SPARQL engine needed for this feature

**Storage**: Files in the repository (`vocabulary/`); no database

**Testing**: `cargo test` (parse, defined-terms, domain/range, provenance, scope, status,
deprecation, IRI-scheme golden tests); one fixture-backed mapping test per tag flag

**Target Platform**: Any platform Rust builds on; the Turtle files are platform independent

**Project Type**: Library crate plus a command-line check, inside the repository's Cargo
workspace (Phase 0 of the roadmap; this feature creates the workspace root if it does not
exist yet)

**Performance Goals**: Check completes in under 2 seconds for the full vocabulary (a few
hundred triples); loading the vocabulary at engine start is negligible

**Constraints**: RDFS only, no OWL axioms (FR-013); no new Stash fields (FR-012); the
vocabulary must not redefine Stash entities (FR-009); no network access

**Scale/Scope**: About 25 classes and 40 properties, 4 Turtle files, 1 mapping document

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Stash is the system of record; engine is a cache | Pass | The vocabulary describes Stash data; it stores none and adds no Stash field (FR-012) |
| II. Design documents are the contract | Pass, with deliberate divergence | Each term cites document section and revision. One deliberate divergence is documented in research.md R3 (reified Pluggable and Mise-en-scène assignments, per clarification) |
| III. One engine, one set of answers | Pass | The vocabulary is the shared definition the single engine will consume |
| IV. Reached only through Stash | N/A | No runtime component in this feature |
| V. Rust, embedded, headless | Pass | Check and loader are Rust, no UI or network dependencies; parser is Oxigraph's |
| VI. Versioned, explicit contracts | Pass | Vocabulary carries a version and change log (FR-008); the check contract is versioned (contracts/) |
| VII. Fast, bounded, test-first | Pass | Check is repeatable and fixture-backed; tests written before the Turtle files |

No violations; Complexity Tracking not required.

**Re-check after Phase 1 design**: Pass. The design adds no runtime component, no network
access, and no Stash writes; RDFS-only structure holds (research.md R4).

## Project Structure

### Documentation (this feature)

```text
specs/001-semantic-vocabulary/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── vocabulary-files.md   # File layout, annotation properties, naming rules
│   └── vocabulary-check.md   # CLI and library contract of the check
├── checklists/requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
Cargo.toml                       # workspace root (created here if absent)
vocabulary/
├── core.ttl                     # media entities, performers, tags, assignments, variations,
│                                #   archetypes, pairings
├── labels.ttl                   # Label formula and rule classes
├── timeline.ttl                 # Timeline, slots, Stages, stage picks
├── stash-mapping.ttl            # Stash fields and flags → terms
├── CHANGELOG.md                 # versions and migration notes (FR-008)
└── TERMS.md                     # generated human-readable term list (FR-011)
crates/
└── semantic-vocabulary/
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs               # embeds files, exposes terms, IRI scheme
    │   ├── iri.rs               # item IRI scheme (FR-014)
    │   ├── check.rs             # validation rules (FR-010)
    │   ├── terms_md.rs          # TERMS.md generator (FR-011)
    │   └── bin/vocab-check.rs   # command-line entry point
    └── tests/
        ├── parse.rs
        ├── defined_terms.rs
        ├── domain_range.rs
        ├── provenance.rs
        ├── iri_scheme.rs
        └── mapping.rs
```

**Structure Decision**: One library crate with a thin binary. Vocabulary data lives outside the
crate in `vocabulary/` so non-Rust consumers (the viewer's tooling, a query editor, the plugin's
documentation) can read the same files; the crate embeds them with `include_str!`.

## Complexity Tracking

No constitution violations to justify.
