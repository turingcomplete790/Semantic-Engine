# Quickstart: validating the library reader

These checks prove spec 005 end to end. The first two need no Stash. The rest are read-only
against the test instance, except for the seed and cleanup step, which writes to the test
instance only. One last check is run by the owner against Production.

Record types are in [data-model.md](data-model.md). The interface is in
[contracts/reader-interface.md](contracts/reader-interface.md).

## Prerequisites

- **Toolchain:** the Rust toolchain of the workspace. `cargo build` works as it does for 004.
- **Test instance:** localhost:9998, Stash v0.31.1. Put its API key in `STASH_TEST_API_KEY`
  without printing it, for example:
  `export STASH_TEST_API_KEY="$(grep -i '^api key:' ../Test-Instance/creds.txt | sed 's/^[^:]*:[[:space:]]*//')"`.
- **Production check:** the owner's own Production API key. The engine never holds it.

## 1. Offline tests (SC-001, SC-006, SC-007, SC-008, SC-009)

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Expected:** everything passes, with networking unavailable. In particular:
- **`reader_full`:** the recorded test-instance library gives every semantic tag, performer,
  scene and image exactly once, and nothing else (SC-001).
- **`reader_failures`:** each of the six failure kinds comes from its recorded case, and none is
  reported as another kind (SC-006). A known secret appears in no message (SC-007).
- **`reader_stop`:** a stop requested before batch *n* sends no request for batch *n*, and the
  read returns `Stopped` (SC-009).
- **`reader_memory`:** `MemoryReader` keeps rules B1 to B9.
- **`reader_measurements`:** every case of the parity set gives the plugin's four values
  (SC-010).
- **Spec 004:** all its tests pass unchanged (SC-008).

## 2. Vocabulary 1.3.0

```sh
cargo test -p semantic-vocabulary
```

**Expected:**
- the vocabulary checks pass, and the version is 1.3.0;
- `sem:FieldMappingRule` and its four properties are present with their annotations;
- `archetypeGraphPosition` is listed under "Display data, passed through".

## 3. Capture fresh fixtures (FR-025)

```sh
STASH_URL=http://localhost:9998 STASH_API_KEY="$STASH_TEST_API_KEY" \
  cargo run -p stash-adapter --bin fixture-capture -- --out crates/stash-adapter/tests/fixtures/test-instance
```

**Expected:**
- the library counts (`findScenes`, `findImages`, `findPerformers` and `findTags` with
  `per_page: 0`) are identical before and after the capture;
- `grep -r "$STASH_TEST_API_KEY" crates/stash-adapter/tests/fixtures` finds nothing;
- the recorded `library-read.json` loads into `MemoryReader`.

## 4. Seed the test instance and time a full read (SC-002 basis, SC-003, SC-004, SC-005)

The seed adds semantic data to every scene and image of the test instance. It is reversible.

```sh
cargo run -p spike --release --bin spike-bench -- seed
cargo run -p stash-adapter --release --bin reader-bench -- full --runs 5
cargo run -p stash-adapter --release --bin reader-bench -- ids --runs 5
cargo run -p stash-adapter --release --bin reader-bench -- change --runs 5
cargo run -p spike --release --bin spike-bench -- cleanup
```

`reader-bench` only reads. It takes `STASH_URL` (default `http://localhost:9998`) and
`STASH_API_KEY`, and writes a JSON result under `specs/005-stash-adapter/results/`. The results
are committed as measurement data, and `.graphifyignore` already keeps them out of the code
graph.

**Expected:**

| Check | Figure | Target |
|---|---|---|
| `full`: median ms per media item | about 0.15–0.4 | projected to 79,000 items, under 2 minutes (SC-002) |
| `full`: peak memory above the returned records | recorded | bounded by `RECORD_BATCH` raw responses, not by library size (SC-005) |
| `full`: request count | recorded | fixed by the key table plus items ÷ 250 |
| `ids`: all four kinds | recorded | projected to Production, under 5 s (SC-003) |
| `change`: after one save | recorded | under 100 ms (SC-004) |

