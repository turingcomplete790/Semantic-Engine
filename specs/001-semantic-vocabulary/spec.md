# Feature Specification: Semantic Vocabulary

**Feature Branch**: `001-semantic-vocabulary`

**Created**: 2026-10-14

**Status**: Draft

**Input**: User description: "vocabulary for semantic plugin."

## Overview

The semantic-tagging plugin's design documents describe a semantic model (media entities,
performers, six flagged tag types, Variations, reified Role and Action instances, Label
formulas, Archetypes) in prose, diagrams, and scattered "vocabulary corrections". Three
projects (the shared engine, the Semantic Stash Viewer, and the plugin) need to agree on that
model exactly. This feature turns it into a formal vocabulary: a set of RDF documents in Turtle
format, plus a short mapping that says how each term relates to the Stash data it comes from.

Sources read: `README.md`, `semantic-query-engine-roadmap.md` (including "Core Design Decisions
§2" and its "Vocabulary corrections", and Milestones 1B, 1C, 1D), and `archetype-system-plan.md`
in `~/Projects/Stash-Plugins/semantic-tagging/`, at plugin commit `3c82c85` (2026-09-10) plus its
uncommitted working copy as of 2026-10-14.

## Clarifications

### Session 2026-10-14

- Q: How should the vocabulary represent "this fact only holds within one scene or image"? → A: Reified
  assignments for every scoped fact: a per-scene assignment node ties media entity, performer,
  and tag together, as Role and Action instances already do, extended to Pluggables and
  Mise-en-scène.
- Q: How much formal logic should the vocabulary contain beyond naming classes and properties? →
  A: RDFS level: subclass and subproperty, domain and range, and deprecation links. Data-validation
  shapes (SHACL) are a later, separate feature; full OWL axioms are not used.
- Q: Should the vocabulary also define how each Stash item is given its identifying IRI in the
  graph? → A: Yes: one deterministic scheme (entity type plus Stash ID, under a per-server base
  IRI), defined as part of the vocabulary.
- Q: Should the vocabulary describe the Timeline in full or only the "at time" position on
  instances? → A: In full, as far as the documents describe it: timeline entries (slots) and
  Stages, their order, and their relationship to instances and to Mise-en-scène.
- Q: Should the Stash-data mapping also cover legacy data the plugin has retired (such as
  `isGeneric` performers and Looks stored as JSON)? → A: No: the mapping covers current data only;
  legacy fields are not mapped, and libraries not yet migrated lose those facts in the graph
  until they are.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One authoritative list of semantic terms (Priority: P1)

A developer building the engine, the viewer, or the plugin needs to know exactly which classes
and relationships exist (for example, that a Role assignment is scene-scoped and a Trait is
performer-global) without re-reading three long design documents.

**Why this priority**: Every later piece of the engine (graph build, queries, rules) depends on
agreeing on the terms. Without it each client keeps its own interpretation.

**Independent Test**: Give the vocabulary to a developer who has not read the design documents
and ask them to list the tag types, which are scene-scoped, and which are performer-global.
Their answers match the documents.

**Acceptance Scenarios**:

1. **Given** the vocabulary documents, **When** a reader looks up "Role", **Then** they find its
   definition, that it is a kind of tag, that assignments are scene-scoped, and that it may have
   a category.
2. **Given** the vocabulary documents, **When** a reader looks up a relationship such as
   "has trait", **Then** they find what it connects, its scope (global or scene-scoped), and
   its source section in the design documents.

---

### User Story 2 - Machine-readable and valid (Priority: P1)

A tool (the engine, a validator, a query editor) needs to load the vocabulary and check data
against it without custom parsing.

**Why this priority**: A vocabulary nobody can load is documentation, not a contract.

**Independent Test**: Load every vocabulary document with a standard RDF parser and a standard
Turtle validator; all parse without errors, every referenced term is defined, and the combined
graph loads in the engine's chosen store.

**Acceptance Scenarios**:

1. **Given** the vocabulary files, **When** they are parsed by a standards-compliant Turtle
   parser, **Then** there are no syntax errors or warnings.
2. **Given** the combined vocabulary, **When** a consistency check runs, **Then** every property
   has a declared domain and range and no class or property is referenced without a definition.

---

### User Story 3 - Traceable to the design documents (Priority: P2)

A maintainer needs to see why a term exists and re-check it when the plugin's design documents
change (they are living documents).

**Why this priority**: The constitution makes the documents the contract; terms without
provenance cannot be re-verified.

**Independent Test**: Pick ten random terms; each carries a pointer to the document and section
it came from and the plugin commit or date it was read at.

**Acceptance Scenarios**:

1. **Given** any class or property, **When** a maintainer inspects it, **Then** it names its
   source document section and the plugin revision it was read at.
2. **Given** a term the documents mark as superseded (for example `Generic` replaced by
   `Archetype`, or `hasLook` renamed `hasPerformerVariation`), **When** a maintainer inspects it,
   **Then** it is marked deprecated and points at its replacement.

---

### User Story 4 - Stash data mapping (Priority: P2)

A developer building the graph needs to know which Stash fields and `custom_fields` produce each
term (for example the `isRole` flag marks a Role; `variantOfTagId` marks a common variant).

**Why this priority**: The vocabulary is only useful to the engine if it says how real Stash data
becomes facts.

**Independent Test**: For each tag-type flag in the documents, the mapping gives the Stash field
it is read from, its allowed values, and the term it produces.

**Acceptance Scenarios**:

1. **Given** the mapping, **When** a developer looks up `isTrait`, **Then** they find that a tag
   with that flag set to 1 is a Trait and that Traits are performer-global.
2. **Given** a Label tag's formula field, **When** the mapping is consulted, **Then** it shows how
   the formula's trait, field, and label-dependency rules, weights, and threshold become rule
   nodes.

---

### User Story 5 - Stable and versioned (Priority: P3)

Client projects need to know when the vocabulary changes so they can adapt.

**Why this priority**: Three independently released projects will drift without explicit versions.

**Independent Test**: Make a breaking change (rename a property); the version identifier changes,
and the change appears in a change log with migration notes.

**Acceptance Scenarios**:

1. **Given** the vocabulary documents, **When** a reader checks them, **Then** each carries a
   version identifier and a short change log.

### Edge Cases

- A term the documents mention only informally (for example a future `hasTag` for derived tags)
  is recorded as planned, not defined as active, so clients do not build on it by accident.
- Where the documents disagree with each other, the vocabulary follows the document the plugin
  declares authoritative (the roadmap's corrections win over earlier sketches), and the
  disagreement is listed in the change log.
- Terms the documents describe as "not yet built" or "UI-only" are marked with their status
  rather than omitted or presented as active; Timeline details the documents leave open are
  marked planned (FR-015).
- Names that mean different things at different scopes (an action's chosen variation versus a
  tag's own declared variations) have distinct terms, not one overloaded name.
- A library whose data predates the Variation and Archetype reforms produces no facts from the
  retired fields; the plugin's migration, not the vocabulary, brings that data forward (FR-016).
- Stash's own native fields (gender, ethnicity, and the like) used by Label formulas are
  referenced, not redefined as new terms.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The vocabulary MUST be provided as one or more RDF documents in Turtle format,
  parseable without errors by a standards-compliant parser.
- **FR-002**: The vocabulary MUST define a class for each semantic entity the documents describe:
  media entities (Scene, Image), Performer, Tag and its six flagged types (Pluggable, Action,
  Role, Trait, Label, Mise-en-scène), Variation, the assignment instances (RoleInstance,
  ActionInstance, and the equivalents for Pluggable and Mise-en-scène), Archetype, and the
  Label rule kinds (trait rule, field rule, label-dependency rule, and the Pluggable rule that
  tests a Variation's Pluggables), and Pairing (a performer-to-performer link).
- **FR-003**: The vocabulary MUST define each relationship and attribute the documents describe,
  each with a declared domain and range, including: performer assignment to media, role and
  action assignment, pluggable assignment, trait and label assignment, performer variations,
  tag categories, tag variations, parent tag, archetype parent, label threshold, rule weight,
  rule target, rule variation, rule recursion, field-rule field, modifier, and values, a
  Variation's own Pluggables and Labels and its owning performer, common-variant links
  between tags, and a pairing's endpoints and direction.
- **FR-004**: Scene- and image-scoped facts (Role, Action, Pluggable, and Mise-en-scène
  assignments) MUST be represented as reified assignment nodes that tie one media entity, one
  performer (where applicable), and one tag together, so the scope is part of the data and a
  consumer cannot join facts across different media entities by accident. Performer-global facts
  (Traits, Labels, performer Variations) MUST remain direct edges on the performer. Each
  predicate and class MUST also state its scope in a machine-readable way.
- **FR-005**: Each term MUST carry a human-readable definition and a pointer to the design
  document section and plugin revision (commit or date) it was read at.
- **FR-006**: Terms the documents supersede or retire MUST be marked deprecated and name their
  replacement; terms the documents only plan MUST be marked as planned.
- **FR-007**: The vocabulary MUST include a mapping, in the same format, from Stash data
  (tag flags such as `plugtag`, `isAction`, `isRole`, `isTrait`, `isLabel`, `isMiseEnScene`,
  `isVariation`; `variantOfTagId`; Label formula fields; performer flags) to the terms they
  produce, including allowed values.
- **FR-008**: The vocabulary MUST carry a version identifier and a change log; a breaking
  change MUST change the major version.
- **FR-009**: The vocabulary MUST NOT redefine Stash's own entities or native fields; it MUST
  reference them.
- **FR-010**: The vocabulary MUST be validated by an automated check (syntax, defined terms,
  domain and range present, no orphan references, provenance present) that can be run
  repeatedly.
- **FR-011**: A human-readable summary (term list with definitions and scopes) MUST accompany the
  Turtle documents so the vocabulary can be reviewed without an RDF tool.
- **FR-012**: The vocabulary MUST be usable by the viewer and the plugin without changes to
  their stored Stash data: it describes the existing data contract and adds no new Stash fields.
- **FR-013**: The vocabulary MUST use RDFS-level structure only (class and property
  declarations, subclass and subproperty, domain and range, labels, comments, deprecation
  links). It MUST NOT include OWL axioms such as disjointness or cardinality, so no consumer
  needs a reasoner to use it.
- **FR-014**: The vocabulary MUST define one deterministic IRI scheme for Stash items
  (scene, image, performer, tag, and the reified assignment nodes) built from the entity type
  and its Stash ID under a per-server base IRI, so any client building the graph from the same
  library produces identical identifiers. Assignment nodes MUST derive their IRI from the
  identifiers they connect, so rebuilding the graph never changes them.
- **FR-015**: The vocabulary MUST describe the Timeline as the plugin implements it
  (`README.md` "Timeline"; `semantic-query-engine-roadmap.md` Milestones 1B and 1D; the
  plugin's `src/ui/timeline.js` as read at the recorded revision). Specifically:
  - A media entity has at most one Timeline: an ordered sequence of entries, stored by the
    plugin as a single ordered list on the Scene or Image. The vocabulary MUST record entry
    order explicitly so a graph consumer can reconstruct it.
  - Each entry has a stable identifier of its own, used to build its IRI (FR-014).
  - A **slot** is an ordinary entry. It refers to one thing it places on the Timeline: a Role
    instance, an Action instance, or a pairing of two performers' Role or Action instances. It
    MAY carry a time position in seconds (Images are sequenced without time), and MAY be linked
    to the Stash marker generated from it.
  - A **Stage** is an entry of a different kind: a heading, not a moment. It carries
    Mise-en-scène picks, each a tag with an optional declared variation, and applies to every
    slot after it until the next Stage or the end of the Timeline. A slot's Stage MUST be
    derivable from order alone (no separate stage field on slots), as the documents specify.
  - A **pairing** (a performer-to-performer Role or Action link with its own stable
    identifier) MUST be a defined term, since slots refer to it.
  - Entries saved before Stages existed carry no kind; they are slots. The vocabulary MUST state
    this default.
  - Where the documents leave a Timeline detail undefined, the vocabulary MUST mark it as
    planned rather than inventing it.
- **FR-016**: The Stash-data mapping (FR-007) MUST cover current data only. Retired fields
  (for example `isGeneric` performers and Looks stored as JSON) MUST NOT be mapped to any term.
  Retired terms stay in the vocabulary only as deprecated entries naming their replacement
  (FR-006), so readers of older documents can find them, and the change log MUST state which
  retired fields are no longer read.

### Key Entities

- **Media entity**: a Scene or an Image; the thing semantic facts are about.
- **Performer**: a participant in a media entity; may be an Archetype.
- **Tag types**: Pluggable, Action, Role, Trait, Label, Mise-en-scène; each a kind of Stash tag
  with its own scope, optional categories, and optional variations.
- **Variation**: a real tag (`isVariation`) carrying its own facts, owned by a performer,
  including the Pluggables and Labels it carries.
- **Assignment instances** (Role, Action, Pluggable, Mise-en-scène): one performer's (or, for
  Mise-en-scène, one media entity's) assignment of one tag within one media entity, optionally
  with a chosen variation and a time position.
- **Label formula and rules**: a Label's threshold and weighted rules (trait, field, label
  dependency, and Pluggable-on-Variation) that describe why a performer qualifies.
- **Timeline, slot, and Stage**: a media entity's ordered sequence of entries; a slot places an
  Action or Role instance (or a pairing) at an optional time and may link to a Stash marker; a
  Stage is a heading carrying Mise-en-scène picks that governs the slots after it.
- **Pairing**: a performer-to-performer Role or Action link with a stable identifier, which slots
  can refer to.
- **Stash marker**: an existing Stash entity a slot may link to once generated; referenced, not
  redefined (FR-009).
- **Archetype**: a performer-kind with parents-only adjacency, inheriting configured attributes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the classes and relationships listed in the design documents'
  "Core Design Decisions §2" and "Vocabulary corrections", Milestone 1B, 1C, and 1D appear in
  the vocabulary or are explicitly listed as excluded with a reason.
- **SC-002**: The vocabulary documents parse with zero errors and zero warnings in a standard
  Turtle parser, and the automated check passes on every revision.
- **SC-003**: 100% of terms carry a definition, scope, and source reference.
- **SC-004**: A developer who has not read the design documents can correctly answer, from the
  vocabulary alone, ten scoping questions (for example "is a Role global or scene-scoped?") with
  at least 90% accuracy.
- **SC-005**: The viewer's maintainer and the plugin's maintainer each confirm that every flag
  and `custom_fields` key they rely on has a mapping entry, with zero unmapped keys remaining.
- **SC-006**: Building the graph twice from the same library, by two independent clients, gives
  identical identifiers for 100% of scenes, images, performers, tags, and assignments.

## Assumptions

- The design documents (not the plugin's JavaScript) are authoritative, per the constitution
  (Principle II); where they conflict, the roadmap's "Vocabulary corrections" win.
- The plugin's documents were uncommitted-modified at read time; provenance records the commit
  plus the read date, and the vocabulary is re-checked when they are committed.
- The vocabulary's own namespace IRI is a stable, project-owned identifier chosen at planning
  time; it does not need to resolve on the web in this version. Item identifiers (FR-014) use a
  separate per-server base IRI supplied by the client when it builds the graph.
- Existing standard vocabularies (for example RDFS and OWL) MAY be reused for structure, but no
  external domain vocabulary is required.
- Data-validation shapes (for example SHACL) are a separate, later feature, taken up once the
  engine exists and there is real data to validate.
- Query-language features, rules, and the engine's operation protocol are out of scope; they
  consume this vocabulary but are separate features.
- The API document for clients (roadmap Phase 0) is a separate deliverable that will reference
  this vocabulary.
