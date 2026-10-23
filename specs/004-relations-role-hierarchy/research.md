# Research: Relations, Role Hierarchy, and Auto-Apply

Sources:
- the spec, the constitution (v3.0.0), and the 003 decision record;
- the vocabulary files (1.1.0);
- the test-instance fixtures (`crates/stash-adapter/tests/fixtures/test-instance/`);
- Stash's pinned schema (`schemas/stash-v0.31.1.graphql`);
- the semantic-tagging design documents at commit `3c82c85`.

## R1. What this feature builds now, given the engine phases

- **Decision:** Auto-apply evaluates **one scene or image at a time, from Stash data**. It needs
  that item's assignments, its performers' Labels, and the role and relation definitions. It
  does not need the full semantic graph (ROADMAP Phase 2), so it is built now on the 002 plugin.
  The rest is split by what it depends on:
  - **Built now, recursive search:** the hierarchy semantics and their expansion (narrower roles
    and relations, recursive or not), exposed through the new `relations` operation.
  - **Phase 3, recursive search:** the search operation itself is built with Phase 3's query
    operations, which call this feature's expansion. SC-004 and SC-007 are verified there.
- **Rationale:** the owner wants auto-apply on save now. Evaluation only needs one item's data,
  and Phase 3's search does not exist yet. Building a search here would design Phase 3 early.
- **Alternatives:**
  - **Build Phase 2 and 3 inside this feature:** rejected, too large.
  - **Vocabulary only, engine work later:** rejected, because auto-apply on save is the point.

## R2. How relations and the hierarchy are stored in Stash (FR-015, FR-017)