For `change`, the bench saves nothing itself. For each kind it takes the newest `updated_at`
and times a change read from one second earlier. That returns at least the newest item, so it is
the read the engine makes right after a single save.

## 5. Production (owner, read-only): the real SC-002 figure

```sh
STASH_URL=http://localhost:9999 STASH_API_KEY=<owner's key> \
  cargo run -p stash-adapter --release --bin reader-bench -- full --runs 3
```

**Expected:**
- the full read finishes in under 2 minutes and never takes more than 5;
- the result file records the item counts and the time.

Pass `--out` to a folder of your own to keep the result apart from the test-instance one;
otherwise it replaces `results/reader-full.json`.

The bench sends only the read operations of the contract. The owner can confirm that the
Production library's counts are unchanged afterwards.

## 6. Spec 004 still works live

```sh
cargo test -p plugin --test live_relations -- --ignored --test-threads=1
```

**Expected:** all 7 live checks pass on the test instance, as they did before this feature.

## Results

### 2026-10-25: test instance, seeded (T032)

- **Library:** Stash v0.31.1. The spike's `seed` added semantic data to all 1,021 scenes and all
  1,327 images, and created 42 tags.
- **Run:** the read included 886 semantic performers. It used release builds, 5 runs each.
- **Cleanup:** restored the library exactly: 1,021 scenes, 1,327 images, 1,107 performers and 22
  tags, before and after.
- **Raw results:** `results/reader-full.json`, `reader-ids.json` and `reader-change.json`.

| Check | Measured | Projection to Production | Target | Verdict |
|---|---|---|---|---|
| `full` | 3,293 records in 49 requests; median 830 ms, p95 854 ms; 0.25 ms per record | about 80,000 records (49,683 scenes, 29,057 images, plus performers and tags): about 20 s | SC-002: under 2 min | meets (projected; T037 measures it) |
| `ids` | 3,519 ids of all four kinds; median 103 ms (0.029 ms per id) | about 90,000 ids: about 2.6 s | SC-003: under 5 s | meets (projected) |
| `change` | median per kind: tags 1.5 ms, performers 2.1 ms, scenes 5.4 ms, images 7.5 ms | n/a | SC-004: under 100 ms | meets |
| peak memory | 27 MB resident for a read of 820 KB of records; 8 MB for the unseeded 905 records | n/a | SC-005 | see note |
| batch size | `RECORD_BATCH` 250 | n/a | n/a | kept: no tuning needed for SC-002 |

**SC-005:** peak memory grew with the records returned (905 to 3,293 records: 8 MB to 27 MB),
not with the library. See the Production result below.

### 2026-10-25: Production, read-only (T037, run by the owner)

**Library and run:**
- Stash v0.31.1 at localhost:9999.
- About 80,000 items in total (49,683 scenes, 29,057 images), of which 1,215 records are
  semantic: 49 tags, 1,156 performers, 10 scenes and 0 images.
- 3 runs, 40 requests each. Raw result: `results/reader-full-production.json`.

| Check | Measured | Target | Verdict |
|---|---|---|---|
| Full read (SC-002) | median 177 ms, p95 181 ms; 0.15 ms per record | under 2 min, never over 5 | meets |
| Peak memory (SC-005) | 8.1 MB resident for a read of 262 KB of records, the same as the test instance's 905 records (8.2 MB) | does not grow with library size beyond the records returned | meets |

**What this shows:**
- **SC-002:** Production's semantic data is still sparse, so its full read is far below the
  budget. The fully tagged case is the seeded test-instance figure above (0.25 ms per record),
  which projects to about 20 s if all 79,000 scenes and images become semantic.
- **SC-005:** a library about 34 times the test instance's size (80,000 items against 2,348) read
  in the same 8 MB. Memory follows the records returned, not the library.

Running the bench writes `results/reader-full.json` whatever the server. This run was renamed
to `reader-full-production.json`, and the seeded test-instance run was taken again (824 ms
median, 27 MB) so both are kept.

