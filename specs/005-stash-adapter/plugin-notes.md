# Notes for the semantic-tagging plugin (spec 005)

Constitution Principle II makes the plugin's design documents the contract for the semantic data.
Spec 005 found two things at commit `2c04141` that the plugin's code uses but its documents do
not describe. The engine has added both deliberately, so that it can resolve them. This note
lists them so the documents can describe them too.

Nothing changes for the plugin's data. The engine only reads these fields.

## Derived measurement fields

**In the code:** `src/ui/matrix.js` (`parseMeasurements`, `DERIVED_PERFORMER_FIELDS`). Four
values are derived from a performer's `measurements` text and can be named by Label field rules
and the Matrix's advanced filters:

| Plugin field name | Engine term (vocabulary 1.3.0) | Value |
|---|---|---|
| `__derived_bust` | `sem:bustSize` | band, plus the cup's step × 2.5, plus 2 per inch of waist under 28 |
| `__derived_waist` | `sem:waist` | the second number |
| `__derived_hip` | `sem:hip` | the third number |
| `__derived_whr` | `sem:waistToHipRatio` | waist ÷ hip, rounded to 2 places |

**Constants the engine copies:**
- **Cup steps:** AA 0, A 1, A½ 2, B 3, B½ 4, C 5, C½ 6, D 7, D½ 8, E 9, E½ 10, F 11, G 12,
  H 13, I 14, J 15, K 16, L 17.
- **Cup synonyms:** DD is E, DDD is F, and DDDD is G.
- **Bust Size:** cup multiplier 2.5; the waist bonus is 2 per inch below a 28-inch waist.
- **Half cup:** written `½`, `.5` or `1/2`.

**Two edge behaviours the engine copies, found by the parity set** (`crates/stash-adapter/tests/fixtures/reader/measurements.json`):
- **A waist of 0 counts as given:** `34D-0-34` gets a waist bonus of (28 − 0) × 2 = 56, giving a
  Bust Size of 107.5. The ratio is skipped, because the code tests `waist && hip`, while the
  bonus tests `waist !== null`.
- **Trailing text is ignored:** `34D-25-35.5` reads as hip 35.

If either is unintended, the documents should say what is meant, and the engine will follow in a
vocabulary update.

**Missing from the documents:** none of this is in `README.md`,
`semantic-query-engine-roadmap.md` or `archetype-system-plan.md`. The code calls the constants
"a starting point, not a settled value".

**Request:** describe the four fields, the parse rule and the constants in the design documents
(for example, next to Label field rules in the roadmap's vocabulary section). Record any later
change to the constants there. The engine follows the documents, and its parity tests compare
with `parseMeasurements` at the cited commit.

**Under constitution v3.0.0:** once the engine serves Label evaluation and search, the plugin no
longer needs `resolveDerivedFieldMatches`. That function downloads every performer's
measurements into the browser.

## Field Mapping

**In the code:** `src/ui/field-mapping.js`, the `fieldMapping` tag key
`{ base?: {field, priority}, variations?: {"<variation>": {field, priority}} }`. The roadmap's
Milestone 1D mentions Field Mapping, but not the key or its shape.

**Engine term:** `sem:FieldMappingRule` (vocabulary 1.3.0).

**Request:** document the key and its shape with the other tag `custom_fields` keys.
