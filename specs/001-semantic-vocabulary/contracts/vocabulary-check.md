# Contract: Vocabulary Check

Version of this contract: 1.0.0

## Command

```text
vocab-check [--dir <vocabulary-dir>] [--format text|json] [--write-terms]
```

- `--dir`: directory holding the `.ttl` files (default `vocabulary/`).
- `--format`: `text` (default) or `json`.
- `--write-terms`: regenerate `TERMS.md` instead of only checking it is current.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | All rules pass |
| 1 | One or more rule violations |
| 2 | Input could not be read or parsed (syntax error, missing file) |

## Rules

| ID | Rule | Spec |
|---|---|---|
| V001 | Every file parses as Turtle with no error or warning | FR-001, SC-002 |
| V002 | Only the `sem:`, `rdf:`, `rdfs:`, `xsd:` namespaces are used; no `owl:` term | FR-013 |
| V003 | Every term referenced is defined in the loaded files | FR-010 |
| V004 | Every property has exactly one domain and one range | FR-003, FR-010 |
| V005 | Every term has the required annotations, each with the stated cardinality | FR-005 |
| V006 | Every property and assignment class has a `sem:scope` | FR-004 |
| V007 | Every deprecated term has `sem:replacedBy` or `sem:retiredNote`; the target is defined and active | FR-006 |
| V008 | Every planned term's comment begins "Planned:" | FR-006 |
| V009 | The version is declared exactly once and matches the top entry of `CHANGELOG.md` | FR-008 |
| V010 | Every mapping names a defined term and an allowed-value list | FR-007 |
| V011 | `TERMS.md` matches the generated output | FR-011 |
| V012 | No term is defined in more than one file | contracts/vocabulary-files.md |
| V013 | No class is defined that duplicates a Stash native entity (Scene, Image, Gallery, Performer, Tag, Studio, Group, Marker as Stash types); the semantic `Scene`, `Image`, `Performer`, and `Tag` classes carry `sem:references` naming the Stash entity instead of redefining it | FR-009 |

## JSON output

```json
{
  "contractVersion": "1.0.0",
  "vocabularyVersion": "1.0.0",
  "ok": false,
  "violations": [
    {"rule": "V005", "term": "sem:hasRole", "file": "core.ttl", "line": 123,
     "message": "missing sem:sourceRevision"}
  ]
}
```

## Library surface

```text
semantic_vocabulary::load(dir) -> Vocabulary
semantic_vocabulary::embedded() -> Vocabulary      // files embedded at build time
Vocabulary::terms() / term(iri) / properties_for_domain(class)
Vocabulary::check() -> Vec<Violation>
semantic_vocabulary::iri::{scene, image, performer, tag, marker, instance, ...}
```

The library surface is versioned with the contract; a breaking change to it bumps the crate's
MAJOR version.
