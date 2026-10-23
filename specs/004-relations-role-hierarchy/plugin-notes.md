# Notes for the semantic-tagging plugin (spec 004)

Constitution Principle II makes the plugin's design documents the contract for the semantic data.
Spec 004 adds to that data: Relations, role and relation hierarchies, and auto-apply. The plugin's
documents at commit `3c82c85` do not describe any of it. This note lists everything added, so the
documents can adopt it.

Nothing existing changes shape or meaning. A plugin that does not know these keys keeps working,
and ignores them.

## New tag data

| Tag field | Meaning |
|---|---|
| `custom_fields.isRelation` | `1` marks a **Relation** tag, like `isRole` marks a Role |
| `custom_fields.relationEnds` | JSON text `{"a": "<roleTagId>", "b": "<roleTagId>"}`: the Roles at the relation's two ends, in order. Both are required; `a` and `b` may be the same Role |
| `custom_fields.relationRules` | JSON text: a list of auto-apply rules (below) |
| Stash `parents` | A Role whose parent is also a Role has that parent as a **broader role**. The same applies to Relations. Parents of another type keep their plain meaning |

**Auto-apply rules.** One rule:

```json
{ "id": "r1", "a": { "label": "<labelTagId>" }, "b": { "role": "<roleTagId>" }, "actions": ["<actionTagId>"] }
```

- **`id`:** required, and unique within its relation.
- **`a` and `b`:** each has exactly one of `label` (the performer has that Label) or `role` (the
  performer has that Role, or a narrower one, in the scene or image).
- **`actions`:** optional. It limits which Actions may link the pair; empty or missing means any
  Action. A common variant (`variantOfTagId`) counts as its base Action.
- **Linking:** two performers are linked only when an `actionPairings` entry joins them. Both
  holding the same Action without a pairing does not link them.

## New scene and image data

| Field | Meaning |
|---|---|
| `custom_fields.relationAssignments` | JSON text: a list of `{relationTagId, a, b, auto?, ruleId?}`. `a` and `b` are performer ids at the relation's ends. `auto: true` with `ruleId` marks an entry the engine applied; any other entry was applied by hand |
| `custom_fields.autoRoleAssignments` | JSON text `{performerId: [roleTagId, …]}`: the end roles the engine added |

**End roles appear in the plugin's own field.** When the engine applies a relation automatically,
it also adds each end's Role to the performer at that end. These roles go into the existing
`roleTagAssignments`, so the plugin's role panels show them and role-based searches see them.
`autoRoleAssignments` only records which roles the engine added, so it can remove them when the
relation stops applying.

**Rules for the plugin when it writes:**
- The engine never changes or removes entries applied by hand, in `relationAssignments` or in
  `roleTagAssignments`.
- If the plugin applies a relation by hand, it writes an entry without `auto`. The engine then
  leaves that (relation, a, b) to the plugin.
- If the plugin removes a role that `autoRoleAssignments` lists, the engine adds it back while
  the relation still applies. To stop that, remove the relation or its rule instead.

## When the engine writes

The engine's hook records each scene or image save, and a background job named "Apply
relations" applies relations shortly afterwards (about 0.1–0.3 s on the test server). It adds
about 1.5–5 ms to a save, and nothing to the plugin's own calls.

Changing a rule, a hierarchy, or a performer's Labels does not re-check other scenes. The owner
runs the "Re-apply relations" task for that.

## Recommendations are now in the engine

The engine's `recommendations` operation (contract 1.1.0) gives the same results as the plugin's
`computeRecommendedTagIds` for Actions, Roles, Pluggables and Mise-en-scène. It also recommends
Relations from the item's native tags. Under constitution v3.0.0, the plugin can call it and
retire its own recommendation code:

```graphql
mutation { runPluginOperation(plugin_id: "semantic-engine", args: { op: "recommendations", media: "scene", id: "<id>" }) }
```

The parity cases are in `crates/engine/tests/fixtures/recommend/`.

## Vocabulary

The new terms are in vocabulary 1.2.0 (`vocabulary/CHANGELOG.md`, "Deliberate additions"), for
example `sem:Relation`, `sem:RelationInstance`, `sem:broaderRole` and `sem:linkingAction`.
