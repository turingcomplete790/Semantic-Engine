# Contract: operations 1.1.0 (additions)

These additions change the plugin's operation contract from **1.0.0** to **1.1.0**. The change
is a MINOR, additive one (constitution Principle VI); nothing in 1.0.0 changes.

`api/operations.json` and `docs/api.md` get the entries below. The existing agreement test keeps
both in line with the plugin.

Every operation answers in the 1.0.0 envelope: `{ ok, op, contractVersion, result | error }`.

**New error code:** `NOT_FOUND`, for a scene or image id that does not exist. It is added to
`ErrorCode`, `api/operations.json` and `docs/api.md`.

## Operation `relations`

Returns the roles and relations with their hierarchy, rules, and every problem found.

**Arguments:**

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `tag` | tag id | no | Return only this role or relation and its broader and narrower terms |
| `recursive` | boolean | no, default `false` | With `tag`: whether `narrower` lists every depth or only direct children |

**Result:**

```json
{
  "roles": [ { "id": "31", "name": "Patient", "broader": ["30"], "narrower": [] } ],
  "relations": [
    { "id": "41", "name": "Extra Care", "ends": { "a": "31", "b": "33" },
      "broader": ["40"], "narrower": [], "valid": true,
      "rules": [ { "id": "r1", "a": { "role": "31" }, "b": { "role": "33" }, "actions": [] } ] }
  ],
  "problems": [
    { "kind": "loop", "tags": ["30", "31"] },
    { "kind": "invalidEnds", "relation": "42", "detail": "end b 35 is not narrower than 33" }
  ]
}
```

**Problem kinds:** `loop`, `missingEnd`, `endNotRole`, `invalidEnds`, `badRule`.

**Errors:** `STASH_UNREACHABLE`, `INVALID_INPUT` (unknown `tag`).

**Reads:** tags flagged `isRole` or `isRelation`, filtered by flag, with parents and custom
fields (research R8).

## Operation `explainRelation`

Explains one relation instance on one scene or image (FR-014).

**Arguments:** `media` (`scene` or `image`), `id`, `relationTagId`, `a`, `b`. All are required.

**Result, for an automatic instance:**

```json
{
  "applied": "auto", "ruleId": "r1",
  "endA": { "performer": "7", "met": { "label": "51" } },
  "endB": { "performer": "8", "met": { "role": "33", "via": ["34"] } },
  "link": { "action": "21", "by": "pairing", "pairingId": "se-pair-1" },
  "addedRoles": { "7": ["60"], "8": ["61"] }
}
```

- **`via`:** the narrower role the performer actually holds, when a broader role was required.
- **Hand-applied:** `{ "applied": "hand" }`.
- **No such instance:** `{ "applied": "none" }`.

**Errors:** `INVALID_INPUT`, `NOT_FOUND` (no such scene or image), `STASH_UNREACHABLE`.

## Operation `recommendations`

Recommended vocabulary tags for one scene or image, by category (FR-018 to FR-020; research
R13).

**Arguments:** `media` (`scene` or `image`) and `id`. Both are required.

**Result:** the [Recommendation](../data-model.md#recommendation-operation-result-not-stored)
shape. Every category is present; an empty list means nothing is recommended.

**Errors:** `INVALID_INPUT`, `NOT_FOUND`, `STASH_UNREACHABLE`.

**Writes:** none.

## Task "Apply relations"

- **Started by:** the hook (research R6). The owner can also start it from the task list.
- **Arguments:** none.
- **What it does:**
  1. Drains `journal.jsonl` and catches up from the `updated_at` watermark (R7).
  2. Evaluates each scene or image (R5).
  3. Writes only the items whose automatic relations or roles changed.
- **Log:** progress lines, plus one summary line, for example
  `apply: 1000 items, 214 changed, 0 skipped, 7.1 s`.

## Task "Re-apply relations"

- **Started by:** the owner, when rules, hierarchy, or Labels changed (FR-011).
- **Arguments:**

  | Argument | Type | Required | Meaning |
  |---|---|---|---|
  | `scope` | `all` \| `scenes` \| `images` | no, default `all` | Which items to walk |
  | `ids` | list of ids | no | Only these items, within `scope` |

- **What it does:** walks the items in batches of 100 with the same evaluation, reporting
  progress. Stopping the job is safe; a rerun completes the rest (R9).

## Hook "Apply relations after a save"

- **Triggers:** `Scene.Create.Post`, `Scene.Update.Post`, `Image.Create.Post`,
  `Image.Update.Post`.
- **Default args:** `op: recordChange`.
- **Work:**
  1. Append the item to the journal.
  2. If no job is pending, start "Apply relations" with `runPluginTask`.
- **Rules:**
  - The hook does no evaluation and no store work (FR-012).
  - Its cost per saved item is measured against the 7 ms budget (SC-003).
  - It talks only to the Stash server that invoked it (Principle IV).
