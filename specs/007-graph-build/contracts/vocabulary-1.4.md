# Contract: vocabulary 1.4.0

These changes move the vocabulary from 1.3.0 to **1.4.0**, a MINOR release. Terms are only added,
and comments are clarified. No domain or range changes, so rule V014 needs no MAJOR. The reasons
are in [research R6](../research.md#r6-vocabulary-corrections-vocabulary-140).

Release procedure (`vocabulary/CHANGELOG.md`):
1. bump `sem:version` in `core.ttl`;
2. add the change log entry;
3. run `vocab-check --write-terms --write-snapshot`, which writes `released/1.4.0.json` and
   `TERMS.md`;
4. run `vocab-check`.

## Added terms (`core.ttl`)

| Term | Kind | Domain | Range | Sub-property of | Comment |
|---|---|---|---|---|---|
| `sem:pairsA` | property | `sem:Pairing` | `sem:TagAssignment` | `sem:pairs` | The instance at the pairing's end A (the stored `a`). |
| `sem:pairsB` | property | `sem:Pairing` | `sem:TagAssignment` | `sem:pairs` | The instance at the pairing's end B (the stored `b`). |
| `sem:roleAppliedAutomatically` | property | `sem:RoleInstance` | `xsd:boolean` | — | True when auto-apply added this role as a relation's end role (`autoRoleAssignments`). |

**Common fields:**
- **All three:** status Active, since 1.4.0, scope SceneScoped.
- **`pairsA` and `pairsB`:** source "README.md § Features; src/ui/actions.js (read as
  implementation evidence, documents win)", revision `2c04141`.
- **`roleAppliedAutomatically`:** source `specs/004-relations-role-hierarchy/spec.md`.

## Clarified comments

| Term | New `rdfs:comment` |
|---|---|
| `sem:pairs` | A pairing links these two instances. `sem:pairsA` and `sem:pairsB` say which end each one is. |
| `sem:pairingDirection` | How an Action pairing reads: `bidirectional`, `give` (end A gives to end B) or `receive` (end A receives from end B). A pairing stored without a direction is bidirectional. Role pairings carry none. |
| `sem:Derived` (status) | Defined as a rule over other data, not by a Stash key of its own. The engine computes it. A term's comment says whether the builder stores it. |
| `sem:bustSize`, `sem:waist`, `sem:hip`, `sem:waistToHipRatio` | The existing comment, plus: "Stored by the builder, from the performer's `measurements` (spec 005 FR-002b)." |
| `sem:inStage` | The existing comment, plus: "Not stored." |

## Mapping file (`stash-mapping.ttl`)

| Entry | Change |
|---|---|
| `sem:map_actionPairings`, `sem:map_roleTagPairings` | `rdfs:comment`: "Also produces sem:pairsA, sem:pairsB and sem:pairingDirection." |
| `sem:map_autoRoleAssignments` | `sem:produces` stays `sem:RoleInstance`. Add `rdfs:comment`: "Also produces sem:roleAppliedAutomatically." |

The mapping file gains no new entries. `sem:map_serverId` stays Planned.

## Item IRIs

There are no new IRI forms. `semantic_vocabulary::iri` gains a public `valid_id(&str) -> bool`,
the check its `try_` forms already apply, so the builder can test plugin ids before it builds
an instance or entry IRI.