- **Decision:** use new tag custom fields and a Stash tag flag. Hierarchy uses Stash's own tag
  parents. Relation instances, and the record of which roles were added automatically, go in new
  scene and image custom fields.

  | Where | Key | Value |
  |---|---|---|
  | Tag | `isRelation` | `1`, like the other type flags |
  | Tag (Relation) | `relationEnds` | JSON `{"a": "<roleTagId>", "b": "<roleTagId>"}` |
  | Tag (Relation) | `relationRules` | JSON list of rules ([data-model](data-model.md#auto-apply-rule)) |
  | Tag (Role, Relation) | Stash `parents` | broader role or relation, when the parent has the same type flag |
  | Scene, Image | `relationAssignments` | JSON list of relation instances |
  | Scene, Image | `autoRoleAssignments` | JSON `{performerId: [roleTagId, …]}`: the end roles auto-apply added |

- **Automatic roles:**
  - Auto-assigned end roles are also written into the existing `roleTagAssignments`, so the
    plugin, the viewer, and role-based rules see them as ordinary role assignments (FR-008).
  - `autoRoleAssignments` records which ones auto-apply added, so they can be removed later
    (FR-009).
  - A role that was already in `roleTagAssignments` before auto-apply is never recorded there,
    so it is never removed.
- **Rationale:** `roleTagAssignments` entries are plain tag ids (`{"7": ["19"]}` in the
  fixtures), so they cannot carry an "automatic" mark. Changing their shape would break the
  plugin. Separate keys are additive: clients that don't know them ignore them (FR-017).
  Stash parents already exist for every tag and are mapped as `sem:parentTag`.
- **Alternatives:**
  - **Turn entries into objects with an `auto` field:** rejected, it breaks the plugin.
  - **A `broaderRole` custom field:** rejected, it duplicates Stash parents.

## R3. What "linked by an action" means (FR-006)

- **Decision:** two performers in one scene or image are **linked** only when an
  `actionPairings` entry joins them, in either direction.
  - A variation tag (`variantOfTagId`) counts as its base Action.
  - A rule's optional `actions` list limits which paired Actions count. An empty list means any
    Action.
  - Holding the same Action tag without a pairing does **not** link two performers
    (clarification, 2026-10-21).
- **Rationale:** the owner's rule is that without a pairing there is no way to know whether two
  performers did the action together. In a three-person scene, P1 may be kissing P3, not P2.
  Pairings are the documents' explicit "these two do this together" link.
- **Alternatives:**
  - **Also count a shared Action tag:** rejected by the owner. It would apply relations between
    performers who never interacted.

## R4. Hierarchy rules (FR-002 to FR-004)

- **Decision:**
  - A Stash parent counts as a broader term only when the parent carries the **same type flag**
    (Role under Role, Relation under Relation). Other parents keep their plain `parentTag`
    meaning.
  - A tag may have several broader terms.
  - Loops are found by a depth-first walk. Every link on a loop is ignored and reported.
  - A narrower relation is valid only if each of its ends is the same role as, or narrower than,
    the matching end of every broader relation. Invalid relations are reported and not applied.
- **Rationale:** reusing parents gives the owner one place to edit hierarchy, Stash's own tag
  editor. Limiting it to same-type parents keeps existing parent links, which are categories in
  the owner's library, from becoming hierarchy by accident.
- **Alternatives:**
  - **Any parent counts:** rejected; tag trees used for browsing would change role meaning.

## R5. Evaluation and the change it writes (FR-007 to FR-010)

- **Decision:** for one scene or image:
  1. List the performers. For every **ordered** pair (p, q) of different performers, and every
     relation rule:
     - apply (relation, a = p, b = q) when p meets end A's condition, q meets end B's, and p and
       q are linked (R3);
     - a Label condition holds when the performer has that Label tag;
     - a Role condition holds when the performer has that Role, or a narrower one, in this item.
  2. **Desired automatic instances** = the set of matches. Leave out a match when a hand-applied
     instance with the same relation and ends already exists.
  3. **Desired automatic roles** = for each desired instance, end A's role for p and end B's role
     for q. Leave out roles the performer already holds by hand.
  4. Compare with the stored automatic instances and roles:
     - remove what is no longer desired, and add what is new;
     - leave hand-applied entries untouched;
     - if nothing differs, write nothing.
- **Rationale:** the result depends only on the item's data and the definitions, so evaluating
  twice gives the same answer (SC-006). Ordered pairs give the direction (Edge Cases: reversed
  direction, both directions qualify).
- **Role-based rules see automatic roles:** they read `roleTagAssignments`, which includes roles
  added automatically. Evaluation repeats until nothing changes, capped at 4 passes per item.
  A rule chain that keeps changing is reported, not written past the cap.

## R6. Starting the work after a save (FR-007, FR-012), replacing 003 rule 6

- **Decision:** a **journal** plus a **background job**.
  - **The hook:** subscribed to `Scene.Update.Post`, `Scene.Create.Post`, `Image.Update.Post`
    and `Image.Create.Post`. It appends `{type, id}` to `journal.jsonl` in the plugin folder.
    Then:
    - if no `apply.pending` marker exists, it creates one and asks Stash to start the task
      "Apply relations" (`runPluginTask`);
    - if a marker exists, it only appends.

    So a 1,000-scene bulk edit starts **one** job: only the first hook pays for the request.
  - **The task:**
    1. Take the journal atomically, by renaming it.
    2. Evaluate those items in batches.
    3. Remove the marker.
    4. Check the journal again. If entries arrived, recreate the marker and loop.

    A duplicate job started in the gap finds an empty journal and exits.
  - **Loop safety:** the task's own writes fire hooks too. Those entries are processed once,
    find no change (R5), and write nothing, so the loop ends after one extra pass. Stash's own
    cyclic-hook guard (depth 10) is never reached.
- **Rationale:**
  - **No client pays:** the owner ruled out charging the apply to the next client call, since a
    1,000-scene edit would add seconds at random. Stash jobs run in its job queue, after the
    save returns.
  - **Hook cost:** the hook does a file append, and for the first hook in a burst one GraphQL
    request to the Stash server that invoked it (Principle IV).
- **To measure** (risk): the cost of the first hook in a burst, with the `runPluginTask` request
  added. It must stay within the 7 ms budget (SC-003). A live test measures save time with the
  plugin enabled and disabled, as `spike-bench hooks` did.
  - **Fallback if it is over budget:** the hook only appends. The job is then started by the
    catch-up in R7 on the next task run or by the owner. This fallback is recorded if used.
  - **Measured 2026-10-23** (T031–T032, `crates/plugin/tests/live_relations.rs` `save_cost`,
    test server, median save time):

    | Save | Median | Extra from the plugin |
    |---|---|---|
    | Plugin disabled | 1.3–1.5 ms | baseline |
    | Hook only appends | 3.0–3.2 ms | +1.5 to +1.8 ms |
    | First save of a burst (also starts the job) | 6.5–6.8 ms | +5.1 to +5.3 ms |

    - **Within budget, no fallback:** the hook stays as designed.
    - **The fix that got it there:** the first measurement of a burst's first save was +14.2 ms,
      over budget. About 8 ms of that was building the HTTP client: it loaded the platform TLS
      certificate store even for Stash's plain `http://` address.
      `stash-adapter` now skips loading certificates for `http://` endpoints
      (`tls_certs_only([])`). `https://` keeps full verification. Client set-up went from about
      8.2 ms to 0.18 ms, which every plugin call now saves.
    - **Freshness:** applied 109–257 ms after a save (SC-002 asks 30 s).
    - **Bulk edit:** 1,000 scenes were fully applied 6.0–7.9 s after the bulk edit, by one job
      (SC-005 asks 5 minutes).
- **Alternatives:**
  - **Apply inside the hook:** rejected; it adds about 190 ms per item to saves (003).
  - **Apply on the next engine call:** rejected by the owner.
  - **A long-lived worker:** rejected in 003 (Principle IV).
- **003 decision record:** rule 6 ("the next engine call applies") is replaced by this design.
  A note is added to the record's rule 6.

## R7. Catching up when hooks were missed (Principle IV, Edge Cases)

- **Decision:**
  - Each run of "Apply relations" also asks Stash for scenes and images with `updated_at` after
    the **watermark**, the start time of the last successful run.
  - The watermark is stored in the plugin folder. Thresholds are sent in the server's local-time
    format, one second early, and never together with `ids` (003 findings).
  - With no watermark (first install), there is no catch-up; the owner runs "Re-apply
    relations".
- **Rationale:** it covers saves made while the plugin was disabled or a hook was lost, for 2–3
  ms per check (003).

## R8. Stash I/O (Principle V, Principle VII, Cynic)

- **Decision:** add typed Cynic operations to `stash-adapter`. These are the shipped plugin's
  first writes; Principle I requires writes to go through GraphQL.
  - **Reads, all bounded and flag-filtered:**
    - tags flagged `isRole`, `isRelation`, `isLabel` or `isAction`, with `parents` and
      `custom_fields`;
    - scenes or images by an `ids` list, in batches of 100, with `custom_fields` and performer
      ids;
    - performers by an `ids` list, with their tag ids (for Labels);
    - scene and image ids with `updated_at` after a threshold.
  - **Writes:**
    - `sceneUpdate` and `imageUpdate` with a partial `custom_fields` update of
      `relationAssignments`, `autoRoleAssignments` and `roleTagAssignments` only;
    - `runPluginTask`.
- **No N+1:** definitions are read once per task run, and items and performers are read per
  batch, not per item.
- **Concurrent edits:**
  - The task re-reads an item just before writing it. If `updated_at` moved since its read, it
    re-evaluates that item instead of writing.
  - A small window remains, because Stash has no compare-and-set. It is documented in the
    contract.

## R9. The re-apply task (FR-011)

- **Decision:**
  - **What it covers:** the task "Re-apply relations" takes `scope` (`all`, `scenes` or
    `images`) and an optional `ids` list.
  - **How it runs:** it walks the items in batches of 100, using the same evaluation as R5.
  - **Progress:** it reports progress through Stash's plugin progress log lines (002's logger).
  - **Cancellation:** cancelling stops the process. Each item's write is a single update, so a
    stopped run leaves every item either old or new, and a rerun is safe (R5).
- **Rationale:** the owner chose an on-demand background job (clarification Q2 C). Progress and
  cancellation are Principle VII requirements.

## R10. Explaining a relation (FR-014)

- **Decision:** a new operation, `explainRelation`, takes an item (`scene` or `image`, and id)
  and a relation instance (relation tag id, performers a and b).
  - It re-evaluates that item and returns:
    - which rule matched;
    - the Label or Role (and through which narrower role) that met each end;
    - the linking Action and the action pairing that linked them.
  - A hand-applied instance is reported as such.
- **Rationale:** the explanation is computed from the same evaluation, so it can't disagree with
  what was applied (Principle III).

## R11. Contracts and versions (Principle VI)

- **Decision:**
  - **Operations contract 1.0.0 → 1.1.0** (MINOR, additive): adds the operations `relations`,
    `explainRelation` and `recommendations`, the tasks "Apply relations" and "Re-apply relations", and the hook.
    `api/operations.json` and `docs/api.md` are updated.
  - **Vocabulary 1.1.0 → 1.2.0** (MINOR, additive): adds the terms in
    [contracts/vocabulary-terms.md](contracts/vocabulary-terms.md).
- **Rationale:** both changes add things without breaking anything that exists. The checks
  (V014) require a MINOR bump for additions.

## R12. Coordination with the plugin (Principle II)

- **Decision:**
  - The new terms cite this spec as their source (`sem:source "specs/004-relations-role-hierarchy/spec.md
    (owner, 2026-10-21)"`), not a plugin document.
  - The vocabulary change log lists them under "Deliberate additions (constitution Principle
    II)".
  - A note for the semantic-tagging plugin lists the new keys and their meaning, so its
    documents can adopt them.
