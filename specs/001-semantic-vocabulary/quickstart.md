# Quickstart: Validate the Semantic Vocabulary

Prerequisites: Rust stable toolchain. No network, no Stash server needed.

## 1. Run the check

```text
cargo run -p semantic-vocabulary --bin vocab-check
```

Expected: exit code 0 and `all rules pass`. See
[contracts/vocabulary-check.md](contracts/vocabulary-check.md) for the rules (V001 to V013).

## 2. Run the tests

```text
cargo test -p semantic-vocabulary
```

Expected: parse, defined-terms, domain/range, provenance, IRI scheme, and mapping tests all
pass.

## 3. Regenerate the term list

```text
cargo run -p semantic-vocabulary --bin vocab-check -- --write-terms
git diff vocabulary/TERMS.md
```

Expected: no diff when the vocabulary is unchanged.

## 4. Spot-check scoping (SC-004)

Open `vocabulary/TERMS.md` and answer, using only that file: is a Role global or
scene-scoped? Which tag types carry categories? What replaces `hasLook`? Expected answers:
scene-scoped; Role, Trait, Mise-en-scène; `hasPerformerVariation`.

## 5. Spot-check identity (SC-006)

```text
cargo test -p semantic-vocabulary iri_scheme
```

Expected: the golden test builds item IRIs twice from the same fixture and finds them
identical.

## 6. Prove the mapping covers real data (SC-005)

```text
cargo test -p semantic-vocabulary mapping
```

Expected: every flag and `custom_fields` key present in the captured fixture library has a
mapping entry, and none is reported as unmapped.
