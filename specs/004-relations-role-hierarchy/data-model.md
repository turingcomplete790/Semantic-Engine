# Data Model: Relations, Role Hierarchy, and Auto-Apply

All of this lives in Stash (constitution Principle I). The engine reads it, and writes only the
scene and image keys marked "engine writes". The storage choices come from
[research.md](research.md) R2.

## Role (extended)

An existing tag flagged `isRole`.

| Field | Source | Rule |
|---|---|---|
| id, name | Stash tag | — |
| broader roles | Stash `parents` that are also flagged `isRole` | zero or more; loops ignored and reported (R4) |
| narrower roles | derived: tags whose broader roles include this one | computed, any depth |

**Takes part in:** every relation whose end is this role or any broader role, at any depth
(FR-002).

## Relation

A tag flagged `isRelation` (new).

| Field | Stash key | Rule |
|---|---|---|
| id, name | Stash tag | — |
| end A, end B | `custom_fields.relationEnds` `{"a": roleId, "b": roleId}` | both required and both must be Role tags; a and b may be equal (symmetric) |
| broader relations | Stash `parents` that are also flagged `isRelation` | each end must be the same role as, or narrower than, the matching end of every broader relation, or the relation is invalid (FR-003) |
| rules | `custom_fields.relationRules` | list of auto-apply rules; may be empty |

**Validity:** a relation that is missing an end, names a non-Role end, sits on a hierarchy loop,
or breaks the end rule is **invalid**. It is reported by `relations` and never applied.

## Auto-apply rule

One entry of `relationRules`:

```json
{ "id": "r1", "a": { "label": "<labelTagId>" }, "b": { "role": "<roleTagId>" }, "actions": ["<actionTagId>"] }
```

| Field | Rule |
|---|---|
| `id` | stable within its relation; required; unique per relation |
| `a`, `b` | exactly one of `label` (a Label tag id) or `role` (a Role tag id) |
| `actions` | Action tag ids that may link the pair; empty or missing means any Action (R3) |

**Meaning:** in one scene or image, performer p at end A and performer q at end B (p ≠ q) get
the relation when all three hold:
- p meets `a`: has the Label, or has the Role or a narrower one in this item;
- q meets `b`, the same way;
- an action pairing on an allowed Action joins p and q. A variation counts as its base Action.
  Holding the same Action without a pairing does not link them (research R3).

## Relation instance

One entry of a scene's or image's `custom_fields.relationAssignments` (engine writes the
automatic entries; people or clients write the hand-applied ones):

```json
{ "relationTagId": "<id>", "a": "<performerId>", "b": "<performerId>", "auto": true, "ruleId": "r1" }
```

| Field | Rule |
|---|---|
| `relationTagId` | a Relation tag |
| `a`, `b` | performers in this item; `a ≠ b` |
| `auto` | `true` when written by auto-apply, otherwise `false` or missing (hand-applied) |
| `ruleId` | the rule that applied it; present when `auto` is `true` |

**Uniqueness:** at most one entry per (relationTagId, a, b). A hand-applied entry takes
precedence: auto-apply never adds a duplicate of it.

## Automatic role record

A scene's or image's `custom_fields.autoRoleAssignments` (engine writes):
`{ "<performerId>": ["<roleTagId>", …] }`.

The roles listed here were added to `roleTagAssignments` by auto-apply as relation end roles
(FR-008):
- they are removed from both keys when no automatic instance needs them any more (FR-009);
- a role the performer held in `roleTagAssignments` before auto-apply added it is never listed,
  so it is never removed.

## Journal and run state (plugin folder, not Stash)

These are local files in the plugin's folder: a disposable cache, per Principle I.

| File | Content | Written by |
|---|---|---|
| `journal.jsonl` | one `{type: "scene" \| "image", id}` per hook | the hook (append) |
| `apply.pending` | present while an "Apply relations" job is queued or running | hook (create), task (remove) |
| `apply-watermark` | start time of the last successful "Apply relations" run, in Stash's local-time format | task |

## State transitions of one automatic instance

```text
            rule holds on evaluation
 (absent) ───────────────────────────▶ applied (auto: true, ruleId)
    ▲                                      │
    │   rule no longer holds, or the       │ rule still holds:
    │   relation becomes invalid or is     │ no write
    └──────── deleted ◀────────────────────┘
```

**When a hand-applied instance appears:** if the owner applies the same (relation, a, b) by
hand, the automatic entry is replaced by the hand-applied one. Auto-apply leaves it alone from
then on.

## Recommendation (operation result, not stored)

Computed by the `recommendations` operation for one scene or image (research R13).

```json
{ "actions": [ { "tag": "21", "from": "88" } ], "roles": [], "pluggables": [],
  "miseEnScene": [], "relations": [ { "tag": "45", "from": "45" } ] }
```

| Field | Rule |
|---|---|
| `tag` | a vocabulary tag of that category (flagged `isAction`, `isRole`, `plugtag`, `isMiseEnScene`, or `isRelation`) |
| `from` | the native tag on the item that suggested it: the tag itself, or a tag whose `variantOfTagId` is `tag` |

Common variants (`variantOfTagId`) count for Actions, Mise-en-scène, and Relations only; Roles
and Pluggables match directly, as in the plugin.

The operation only reads; it never writes native tags (FR-020).

## Vocabulary terms

The terms that describe this model in RDF are listed in
[contracts/vocabulary-terms.md](contracts/vocabulary-terms.md).
