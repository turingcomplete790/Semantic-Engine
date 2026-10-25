# Library reader fixtures (spec 005)

## `library.json`

A small library for the reader tests. `tests/common::library_handler` serves it as a fake Stash.
The handler evaluates the reader's own queries against it:
- "key is not null" filters on `custom_fields`;
- `tags` INCLUDES;
- `measurements` NOT_NULL;
- `updated_at` GREATER_THAN;
- `ids`, which ignore any filter, as Stash does;
- newest-first `per_page: 1`.

It returns rows in descending id order, so the reader's sorting is exercised.

This replaces fixed per-query response files on purpose. Cynic generates the query text, and a
fake that interprets requests keeps working when that text changes. The item shapes follow
read-only queries against the test instance (localhost:9998, Stash v0.31.1) on 2026-10-24. The
real captured library is in `../test-instance/` (task T030).

**Contents:**
- **Tags:**
  - Kissing (`isAction`, declared variations);
  - Kissing (slow), a variant with no flag of its own;
  - Blonde (`isTrait`, `fieldMapping`, parent Hair);
  - Tall (`isLabel` stored as `true`, a formula naming `__derived_bust`);
  - Dom (`isRole` stored as `"true"`);
  - a Variation;
  - Plain and Hair (not semantic);
  - "Switched off" (`isAction: "0"`). Discovery finds it, but it is not semantic.
- **Performers:**
  - 10, an Archetype with parents, a position, and an `archetypeRecipe` that must not be returned;
  - 11, Trait-tagged, with an empty `measurements` and the retired `isGeneric` and `looks`;
  - 12, with `measurements` only;
  - 13, only referenced by scenes;
  - 14, Label-tagged, with `measurements: "natural"` and a malformed position;
  - 15, with `isArchetype: "0"` and nothing else.
- **Scenes:**
  - 20, with three semantic keys;
  - 21, with one field of invalid JSON;
  - 22, with no semantic data;
  - 23, with a relation list stored as an object.
- **Images:**
  - 30 and 31, semantic;
  - 32, with no semantic data.

## Measurements parity set

- **`measurements-input.json`:** 20 distinct real `measurements` values from the test instance
  (every 35th of the 702 distinct values among its 791 performers with measurements). Also 20
  edge cases: bust-only, half cups written three ways, DD to DDDD, an unknown cup, no cup, a zero
  band, a zero waist or hip, unparseable and empty text, extra spaces, and a decimal hip.
- **`gen-measurements.mjs`:** contains the semantic-tagging plugin's `parseMeasurements` and
  `DERIVED_PERFORMER_FIELDS`, copied verbatim from `src/ui/matrix.js` at commit `2c04141`
  (lines 218–356). It writes `measurements.json`, the plugin's own results.
- **Regenerate:** run `node gen-measurements.mjs` in this folder. It was generated with Node
  v26.8.1.

`tests/reader_measurements.rs` checks the reader's derivation against `measurements.json`
(SC-010).
