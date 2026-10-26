# Semantic vocabulary change log

Release procedure: bump `sem:version` in `core.ttl`, add the entry here, run
`vocab-check --write-terms --write-snapshot`, then `vocab-check`. A removed or renamed term, or an
incompatible domain or range, needs a higher MAJOR; added terms need a higher MINOR (rule V014).

## 1.4.0 (2026-10-26)

Corrections found while mapping the library into the graph (`specs/007-graph-build`, research
R6). Terms are only added and comments clarified; no domain or range changes.

### Additions

- **`pairsA` and `pairsB`:** the instances at a pairing's ends A and B, both sub-properties of
  `pairs`. A directed Action pairing ("A gives to B") needs to know which instance is end A, and
  RDF facts have no order.
- **`roleAppliedAutomatically`:** true on a role instance that auto-apply added
  (`autoRoleAssignments`). `map_autoRoleAssignments` marks automatic roles, but no term could
  mark a role instance, and `appliedAutomatically` is limited to relation instances.

### Clarifications

- **`pairingDirection`:** the plugin stores `bidirectional`, `give` and `receive`, and treats a
  missing direction as bidirectional (`src/ui/actions.js`). The comment said "bidirectional or
  directed". Role pairings carry no direction.
- **`pairs`:** points to `pairsA` and `pairsB` for the ends.
- **Status Derived:** now "defined as a rule over other data, not by a Stash key of its own".
  The four measurement properties are stored by the builder, because their input is not a fact
  and no query could compute them. `inStage` stays not stored.
- **Mapping comments:** `map_actionPairings`, `map_roleTagPairings` and
  `map_autoRoleAssignments` name the terms they also produce.

## 1.3.0 (2026-10-24)

Terms for what the library reader reads (`specs/005-stash-adapter`), from the semantic-tagging
plugin at `2c04141`.

### Additions

- **Field Mapping:** `FieldMappingRule`, with `hasFieldMapping` (Tag to rule), `mapsToField`,
  `mappingPriority` (lowest wins) and `mappingVariation`. They describe the tag key
  `fieldMapping` (`{base?: {field, priority}, variations?: {"<variation>": {field, priority}}}`)
  from `src/ui/field-mapping.js`. Like Label rules, the rule is described in the graph; the field
  it sets stays the plugin's write.
- **Mapping:** `map_fieldMapping`.
- **Item IRIs:** `{tag}/fieldMapping:base`, and `{tag}/fieldMapping:variation:{n}` where `n` is
  the variation's index in the sorted variation names.

### Deliberate additions (constitution Principle II)

The plugin's code (`src/ui/matrix.js`, `parseMeasurements` and `DERIVED_PERFORMER_FIELDS`)
derives four values from a performer's `measurements` text that Label field rules and the
Matrix's filters can name. No design document describes them, so they are added deliberately and
noted for the plugin (`specs/005-stash-adapter/plugin-notes.md`):

- `bustSize` (`__derived_bust`), `waist` (`__derived_waist`), `hip` (`__derived_hip`) and
  `waistToHipRatio` (`__derived_whr`), all with status Derived.
- **Formula, as at `2c04141`:** the text must start with a band number. The cup letters (DD, DDD
  and DDDD read as E, F and G) and an optional half cup (`½`, `.5` or `1/2`) give a step on the
  scale AA 0, A 1, A½ 2, B 3, B½ 4, C 5, C½ 6, D 7, D½ 8, E 9, E½ 10, F 11, G 12, H 13 to L 17.
  Bust Size is band + step × 2.5 + 2 per inch of waist under 28. Waist and hip are the second
  and third numbers. The ratio is waist ÷ hip rounded to 2 places, when neither is 0.
- `pluginFieldName`, an annotation linking each property to the plugin's field name, so a
  `FieldRule` naming `__derived_bust` resolves to `bustSize`.
- **Mapping:** `map_measurements`, from the native performer field `measurements`.

### Display data, passed through