- **Rationale:** Principle II lets the engine add to the contract if the addition is deliberate,
  documented, and safe for existing data. Nothing existing changes meaning (FR-017).

## R13. Recommendations in the engine (FR-018 to FR-020)

- **Decision:** a new operation, `recommendations`, takes an item (`scene` or `image`, and id)
  and returns recommended tag ids for Actions, Roles, Pluggables, Mise-en-scène and Relations.
  - **Which tags:** a vocabulary tag of a category is recommended when it is among the item's
    native Stash tags. For Actions, Mise-en-scène and Relations it is also recommended when one of
    those native tags has `variantOfTagId` pointing at it.
  - **What is returned:** each recommendation names the native tag that suggested it.
  - **Reads:** one read of the item's native tags. Variant tags are read once and filtered by
    flag, as `FIND_VARIANT_TAGS` does in the plugin.
  - **Writes:** none. Native tags are only read (clarification Q, 2026-10-21).
- **Rationale:** this ports the plugin's `computeRecommendedTagIds` (plugin roadmap, Milestone
  1C: a direct match, or a match through a common variant) into the engine, which Principle III
  requires. Relations are one more category with the same rule.
  - **Variants and categories** (corrected 2026-10-23, after reading the call sites for T035):
    the plugin resolves common variants for Actions (`actions.js`) and Mise-en-scène
    (`timeline.js`). Roles (`roles.js`) and Pluggables (`pluggables.js`) pass an empty variant
    map, so they match directly only. The engine follows the same rule per category, so a variant
    that points at a Role is not resolved. Relations resolve variants like Actions.
- **Parity:** a fixture test checks that the engine's output for the four existing categories
  matches the plugin's (SC-008). The plugin's call sites are `actions.js`, `roles.js`,
  `pluggables.js` and `timeline.js`.
- **Alternatives:**
  - **Leave recommendations in the plugin:** rejected by Principle III and clarification Q1.
  - **Write applied Relations into native tags:** rejected by the owner (clarification Q2).
