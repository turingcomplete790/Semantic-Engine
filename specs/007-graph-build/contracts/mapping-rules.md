# Contract: mapping rules

This contract lists every fact the builder writes, by Stash key. The terms are vocabulary 1.4.0
([vocabulary-1.4.md](vocabulary-1.4.md)), and the IRIs are the item-IRI scheme
(`semantic_vocabulary::iri`). Anything not listed here is not written (FR-009).

## Notation

| Short form | Meaning |
|---|---|
| `M` | the media item: `ItemIris::scene(id)` or `ItemIris::image(id)` |
| `P(x)` | `ItemIris::performer(x)` |
| `T(x)` | `ItemIris::tag(x)` |
| `role(M,p,t)`, `action(…)`, `pluggable(…)` | the instance IRIs `{M}/role:{p}:{t}`, and so on |
| `str`, `dec`, `int`, `nni`, `bool` | literals of `xsd:string`, `xsd:decimal`, `xsd:integer`, `xsd:nonNegativeInteger`, `xsd:boolean` |
| `a` | `rdf:type` |

**Values:**
- A "Stash id" is a string or JSON integer of decimal digits.
- A "plugin id" (pairing, entry, rule) must pass the item-IRI check.
- A value that fails either check gives an `invalidId` note, and the entry is skipped.
- A field the reader marked unreadable gives an `unreadable` note, and the key is skipped.
- Every note reason is in [research R8](../research.md#r8-build-notes-and-the-summary).

## Tags (every `SemanticTag`)

| Source | Facts |
|---|---|
| the tag | `T a C`, for each flag's class `C` (`Pluggable`, `Action`, `Role`, `Trait`, `Label`, `MiseEnScene`, `Variation`, `Relation`); `T a sem:Tag` when it has no flag; `T rdfs:label str(name)` |
| `parents` | `T sem:parentTag T(p)` for each parent. If both are Roles, also `T sem:broaderRole T(p)`. If both are Relations, also `T sem:broaderRelation T(p)` |
| `categories` (list of names) | `T sem:hasCategory str(c)` for each. Note `tagWrongType` unless the tag is a Role, Trait or Mise-en-scène |
| `variations` (list of names) | `T sem:hasTagVariation str(v)` for each. Note `tagWrongType` unless the tag is an Action, Trait or Mise-en-scène |
| `variantOfTagId` | `T sem:variantOfTag T(v)` |
| `labelFormula` | see [Label formulas](#label-formulas) |
| `variationData` | `T sem:variationOf P(performerId)`; `T sem:hasVariationPluggable T(x)` for each of `pluggableTagIds`; `T sem:hasVariationLabel T(x)` for each of `labelTagIds`. `traitTagIds` is not mapped |
| `fieldMapping` | see [Field Mapping](#field-mapping) |
| `relationEnds` `{a, b}` | `T sem:endARole T(a)`; `T sem:endBRole T(b)` |
| `relationRules` | for each rule `R = {T}/rule:{id}`: `T sem:hasRelationRule R`; `R a sem:RelationRule`; `R sem:relationRuleId str(id)`; end A `{label: x}` gives `R sem:endALabel T(x)`, and `{role: x}` gives `R sem:endARoleCondition T(x)`; end B likewise; `R sem:linkingAction T(x)` for each of `actions` |

**Parsing:** `relationEnds` and `relationRules` are read as `engine::relations::defs` reads them.

**Mismatches:** a parent that is not among the reader's tags still gives `parentTag`, with no
broader link and no note. An ordinary Stash tag can be a parent.

### Label formulas

These apply to a tag whose `labelFormula` is present. The tag should be a Label: otherwise the
formula is mapped and noted `tagWrongType`.

| Source | Facts |
|---|---|
| `threshold` | `T sem:hasThreshold dec(n)` when `n` is a number greater than 0, otherwise `dec(100)` |
| each rule (`R = {T}/rule:{id}`) | `T sem:hasRule R`; `R sem:ruleId str(id)`; `R sem:hasWeight dec(weight)`, when `weight` is a number |
| `traitRules` | `R a sem:TraitRule`; `R sem:ofTag T(tagId)`; `R sem:hasVariation str(variationMatch)`, when it is a non-empty string other than `__any__`; `R sem:isRecursive bool(recursive)`, when it is a boolean |
| `fieldRules` | `R a sem:FieldRule`; `R sem:hasField str(field)`; `R sem:hasModifier str(modifier)`; `R sem:hasValue str(value)`; `R sem:hasValue2 str(value2)`, when present. Values that are numbers are written as their text |
| `labelRules` | `R a sem:LabelDependencyRule`; `R sem:ofTag T(labelTagId)` |
| `pluggableRules` | `R a sem:PluggableRule`; `R sem:ofTag T(pluggableTagId)` |

A rule with no `id` is skipped and noted `badShape`.

### Field Mapping

`fieldMapping` is `{base?: {field, priority}, variations?: {"<name>": {field, priority}}}`.

| Slot | IRI | Facts |
|---|---|---|
| `base` | `F = {T}/fieldMapping:base` | `T sem:hasFieldMapping F`; `F a sem:FieldMappingRule`; `F sem:mapsToField str(field)`; `F sem:mappingPriority int(priority)` |
| each variation | `F = {T}/fieldMapping:variation:{n}`, where `n` is the name's index in the sorted names of `variations` | the same facts, and `F sem:mappingVariation str(name)` |

On a Label or Variation tag, a `variations` slot is mapped and noted `tagWrongType`.

## Performers (every `SemanticPerformer`)

| Source | Facts |
|---|---|
| the performer | `P a sem:Performer` |
| `tags` | for each tag among the reader's tags: `P sem:hasTrait T` if it is a Trait; `P sem:hasLabel T` if it is a Label; `P sem:hasPerformerVariation T` if it is a Variation. A tag with several of these flags gives several facts. Other tags give nothing |
| `archetype` | `P a sem:Archetype` |
| `archetypeParents` | `P sem:parentArchetype P(x)` for each id |
| `measurements` | `P sem:bustSize dec`, `P sem:waist int`, `P sem:hip int` and `P sem:waistToHipRatio dec`, each only when spec 005 derived it |

`archetypeParents` on a performer that is not an Archetype is mapped and noted
`tagWrongType`. `archetypeGraphPosition` is never written.

## Media (every `MediaRecord`)

| Source | Facts |
|---|---|
| the item | `M a sem:Scene`, or `M a sem:Image`; `M sem:hasPerformer P(x)` for each of `performers` |

### Assignments

Let `I` be the instance IRI. Every instance gets `I sem:inMedia M`, `I sem:forPerformer P(p)` and
`I sem:ofTag T(t)`.

| Key and shape | Facts |
|---|---|
| `roleTagAssignments`: `{p: [t, …]}` | `I = role(M,p,t)`; `I a sem:RoleInstance`; `P(p) sem:hasRole I` |
| `actionAssignments`: `{p: [{actionTagId: t, variations: [v, …]}, …]}` | `I = action(M,p,t)`; `I a sem:ActionInstance`; `P(p) sem:hasAction I`; `I sem:hasVariation str(v)` for each variation |
| `pluggableAssignments`: `{p: [t, …]}` | `I = pluggable(M,p,t)`; `I a sem:PluggableInstance`; `P(p) sem:hasPluggable I` |
| `autoRoleAssignments`: `{p: [t, …]}` | `I = role(M,p,t)`, with every fact of a role instance, and `I sem:roleAppliedAutomatically bool(true)`. Noted `notRecordedRole` if the role is not also in `roleTagAssignments` |

**Notes on assignments:**
- **Repeated entries:** the same (performer, tag) listed twice gives one instance. Variations
  from both entries are kept.
- **Tag of another type:** an assignment whose tag lacks the expected flag is noted
  `tagWrongType`. A tag the reader did not return is noted `tagMissing`. Both are mapped.
- **Performer not on the item:** the instance is mapped and noted `performerNotInMedia`.

### Pairings

The IRI is `Q = pairing(M, pairingId)`. The ends `a` and `b` name instances of the same record.

| Key | Facts |
|---|---|
| `actionPairings`: `[{a: {performer, actionTagId}, b: {…}, direction?, pairingId}]` | `Q a sem:ActionPairing`; `Q sem:inMedia M`; `Q sem:pairingId str(pairingId)`; `Q sem:pairsA action(M,a…)`; `Q sem:pairsB action(M,b…)`; `Q sem:pairingDirection str(direction or "bidirectional")` |
| `roleTagPairings`: `[{a: {performer, roleTagId}, b: {…}, pairingId}]` | the same, with `sem:RolePairing` and role instances. `pairingDirection` only when a direction is stored |

**Notes on pairings:**
- **An end with no instance:** if an end names an instance that the record's assignments do not
  hold, the pairing is skipped and noted `danglingRef`.
- **Unknown direction:** a direction other than `bidirectional`, `give` or `receive` is noted
  `badShape`, and the pairing is mapped without a direction.

### Relations

| Key | Facts |
|---|---|
| `relationAssignments`: `[{relationTagId: r, a, b, auto?, ruleId?}]` | `X = {M}/relation:{r}:{a}:{b}`; `X a sem:RelationInstance`; `X sem:inMedia M`; `X sem:ofRelation T(r)`; `X sem:atEndA P(a)`; `X sem:atEndB P(b)`; `X sem:appliedAutomatically bool(auto == true)`; `X sem:appliedByRule {T(r)}/rule:{ruleId}`, when `ruleId` is present |

### Timeline

`timeline` is an ordered list. Let `L = timeline(M)`. When the list is non-empty:
`M sem:hasTimeline L`; `L a sem:Timeline`; `L sem:inMedia M`.

For entry number `i` (from 0), with `id`, let `E = timeline_entry(M, id)`. Every entry gets
`L sem:hasEntry E`, `E sem:position nni(i)` and `E sem:entryId str(id)`.

| Entry | Facts |
|---|---|
| `type: "stage"`, with `miseEnScenePicks: [{tagId, variation?}]` | `E a sem:Stage`. For each distinct `tagId`, `K = stage_pick(E, tagId)`: `E sem:hasPick K`; `K a sem:StagePick`; `K sem:ofTag T(tagId)`; `K sem:hasVariation str(v)` for each distinct non-empty variation |
| any other entry, with `ref.kind` in `action-instance`, `role-instance` | `E a sem:Slot`; `E sem:places I`, where `I` is the instance `ref` names (`performerId` with `actionTagId` or `roleTagId`) |
| any other entry, with `ref.kind` in `action-pairing`, `role-pairing` | `E a sem:Slot`; `E sem:places pairing(M, ref.pairingId)` |
| every slot | `E sem:slotSeconds dec(seconds)`, when `seconds` is a number; `E sem:linkedMarker ItemIris::marker(markerId)`, when present |
| every slot placing an instance, with seconds | `I sem:atTime dec(seconds)` |

**Mise-en-scène instances:** after the entries, for each distinct `tagId` picked on any Stage of
the item, let `N = mise_en_scene_instance(M, tagId)`. It gets:
- `M sem:hasMiseEnScene N`; `N a sem:MiseEnSceneInstance`; `N sem:inMedia M`;
  `N sem:ofTag T(tagId)`;
- `N sem:hasVariation str(v)` for every distinct variation picked for that tag;
- `N sem:fromPick K` for each of its picks.

**Skipped entries and slots:**

| Case | Note |
|---|---|
| an entry with no `id` | `badShape` |
| a slot with no `ref` | `badShape` |
| `ref.kind` of `mise-en-scene` | `retired`; the slot is skipped |
| any other `ref.kind` | `unknownKind`; the slot is skipped |
| a slot whose instance or pairing is not in the record | `danglingRef`; the slot keeps its other facts, without `places` or `atTime` |

`sem:inStage` is never written: it is a rule over `position`.

## The vocabulary

Every build also writes every fact of `core.ttl`, `labels.ttl` and `timeline.ttl` from the
embedded vocabulary (research R7).

## What is never written

- `archetypeGraphPosition`;
- the retired keys `isGeneric` and `looks`;
- native media tags (`sem:hasTag` is Planned);
- `variationData.traitTagIds`;
- performer names, and any value that is only display data.
