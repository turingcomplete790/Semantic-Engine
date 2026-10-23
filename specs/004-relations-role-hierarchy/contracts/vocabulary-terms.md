# Contract: vocabulary 1.2.0 additions

These terms are added to `vocabulary/core.ttl` and `vocabulary/stash-mapping.ttl`. All of them
are new, so vocabulary 1.1.0 becomes **1.2.0** (MINOR, rule V014), and no existing term changes
meaning (FR-017).

Every term carries the required annotations (V005):
- `sem:source "specs/004-relations-role-hierarchy/spec.md (owner, 2026-10-21)"`;
- `sem:sourceRevision "owner request 2026-10-21; not in the plugin documents at 3c82c85"`;
- `sem:status sem:Active`;
- `sem:since "1.2.0"`.

Every property has exactly one domain and one range (V004), and every property and assignment
class has a `sem:scope` (V006).

## Classes

| Term | Subclass of | Scope | Meaning |
|---|---|---|---|
| `sem:Relation` | `sem:Tag` | — | A Relation tag: links two performers in one scene or image; defined by the Roles at its two ends. |
| `sem:RelationInstance` | `sem:Placeable`, `sem:MediaScoped` | `sem:SceneScoped` | One Relation linking two performers in one scene or image, applied by hand or automatically. |
| `sem:RelationRule` | — | — | An auto-apply rule of a Relation: a condition per end and the Actions that may link the pair. |

## Properties

| Term | Domain | Range | Scope | Meaning |
|---|---|---|---|---|
| `sem:broaderRole` | `sem:Role` | `sem:Role` | `sem:Global` | A broader role. A narrower role takes part in every relation of its broader roles. Sub-property of `sem:parentTag`. |
| `sem:broaderRelation` | `sem:Relation` | `sem:Relation` | `sem:Global` | A broader relation. Recursive searches for it also match this one. Sub-property of `sem:parentTag`. |
| `sem:endARole` | `sem:Relation` | `sem:Role` | `sem:Global` | The Role at the relation's first end. |
| `sem:endBRole` | `sem:Relation` | `sem:Role` | `sem:Global` | The Role at the relation's second end. |
| `sem:hasRelationRule` | `sem:Relation` | `sem:RelationRule` | `sem:Global` | One of the relation's auto-apply rules. |
| `sem:relationRuleId` | `sem:RelationRule` | `xsd:string` | `sem:Global` | The rule's stable id within its relation. |
| `sem:endALabel` | `sem:RelationRule` | `sem:Label` | `sem:Global` | End A's condition: the performer has this Label. |
| `sem:endARoleCondition` | `sem:RelationRule` | `sem:Role` | `sem:Global` | End A's condition: the performer has this Role, or a narrower one, in the item. |
| `sem:endBLabel` | `sem:RelationRule` | `sem:Label` | `sem:Global` | End B's condition: the performer has this Label. |
| `sem:endBRoleCondition` | `sem:RelationRule` | `sem:Role` | `sem:Global` | End B's condition: the performer has this Role, or a narrower one, in the item. |
| `sem:linkingAction` | `sem:RelationRule` | `sem:Action` | `sem:Global` | An Action that may link the pair. If a rule has none, any Action links. |
| `sem:ofRelation` | `sem:RelationInstance` | `sem:Relation` | `sem:SceneScoped` | The relation this instance applies. |
| `sem:atEndA` | `sem:RelationInstance` | `sem:Performer` | `sem:SceneScoped` | The performer at end A. |
| `sem:atEndB` | `sem:RelationInstance` | `sem:Performer` | `sem:SceneScoped` | The performer at end B. |
| `sem:appliedAutomatically` | `sem:RelationInstance` | `xsd:boolean` | `sem:SceneScoped` | True when auto-apply wrote the instance. |
| `sem:appliedByRule` | `sem:RelationInstance` | `sem:RelationRule` | `sem:SceneScoped` | The rule that applied it, for automatic instances. |

An instance's scene or image uses the existing `sem:inMedia`.

## Mappings (`stash-mapping.ttl`)

| Term | From entity | From field | Allowed values | Produces |
|---|---|---|---|---|
| `sem:map_isRelation` | Tag | `custom_fields.isRelation` | 1 (stored as a 0/1 value; other values mean not set) | `sem:Relation` |
| `sem:map_relationEnds` | Tag | `custom_fields.relationEnds` | JSON `{"a": roleTagId, "b": roleTagId}` | `sem:endARole` |
| `sem:map_relationRules` | Tag | `custom_fields.relationRules` | JSON list of `{id, a: {label \| role}, b: {label \| role}, actions?}` | `sem:RelationRule` |
| `sem:map_relationAssignments` | Scene, Image | `custom_fields.relationAssignments` | JSON list of `{relationTagId, a, b, auto?, ruleId?}` | `sem:RelationInstance` |
| `sem:map_autoRoleAssignments` | Scene, Image | `custom_fields.autoRoleAssignments` | JSON map of performer id to the Role tag ids auto-apply added | `sem:RoleInstance` |

Each mapping's `rdfs:comment` adds detail:
- **`sem:map_relationEnds`** also produces `sem:endBRole`.
- **`sem:map_autoRoleAssignments`** marks which roles are automatic. The same roles also appear
  in `roleTagAssignments`.
- **`sem:map_tagParents`** (existing) gains a comment: a parent with the same type flag (Role
  under Role, Relation under Relation) also produces `sem:broaderRole` or `sem:broaderRelation`.

## Item IRIs

These follow the existing scheme in `semantic_vocabulary::iri::ItemIris`, where derived IRIs are
built on the media item's IRI or the tag's IRI:

- **Relation instance:** `{media}/relation:{relationTagId}:{a}:{b}`, for example
  `urn:semantic-stash:{serverId}:scene:12/relation:45:7:8`. It is deterministic, and the ordered
  ends keep the two directions distinct.
- **Relation rule:** `{relationTag}/rule:{ruleId}`, the same pattern as 1.1.0's Label rules.

## Change log (`vocabulary/CHANGELOG.md`)

The `1.2.0` entry lists the terms above under "Deliberate additions (constitution Principle
II)". It notes that the plugin documents at `3c82c85` have no Relations, and that the addition
is additive.

`TERMS.md` and `released/1.2.0.json` are regenerated with
`vocab-check --write-terms --write-snapshot`.