- `archetypeGraphPosition` (an Archetype's saved Hierarchy-tab position, `{x, y}`) is read and
  passed to clients next to the Archetype hierarchy. It is not a graph term (spec 005 FR-002a).
  It was listed under "Excluded on purpose" before.

## 1.2.0 (2026-10-21)

Relations, role and relation hierarchies, and auto-apply rules
(`specs/004-relations-role-hierarchy`).

### Deliberate additions (constitution Principle II)

The semantic-tagging design documents at `3c82c85` have no Relations, role hierarchy, or
auto-apply. The owner requested them on 2026-10-21. The additions are additive: no existing term
or stored key changes meaning or shape, so clients that do not know them keep working.

- **Classes:** `Relation` (a tag type, flag `isRelation`), `RelationInstance` (one relation
  between two performers in one scene or image), and `RelationRule` (an auto-apply rule).
- **Hierarchy:** `broaderRole` and `broaderRelation`, both sub-properties of `parentTag`. They
  come from a Stash parent that has the same type flag; other parents keep their plain
  `parentTag` meaning.
- **Relation ends and rules:** `endARole`, `endBRole`, `hasRelationRule`, `relationRuleId`,
  `endALabel`, `endARoleCondition`, `endBLabel`, `endBRoleCondition`, `linkingAction`. Only an
  action pairing links two performers; holding the same Action does not.
- **Instances:** `ofRelation`, `atEndA`, `atEndB`, `appliedAutomatically`, `appliedByRule`.
- **Mappings:** `isRelation`, `relationEnds`, `relationRules`, `relationAssignments`,
  `autoRoleAssignments`. Automatic end roles also appear in the existing `roleTagAssignments`,
  so every client sees them; `autoRoleAssignments` only records which ones auto-apply added.
- **Item IRIs:** `{media}/relation:{relationTagId}:{a}:{b}` and `{relationTag}/rule:{ruleId}`.

## 1.1.0 (2026-10-15)

- Added `ruleId` (Label rule to string): the plugin's stable identifier of a rule within its
  Label's formula. Label rule IRIs are now `{label}/rule:{ruleId}` instead of position-based, so
  reordering a formula no longer changes them. Found when the plugin's real data showed that
  every rule carries its own `id`.
- Mapping entries for the Label formula rule lists now name the `id` field.
- No client had consumed 1.0.0 when the rule IRI pattern changed.

## 1.0.0 (2026-10-14)

Initial release, derived from the semantic-tagging design documents (`README.md`,
`semantic-query-engine-roadmap.md`, `archetype-system-plan.md`) at plugin commit `3c82c85` plus
its working copy as of 2026-10-14.

### Deliberate divergences from the documents (constitution Principle II)

- Pluggable and Mise-en-scène assignments are reified (`PluggableInstance`,
  `MiseEnSceneInstance`) so scope is part of the data. The documents and the plugin's
  `vocabulary.js` keep `hasPluggable` and `hasMiseEnScene` as direct edges with scope annotation
  only. The stored Stash data is unchanged; the mapping reads the same fields.
- `hasRole`, `hasAction`, and `hasPluggable` remain as sub-properties of `hasAssignment` so the
  documents' query style still reads naturally.
- `inMedia` is declared on the abstract `MediaScoped` so assignments, pairings, and Timelines
  share one scope property without union domains (RDFS only).

### Document versus code notes

- The documents sometimes call the role pairing key `rolePairings`; the stored key in the
  plugin source is `custom_fields.roleTagPairings`. The mapping uses the stored key.
- The Variation reverse index lives in `custom_fields.variationData`; the documents describe it
  as "performer id, pluggable tag ids, and label tag ids". The plugin's stored object also holds
  `traitTagIds`; the documents expose no Trait fact for a Variation, so the vocabulary has no
  term for it and it is not mapped (to be settled by the plugin's documents).

### Deprecated terms

| Term | Replaced by |
|---|---|
| `Generic` | `Archetype` |
| `Look` | `Variation` |
| `hasLook` | `hasPerformerVariation` |
| `hasArchetype` | none (removed in Milestone 1D) |

### Retired fields no longer read (FR-016)

`custom_fields.isGeneric`, `custom_fields.looks`, and legacy per-slot Mise-en-scène refs in
`custom_fields.timeline`. Libraries not yet migrated lose those facts in the graph.

### Planned

- `hasTag` (derived tags from CONSTRUCT, Milestone 5).
- `serverIdentifier` and its mapping: the UUID setting generated by the engine plugin.

### Excluded on purpose (SC-001)

| Item | Reason |
|---|---|
| Semantic Search Matrix, Quick Pairings, Graph Query Preview | UI behaviour, not graph vocabulary |
| `hasGeneric` | never a predicate; a Generic is a Performer subclass (Deprecated) |
| `archetypeRecipe` | plugin application data, not graph vocabulary (`archetypeGraphPosition` is passed through as display data since 1.3.0) |
| `directTraitVariations`, `inheritedVariations`, `inheritedTraitVariations` | caches computed by the plugin |
