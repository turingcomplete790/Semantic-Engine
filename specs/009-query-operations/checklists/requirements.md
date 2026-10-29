# Specification Quality Checklist: Query Operations

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-10-27
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- **Implementation details:** the spec names only what the constitution, the documents and
  earlier specs fix:
  - the query language (the documents' SPARQL-inspired `SELECT`);
  - Stash's plugin operations, tasks and hooks;
  - the store's published generations (spec 006);
  - the decision record's rules and the contract files.

  It names no code structure and no library.
- **Scope:** the whole of Phase 3, as seven roadmap items in six stories. Client adoption is
  Phase 4; boolean logic and further constructs are Phase 5.
- **Defaults chosen without asking:**
  - **The language subset:** Milestone 2's "basic filters" are read as single comparisons and
    inequality; everything else is refused, to keep the contract stable.
  - **The short form and the graph form are both accepted**, with equal answers. The documents
    require the short form; the graph form costs little and makes scoping checkable (SC-002).
  - **Tags by name:** constants may name a tag by its Stash name, as the documents' examples do.
  - **Freshness:** hooks start an update job (004 R6). Target: 30 s from save to answer.
  - **One hook per save:** the 004 relations hook is extended, so the 7 ms budget covers both.
  - **Contract 1.4.0**, additive.
  - **The query time limit** is left to the plan.
- **Check on the scope rule (FR-012):** a short-form fact whose performer is linked to two scene
  variables is refused rather than guessed. The documents give no answer for that case.
