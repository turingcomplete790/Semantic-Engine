# Contract: vocabulary 1.3.0 additions

The reader reads `fieldMapping` (FR-015), so the vocabulary gains terms for it. All of them are
new, so vocabulary 1.2.0 becomes **1.3.0** (MINOR, rule V014), and no existing term changes
meaning.

Every term carries the required annotations (V005):
- `sem:source "semantic-query-engine-roadmap.md § Milestone 1D (Field Mapping); src/ui/field-mapping.js"`;
- `sem:sourceRevision "2c04141, read 2026-10-24"`;
- `sem:status sem:Active`;
- `sem:since "1.3.0"`.

Every property has exactly one domain and one range (V004), and every property has a `sem:scope`
(V006). Label rules already use the names `sem:hasField`, `sem:hasWeight` and `sem:ruleId`, so
the new properties have names of their own.

The terms go in `vocabulary/labels.ttl`, next to the Label rules they resemble.

## Classes

| Term | Subclass of | Meaning |
|---|---|---|
| `sem:FieldMappingRule` | — | One Field Mapping slot of a Trait, Label or Variation tag: a performer who carries the tag (and, for a Trait, optionally one of its declared variations) gets a native performer field set to the tag's name. The plugin applies it; the graph only describes it. |

## Properties

| Term | Domain | Range | Scope | Meaning |
|---|---|---|---|---|
| `sem:hasFieldMapping` | `sem:Tag` | `sem:FieldMappingRule` | `sem:Global` | One of the tag's Field Mapping slots. |
| `sem:mapsToField` | `sem:FieldMappingRule` | `xsd:string` | `sem:Global` | The native performer field the slot sets, for example `hair_color`. |
| `sem:mappingPriority` | `sem:FieldMappingRule` | `xsd:integer` | `sem:Global` | When several slots set the same field for one performer, the lowest number wins. |
| `sem:mappingVariation` | `sem:FieldMappingRule` | `xsd:string` | `sem:Global` | For a Trait's variation slot: the declared variation it applies to. Absent for the `base` slot. |

### Derived measurements (spec FR-002b)

The plugin's documents do not describe these. They are a deliberate addition (Principle II):
`sem:source "src/ui/matrix.js parseMeasurements, DERIVED_PERFORMER_FIELDS (not in the design documents)"`,
`sem:sourceRevision "2c04141, read 2026-10-24"`, `sem:status sem:Derived`.

| Term | Domain | Range | Scope | Plugin field name | Meaning |
|---|---|---|---|---|---|
| `sem:bustSize` | `sem:Performer` | `xsd:decimal` | `sem:Global` | `__derived_bust` | Bust Size from Measurements: band, plus the cup step on the half-cup scale times 2.5, plus 2 per inch of waist under 28. Not literal inches. |
| `sem:waist` | `sem:Performer` | `xsd:integer` | `sem:Global` | `__derived_waist` | Waist from Measurements. |
| `sem:hip` | `sem:Performer` | `xsd:integer` | `sem:Global` | `__derived_hip` | Hip from Measurements. |
| `sem:waistToHipRatio` | `sem:Performer` | `xsd:decimal` | `sem:Global` | `__derived_whr` | Waist divided by hip, rounded to 2 places. |

Each property records its plugin field name with a new annotation `sem:pluginFieldName`, so a
`sem:FieldRule` whose `sem:hasField` is `__derived_bust` resolves to `sem:bustSize`.
`sem:pluginFieldName` is an annotation property: one domain (`rdf:Property`), range `xsd:string`.

## Mapping (`stash-mapping.ttl`)

| Term | From entity | From field | Allowed values | Produces |
|---|---|---|---|---|
| `sem:map_measurements` | Performer | `measurements` | free text such as `34D-26-35`; see spec FR-002b for the derivation | `sem:bustSize` |
| `sem:map_fieldMapping` | Tag (Trait, Label, Variation) | `custom_fields.fieldMapping` | JSON `{base?: {field, priority}, variations?: {"<variation>": {field, priority}}}`; Labels and Variations use `base` only | `sem:FieldMappingRule` |

## Item IRIs

- **Field Mapping slot:** `{tag}/fieldMapping:base` for the base slot, and
  `{tag}/fieldMapping:variation:{n}` for a variation slot. `n` is the variation's position in the
  sorted variation names, because variation names are free text and not valid IRI ids. It is
  built by a new `ItemIris::field_mapping(tag, slot)`.

## Change log (`vocabulary/CHANGELOG.md`)

The `1.3.0` entry:
- **Additions:** lists the terms above, citing plugin commit `2c04141`.
- **Deliberate additions (Principle II):** the derived measurement properties, which exist in the
  plugin's code but not its documents, with the formula and constants. The mapping's
  `rdfs:comment` notes that it also produces `sem:waist`, `sem:hip` and `sem:waistToHipRatio`.
- **Display data, passed through:** a new list. It moves `archetypeGraphPosition` there from
  "Excluded on purpose". The engine passes it to clients next to the Archetype hierarchy, and it
  is never a graph term (spec 005 FR-002a).
- **Excluded on purpose:** `archetypeRecipe` stays there.

`TERMS.md` and `released/1.3.0.json` are regenerated with the vocabulary tool. The `status`
operation then reports vocabulary 1.3.0.
