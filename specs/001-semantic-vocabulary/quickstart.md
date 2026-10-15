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

## 7. Release procedure (FR-008)

```text
# bump `sem:version` in vocabulary/core.ttl and add the entry to vocabulary/CHANGELOG.md
cargo run -p semantic-vocabulary --bin vocab-check -- --write-snapshot
cargo run -p semantic-vocabulary --bin vocab-check
```

Expected: the snapshot `vocabulary/released/<version>.json` is written and the check passes.
On the first release there is no earlier snapshot, so the compatibility rule (V014) passes with
a notice. Afterwards, removing or renaming a term without a higher MAJOR makes V014 fail.

## 8. Server identifier

The vocabulary tests use the fixed example UUID `00000000-0000-4000-8000-000000000001` for the
`{server}` part of item IRIs. Generating and storing the real identifier is a task of the engine
plugin (see `ROADMAP.md`, Phase 0); a client that cannot find it reports that it has not been
generated.
