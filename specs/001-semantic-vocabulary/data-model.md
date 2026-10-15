# Data Model: Semantic Vocabulary

Namespace `sem:` = `urn:semantic-stash:vocab:`. Every term below carries `rdfs:label`,
`rdfs:comment`, `sem:source`, `sem:sourceRevision`, `sem:scope`, `sem:status`, and `sem:since`
(see [contracts/vocabulary-files.md](contracts/vocabulary-files.md)). "Global" and
"SceneScoped" are values of `sem:scope`.

## Classes

### core.ttl

| Class | Subclass of | Scope | Notes |
|---|---|---|---|
| `MediaEntity` | – | – | Abstract base |
| `Scene`, `Image` | `MediaEntity` | – | References Stash scene and image (FR-009) |
| `Performer` | – | – | References Stash performer |
| `Tag` | – | – | Abstract base of the six flagged types and Variation |
| `Pluggable`, `Action`, `Role`, `Trait`, `Label`, `MiseEnScene` | `Tag` | – | Flag per [stash-mapping.ttl](#stash-mapping) |
| `Variation` | `Tag` | Global | A real tag (`isVariation`) owned by one performer |
| `Archetype` | `Performer` | Global | Performer flagged `isArchetype`; parents-only adjacency |
| `MediaScoped` | – | – | Abstract; anything that belongs to exactly one media entity (`inMedia`): assignments, pairings, Timelines |
| `Marker` | – | – | References a Stash scene marker (`sem:references`) |
| `TagReference` | – | – | Abstract; anything that points at one tag (`ofTag`): assignments, label rules, stage picks |
| `Placeable` | – | – | Abstract; anything a Timeline slot can place: assignments and pairings |
| `TagAssignment` | `TagReference`, `Placeable` | SceneScoped | Abstract base of the reified assignments |
| `RoleInstance`, `ActionInstance`, `PluggableInstance`, `MiseEnSceneInstance` | `TagAssignment` | SceneScoped | One assignment of one tag within one media entity |
| `Pairing` | `Placeable` | SceneScoped | Performer-to-performer link with its own identifier |
| `RolePairing`, `ActionPairing` | `Pairing` | SceneScoped | Pairing of Role or Action instances |
| `Generic` | `Performer` | Global | **Deprecated**, replaced by `Archetype` |
| `Look` | – | Global | **Deprecated**, replaced by `Variation` |

### labels.ttl

| Class | Subclass of | Notes |
|---|---|---|
| `LabelRule` | `TagReference` | Abstract; every rule carries a weight |
| `TraitRule` | `LabelRule` | Satisfied by a performer carrying a Trait (optionally one variation, optionally recursive) |
| `FieldRule` | `LabelRule` | Satisfied by a native Stash performer field; references the field, does not redefine it |
| `LabelDependencyRule` | `LabelRule` | Satisfied by already carrying another Label |
| `PluggableRule` | `LabelRule` | Satisfied by a Variation carrying a Pluggable (Milestone 1D) |

### timeline.ttl

| Class | Subclass of | Notes |
|---|---|---|
| `Timeline` | – | One per media entity at most |
| `TimelineEntry` | – | Abstract; ordered |
| `Slot` | `TimelineEntry` | Ordinary entry; default for entries without a kind |
| `Stage` | `TimelineEntry` | Heading; governs following slots |
| `StagePick` | `TagReference` | A Mise-en-scène tag with optional declared variation |

## Properties

### Participation and assignment (core.ttl)

| Property | Domain → Range | Scope | Notes |
|---|---|---|---|
| `hasPerformer` | `MediaEntity` → `Performer` | SceneScoped | Participation; union of every performer referenced by an assignment or pairing |
| `hasAssignment` | `Performer` → `TagAssignment` | SceneScoped | Super-property of the three below |
| `hasRole` | `Performer` → `RoleInstance` | SceneScoped | sub-property of `hasAssignment` |
| `hasAction` | `Performer` → `ActionInstance` | SceneScoped | sub-property of `hasAssignment` |
| `hasPluggable` | `Performer` → `PluggableInstance` | SceneScoped | sub-property of `hasAssignment` |
| `hasMiseEnScene` | `MediaEntity` → `MiseEnSceneInstance` | SceneScoped | No performer involved; one per distinct tag per media entity, produced from Stage picks (FR-015) |
| `fromPick` | `MiseEnSceneInstance` → `StagePick` | SceneScoped | The Stage pick(s) this media-level assignment came from |
| `inMedia` | `MediaScoped` → `MediaEntity` | SceneScoped | Carries the scope into the data (FR-004); `TagAssignment`, `Pairing`, and `Timeline` are `MediaScoped` |
| `forPerformer` | `TagAssignment` → `Performer` | SceneScoped | Absent on `MiseEnSceneInstance` |
| `ofTag` | `TagReference` → `Tag` | Global | Which tag this node is about: assignments, rules, and stage picks |
| `hasVariation` | `TagReference` → string | Global | The variation chosen for this occurrence (per-occurrence selection); may repeat, notably on `MiseEnSceneInstance` when a tag is picked with different variations in different Stages |
| `atTime` | `TagAssignment` → decimal seconds | SceneScoped | Derived from the enclosing slot's `slotSeconds`; absent if never placed |
| `pairs` | `Pairing` → `TagAssignment` | SceneScoped | Two endpoints per pairing |
| `pairingDirection` | `Pairing` → string | Global | `bidirectional` or `directed` |
| `pairingId` | `Pairing` → string | Global | The plugin's stable identifier |

### Performer-global facts (core.ttl)

| Property | Domain → Range | Notes |
|---|---|---|
| `hasTrait` | `Performer` → `Trait` | From performer tags |
| `hasLabel` | `Performer` → `Label` | From performer tags |
| `hasPerformerVariation` | `Performer` → `Variation` | Renamed from `hasLook` |
| `hasVariationPluggable` | `Variation` → `Pluggable` | From the Variation tag's own reverse index |
| `hasVariationLabel` | `Variation` → `Label` | Same |
| `variationOf` | `Variation` → `Performer` | Owner; from the reverse index |
| `parentArchetype` | `Archetype` → `Archetype` | Parents only; children are derived |

### Tag metadata (core.ttl)

| Property | Domain → Range | Notes |
|---|---|---|
| `hasCategory` | `Role`, `Trait`, `MiseEnScene` tags → string | Not on Action, Pluggable, Label |
| `hasTagVariation` | `Action`, `Trait`, `MiseEnScene` tags → string | Declared alternate name; distinct from `hasVariation` |
| `parentTag` | `Tag` → `Tag` | Stash's native hierarchy, direct parents only |
| `variantOfTag` | `Tag` → `Tag` | Common variant link (`variantOfTagId`) |

Domain restrictions to a subset of `Tag` types are stated in `rdfs:comment` rather than with union
domains (RDFS, FR-013).

### Label formulas (labels.ttl)

| Property | Domain → Range | Notes |
|---|---|---|
| `hasThreshold` | `Label` → decimal | Default 100 when unset (documented, not stored) |
| `hasRule` | `Label` → `LabelRule` | One per formula entry |
| `ruleId` | `LabelRule` → string | The plugin's stable id of a rule within its formula (since 1.1.0) |
| `hasWeight` | `LabelRule` → decimal | |
| `isRecursive` | `TraitRule` → boolean | Descendants count too |
| `hasField`, `hasModifier`, `hasValue`, `hasValue2` | `FieldRule` → string | Values are Stash filter terms; not redefined |

`ofTag` and `hasVariation` apply to `TagReference`, the shared superclass of assignments, label
rules, and stage picks, so no union domain is needed. Which subclasses actually carry
`hasVariation` (Action instances, Trait rules, stage picks) is stated in `rdfs:comment`.

### Timeline (timeline.ttl)

| Property | Domain → Range | Notes |
|---|---|---|
| `hasTimeline` | `MediaEntity` → `Timeline` | At most one |
| `hasEntry` | `Timeline` → `TimelineEntry` | Unordered membership |
| `position` | `TimelineEntry` → non-negative integer | Zero-based order within the timeline |
| `entryId` | `TimelineEntry` → string | Plugin's stable identifier; used in the entry's IRI |
| `places` | `Slot` → `Placeable` | A `TagAssignment` or a `Pairing` |
| `slotSeconds` | `Slot` → decimal | Optional; absent for sequence-only images |
| `linkedMarker` | `Slot` → Stash marker IRI | Reference to a Stash marker (FR-009) |
| `hasPick` | `Stage` → `StagePick` | |
| `inStage` | `Slot` → `Stage` | **Derived**: nearest preceding Stage by `position`; not materialised by this feature |


## Identity (FR-014)

| Node | IRI pattern |
|---|---|
| Scene, Image, Performer, Tag, Marker | `urn:semantic-stash:{server}:{type}:{id}`; `{server}` is the UUID stored in Stash (FR-014) |
| Role, Action, Pluggable instance | `{media}/{kind}:{performerId}:{tagId}` |
| Mise-en-scène instance | `{media}/mise:{tagId}` (one per distinct tag, variations carried over) |
| Pairing | `{media}/pairing:{pairingId}` |
| Timeline | `{media}/timeline` |
| Entry | `{media}/timeline/{entryId}` |
| Stage pick | `{entry}/pick:{tagId}` |
| Label rule | `{label}/rule:{ruleId}` (the rule's own stable id in the formula, `sem:ruleId`; never its position) |

Where the same tag can be assigned twice to one performer in one media entity (the
documents allow one assignment per performer and tag), the IRI is unique by construction.

## Stash mapping (stash-mapping.ttl)

Each row is a `sem:Mapping` with `sem:fromField`, `sem:fromEntity`, `sem:allowedValues`,
`sem:produces`, and `sem:source`.

| Stash data | Entity | Allowed values | Produces |
|---|---|---|---|
| `custom_fields.plugtag` | Tag | `1` | `Pluggable` |
| `custom_fields.isAction` | Tag | `1` | `Action` |
| `custom_fields.isRole` | Tag | `1` | `Role` |
| `custom_fields.isTrait` | Tag | `1` | `Trait` |
| `custom_fields.isLabel` | Tag | `1` | `Label` |
| `custom_fields.isMiseEnScene` | Tag | `1` | `MiseEnScene` |
| `custom_fields.isVariation` | Tag | `1` | `Variation` |
| `custom_fields.variantOfTagId` | Tag | tag ID | `variantOfTag` |
| `custom_fields.categories` | Tag | list | `hasCategory` |
| `custom_fields.variations` | Tag | list | `hasTagVariation` |
| `custom_fields.labelFormula` | Tag (Label) | `{threshold, traitRules, fieldRules, labelRules}` | `hasThreshold`, rules |
| `custom_fields.isArchetype` | Performer | `1` | `Archetype` |
| `custom_fields.archetypeParents` | Performer | list of IDs | `parentArchetype` |
| `custom_fields.roleTagAssignments` | Scene, Image | map performer → entries | `RoleInstance` |
| `custom_fields.actionAssignments` | Scene, Image | map performer → entries | `ActionInstance` |
| `custom_fields.pluggableAssignments` | Scene, Image | map performer → entries | `PluggableInstance` |
| `custom_fields.actionPairings`, rolePairings | Scene, Image | list | `Pairing` |
| `custom_fields.timeline` | Scene, Image | ordered list | `Timeline`, entries, `StagePick`, and the media-level `MiseEnSceneInstance` (one per distinct picked tag) |
| Stash plugin setting `serverId` of this plugin (**planned**: exact setting name fixed by the engine feature) | Stash server | lower-case hyphenated UUID | the `{server}` part of every item IRI (FR-014); not a graph term |
| native performer tags | Performer | flagged tags | `hasTrait`, `hasLabel`, `hasPerformerVariation` |
| native tag parents | Tag | tag IDs | `parentTag` |

Not mapped (FR-016): `isGeneric`, Looks JSON, legacy `mise-en-scene` slot refs. Stage picks are the only source of Mise-en-scène assignments.

The exact field names above were read from the plugin source for the mapping only; where a
document and the code differ, the document wins and the difference is listed in the change log.
