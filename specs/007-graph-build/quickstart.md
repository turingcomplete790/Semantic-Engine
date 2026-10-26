# Quickstart: validating the graph build

These checks prove spec 007 end to end. Sections 1 and 2 need no Stash. The rules are in
[contracts/mapping-rules.md](contracts/mapping-rules.md), and the task is in
[contracts/operations-1.2.md](contracts/operations-1.2.md).

## Prerequisites

- The workspace's Rust toolchain, as for spec 006.
- For section 3: the test instance at `localhost:9998`, with its API key in the environment
  (read from `Test-Instance/creds.txt`, never printed). The plugin must be installed there, with
  a server id generated.
- For section 4: the owner, with read access to Production.

## 1. Offline tests (SC-001, SC-002, SC-003, SC-005, SC-006, SC-007)

```sh
cargo test -p engine
cargo test -p plugin
cargo test -p semantic-vocabulary
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p semantic-vocabulary --bin vocab-check
```

**Expected:** all pass. In particular:

| Test | What it checks |
|---|---|
| `build_fixture` | the fixture library's graph equals `expected.nt`: 0 missing, 0 extra (SC-001) |
| `build_coverage` | every Active mapping entry has a fixture fact (SC-002) |
| `build_test_instance` | counts, complete instances, only defined terms (SC-003) |
| `build_notes` | each bad field noted once; the rest of each record mapped (SC-006) |
| `build_determinism` | the same facts twice, and after shuffling record order (SC-007) |
| `build_run` | first build, rebuild after a change, busy, cleared store, no server id, reader failure (US2) |
| `build_kill` | 20 builds killed at varied points: the previous generation stays current every time (SC-005) |
| `vocab-check` | vocabulary 1.4.0 is consistent, and its snapshot is written |
| plugin `api_agreement` | `buildGraph` and `BUSY` agree across the plugin, `api/operations.json` and `docs/api.md` |

**A failure must name the cause.** Change one rule on purpose, for example by dropping
`sem:hasVariation` from action instances. `build_fixture` must then fail, listing the missing
lines, and `build_coverage` must still pass.

## 2. Synthetic Production size (SC-004, FR-023)

```sh
D=/tmp/se-build-synth
cargo run -p engine --release --bin build-bench -- synth --scenes 50000 --images 29000 --dir $D
```

**Expected:** the build ends in under 5 minutes, with peak memory under 2 GB. The JSON report
goes to `specs/007-graph-build/results/build-synth.json`.

## 3. The test instance, from Stash (US2, SC-008)

1. Install the plugin build (`scripts/package.sh` and the install script of spec 002).
2. In Stash: Settings → Tasks → Semantic Engine → **Build semantic graph**.
3. **Check:**
   - the job's progress moves to 100%;
   - the log ends with the summary line;
   - `graph/current` exists under the plugin folder.
4. Run the build again while it is still running. The second job ends at once with `BUSY`.
5. Start a build and press Stop in the job list. `graph/current` still names the earlier
   generation.
6. **SC-008:** query the published generation with a development query (the `graph-bench`
   `query` command, pointed at the plugin's `graph` folder) for "scenes where one performer has
   Role 19 and another has Action 21". Compare with the scenes whose `roleTagAssignments` and
   `actionAssignments` say so, read from Stash with `querySQL`.

## 4. Production, read-only, run by the owner (SC-004)

```sh
STASH_URL=… STASH_API_KEY=… cargo run -p engine --release --bin build-bench -- stash --dir /tmp/se-build-prod
```

**Expected:** under 5 minutes. The report goes to
`specs/007-graph-build/results/build-production.json`.

The store is written to a local folder, and Production is only read.

## 5. The package stays clean

```sh
scripts/package.sh --out /tmp/se-dist
tar -tzf /tmp/se-dist/semantic-engine-*.tar.gz | grep -E 'build-bench|graph-bench|spike' && echo "FAIL" || echo "ok"
```

**Expected:** `ok`.

## Results

### 2026-10-26, Linux, release build

**Synthetic Production size** (T039; `results/build-synth.json`), on a disk-backed folder:

| Measure | Result | Target | Verdict |
|---|---|---|---|
| Library | 50,000 scenes, 29,000 images, 1,000 performers, 48 tags | Production size | — |
| Facts | 4,293,495 (store 973 MB) | — | — |
| Whole build | 39.9 s (read 0.9 s, map 4.1 s, write 26.5 s, publish 1.3 s) | under 5 minutes (SC-004) | meets |
| Peak memory | 1.61 GB, of which 0.95 GB is the library itself in memory | under 2 GB | meets |

**About the memory figure:**
- **The 2.56 GB first measurement was a bench artifact.** `MemoryReader::full_read` returns a
  copy of its library, so the first run held two. `build-bench` now uses a reader that hands its
  library over once, as a read from Stash does.
- **Batch size barely matters:** 10,000-fact batches gave 2.47 GB against 2.57 GB, with the copy
  included, and a slower write. 50,000 stays.

**Test instance, from Stash** (T040; `localhost:9998`, Stash v0.31.1):

| Check | Outcome |
|---|---|
| Task from Stash's job queue | job finished, progress 100%. Summary: `build: generation 1 published; 905 records (17 tags, 886 performers, 1 scenes, 1 images); 5985 facts; skipped 1 (danglingRef 1); mismatches 1 (performerNotInMedia 1); 0.5 s` |
| Notes | the two known problems of image 15, as the `build_test_instance` test pins them |
| A second build | **Stash's job queue runs one job at a time**, so a second task waits and then succeeds as the next generation. The `buildGraph` operation, called while the task ran, returned `BUSY`, and nothing changed |
| Stop during a build | 3 of 3 jobs `CANCELLED`; `current` unchanged; `work.dirty` left, as designed. The next build reset the working store and published generation 4 |
| SC-008: scenes where one performer has Role 19 and another has Action 21 | the graph answers `scene:2`. The plugin's own data, read with `querySQL` and evaluated in Python, also says `scene:2`. Meets |

**Production** (T041; `results/build-production.json`), run by the owner, read-only, into a
local folder:

| Measure | Result | Target | Verdict |
|---|---|---|---|
| Records | 1,220 (49 tags, 1,161 performers, 10 scenes, 0 images) | — | — |
| Facts | 7,460 | — | — |
| Whole build | 0.35 s (read 201 ms, map 5 ms, write 63 ms, publish 4 ms) | under 5 minutes (SC-004) | meets |
| Peak memory | 377 MB | under 2 GB | meets |
| Notes | 1 skipped (`invalidId`), no mismatches | — | — |

Production is mostly untagged so far, so the synthetic library above is the budget's real test.
The one `invalidId` note's location was not printed, because `build-bench` then hid note lines.
It now prints them (they hold no values), so a rerun shows which item and key it is.
