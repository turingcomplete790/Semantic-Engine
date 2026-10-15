# Research: Semantic Vocabulary

All Technical Context items were resolved; no NEEDS CLARIFICATION remains. Sources: the plugin's
`README.md`, `semantic-query-engine-roadmap.md` (Core Design Decisions §2 and its vocabulary
corrections; Milestones 1B, 1C, 1D), `archetype-system-plan.md`, and as a cross-check only the
plugin's `src/semantic/vocabulary.js` and `src/ui/timeline.js`, read at plugin commit `3c82c85`
(2026-09-10) plus its uncommitted working copy on 2026-10-14.

## R1. Namespace and item IRIs

- **Decision**: Vocabulary namespace `urn:semantic-stash:vocab:` (prefix `sem:`). Item IRIs
  `urn:semantic-stash:{server}:{type}:{id}` where `{server}` is a UUID (lower-case, hyphenated)
  generated once by an engine-plugin task and stored in Stash, read by every client, `{type}` is one of `scene`, `image`, `performer`,
  `tag`, `marker`, and `{id}` is Stash's numeric ID. Derived nodes: `.../{type}:{id}/{kind}:{key}`,
  for example `.../scene:12/role:{performerId}:{tagId}` and `.../scene:12/timeline:{entryId}`. Label rule nodes use the rule's own stable id (`{label}/rule:{ruleId}`), not its position.
- **Rationale**: URNs need no owned web domain (spec assumption) and cannot be mistaken for
  something that should resolve. Stash IDs are unique within one server only, which matches the
  constitution's rule that a store is never shared across servers. Deriving derived-node IRIs
  from the identifiers they connect makes rebuilds stable (FR-014, SC-006).
- **Alternatives considered**: Normalised base URL as the server part (fragile: the same
  server is reached by several URLs; rejected in the second clarification pass). An `https://` namespace (needs a domain the project does not own
  and invites dereferencing). Blank nodes for assignments (not stable across rebuilds, cannot be
  compared between clients). Hash-based IDs (rejected during clarification).

## R2. Where the Turtle is split

- **Decision**: Four files: `core.ttl`, `labels.ttl`, `timeline.ttl`, `stash-mapping.ttl`. The
  mapping is separate from the terms so it can change when plugin storage changes without
  changing the vocabulary's meaning.
- **Rationale**: Labels and the Timeline are the most volatile areas (Milestone 1D, Stages);
  isolating them keeps changes local and MINOR-versionable.
- **Alternatives considered**: One file (simple but every plugin change touches everything);
  one file per class (too many files for ~65 terms).

## R3. Scope as reified assignments (clarification Q1)

- **Decision**: Role, Action, Pluggable, and Mise-en-scène assignments are all reified nodes
  with `sem:inMedia`. Direct performer edges (`sem:hasRole`, `sem:hasAction`,
  `sem:hasPluggable`) remain as sub-properties of `sem:hasAssignment` so queries written in the
  documents' style still read naturally. Traits, Labels, and performer Variations stay direct,
  performer-global edges.
- **Divergence from the documents**: The documents (and the plugin's `vocabulary.js`) keep
  `hasPluggable` and `hasMiseEnScene` as direct edges with scope annotation only. The
  clarification chose reification for all scoped facts. This is a deliberate divergence under
  Principle II; it is recorded in the change log and the mapping's notes, and does not change
  the stored Stash data (the mapping reads the same fields).
- **Rationale**: Scope inside the data means no consumer needs special join rules, and
  Pluggables may later carry a Variation context.
- **Alternatives considered**: Annotation only; named graph per media entity (rejected in
  clarification).

## R4. RDFS-level annotation vocabulary (clarification Q2)

- **Decision**: Use `rdf:`, `rdfs:` (`Class`, `Property`, `subClassOf`, `subPropertyOf`,
  `domain`, `range`, `label`, `comment`, `seeAlso`) and `xsd:` datatypes. Project annotations
  in `sem:`: `sem:source` (document and section), `sem:sourceRevision`, `sem:scope`
  (`sem:SceneScoped` or `sem:Global`), `sem:status` (`sem:Active`, `sem:Deprecated`,
  `sem:Planned`, `sem:Derived`), `sem:replacedBy`, `sem:since` (vocabulary version).
- **Rationale**: No OWL means no reasoner is needed (FR-013). Status is an annotation, not
  `owl:deprecated`, to stay strictly RDFS. Union domains in the documents (for example `ofTag`
  on several instance types) are expressed with a shared superclass (`sem:TagAssignment`,
  `sem:LabelRule`) instead of `owl:unionOf`.
- **Alternatives considered**: `owl:deprecated`, `dcterms`, `skos:` (extra vocabularies with no
  consumer need).

## R5. Vocabulary check as a Rust library plus command

- **Decision**: `semantic-vocabulary` crate, parser from `oxttl`, check rules listed in
  `contracts/vocabulary-check.md`, a `vocab-check` binary for CI and local runs.
- **Rationale**: Principle V (Rust, embedded) and the engine will use the same parser; embedding
  the files in the crate gives the engine a versioned vocabulary for free.
- **Alternatives considered**: Python rdflib script (a second toolchain for a Rust project);
  Apache Jena `riot` (JVM dependency, not commonly packaged). Raptor's `rapper` is used as the independent second parser in tests and CI, since it is C, widely packaged, and already installed.

## R6. Items the spec lists that the documents extend

Reading the documents found terms not named in FR-002, all covered by FR-003's "including" list
and the documents' Milestone 1D; they are included in the vocabulary and should be added to the
spec's lists at the next spec revision:

- `PluggableRule` (a Label rule satisfied by a Variation carrying a Pluggable) and the
  Variation reverse-index properties `hasVariationPluggable`, `hasVariationLabel`.
- `variantOfTag` (common variants, Milestone 1C).
- Archetype `parentArchetype` and the Archetype recipe fields (the recipe is mapped for
  provenance but only parentage is graph vocabulary; recipe application is a plugin concern).
- Pairing direction (`bidirectional` or directed) on pairings.

## R7. Items deliberately excluded

- `hasLook` / `Look` (renamed `hasPerformerVariation` / Variation): deprecated terms only.
- `Generic` / `isGeneric`: deprecated, replaced by `Archetype` (FR-016: not mapped).
- `hasArchetype` (removed in Milestone 1D).
- A blanket `hasTag`: marked planned for CONSTRUCT-derived tags (Milestone 5).
- Matrix, Quick Pairings, Semantic Search UI state: UI behaviour, not graph vocabulary.

## R8. Timeline details left open

Resolved from `src/ui/timeline.js`: entry order is array order; entry `id` stable; `seconds`
optional; slot `ref.kind` values `action-instance`, `role-instance`, `action-pairing`,
`role-pairing` (legacy `mise-en-scene` ref is not mapped, FR-016); Stage `miseEnScenePicks`
with `tagId`, optional `variation` and `variationTagId`; `markerId` for a generated Stash marker.
Marked **planned**: slot-to-Stage derivation as a stored edge (`sem:inStage` is `sem:Derived`,
not materialised by this feature), and any timeline relation operators ("same time", "before",
"after") beyond comparing `sem:atTime`.
