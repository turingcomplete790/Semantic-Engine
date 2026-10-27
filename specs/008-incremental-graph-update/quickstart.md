# Quickstart: validating the incremental update

These checks prove spec 008 end to end. Sections 1 to 3 need no Stash.

## Prerequisites

- The workspace's Rust toolchain, as for specs 006 and 007.
- For section 4: the test instance at `localhost:9998`, with the plugin installed and a server
  id generated. Credentials are read into environment variables only.

## 1. Offline tests (SC-001, SC-004, SC-005, SC-006)

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Expected:** all pass. In particular:

| Test | What it checks |
|---|---|
| `graph_owned` (engine) | `add_owned`, `replace_owned` (including the no-op case), `remove_owned`, `owners`, `owners_referencing`; the kept count equals `Store::len` (G10–G13) |
| `graph_read` (engine) | queries over the union of graphs give 006's answers; a format-1 store needs a rebuild |
| `reader_semantic` (adapter) | the semantic rule matches `full_read`'s filtering; `semantic_tags` and `performers_with_tags` on the fake Stash and `MemoryReader` |
| `update_equivalence` | at least 20 change sequences, each updated graph equal to a full rebuild (SC-001) |
| `update_fallback` | first build, damaged store, mapping and vocabulary versions, no catch-up point, too many changes: each rebuilds and names its reason (SC-005) |
| `update_current` | no change, and a save that changes no fact: nothing published (FR-008) |
| `update_reads` | the counting reader: no record reads without changes; record reads follow the changes and their dependents (SC-006) |
| `update_kill` | killed updates leave a whole generation, and the next update matches a full rebuild (SC-004) |
| `owners_disjoint` | no fact belongs to two owners, over the fixture and the synthetic library |
| plugin `ops_update`, `api_agreement` | the `updateGraph` contract |

## 2. Budgets at 250,000 scenes (SC-002, SC-003)

```sh
D=$HOME/.cache/se-update-250k
for k in 0 1 100 1000; do
  cargo run -p engine --release --bin build-bench -- update --scenes 250000 --images 325000 --change $k --dir $D
done
cargo run -p engine --release --bin graph-bench -- build --scenes 250000 --dir $HOME/.cache/se-graph-250k
cargo run -p engine --release --bin graph-bench -- query --dir $HOME/.cache/se-graph-250k --runs 50
```

**Expected**, with results in `specs/008-incremental-graph-update/results/`:

| Run | Target |
|---|---|
| `--change 0` | nothing published; under 2 s (at Production size this is measured too) |
| `--change 1` | under 5 s end to end; publish around 40 ms (research R2) |
| `--change 1000` | under 60 s |
| `graph-bench query` on format 2 | p95 under 200 ms (006 SC-003, kept) |

## 3. Discovery bounds (FR-020)

`build-bench stash` (spec 007) on the test instance, and the owner's Production run, report the
request count. Record:
- the requests per semantic item;
- the batch sizes;
- the times.

The roadmap's discovery item cites them.

## 4. The test instance, from Stash (SC-007)

1. Install the plugin build. The store is format 1, so the first "Update semantic graph" rebuilds
   in full, with reason `storeNeedsRebuild` (`otherFormat`).
2. In Stash, edit:
   - one scene's role assignments;
   - one tag's categories;
   - one performer's tags.
3. Run "Update semantic graph". The log shows the three changes, with the performer counted as
   changed.
4. Run "Build semantic graph" into a copy of the plugin folder, or compare query answers: every
   fact of the updated generation equals the rebuilt one.
5. Run "Update semantic graph" again. The log says the graph is current, and no generation is
   published.

## Results

### 2026-10-26, Linux, release build, disk-backed folders

**Budgets** (T025, T026; `results/update-*.json`, `results/graph-query-250000.json`). The
synthetic library is the fully tagged one of spec 007, read through `MemoryReader`:

| Run | Update | Reads | Writes | Publish | Target | Verdict |
|---|---|---|---|---|---|---|
| 250,000 scenes, 325,000 images (24.2 million facts), 0 changes | 5.4 s, nothing published | 4.0 s | — | — | — (SC-003 is at Production size) | — |
| 250k, 1 changed scene | 4.6 s | 3.2 s | 12 ms | 0.6 s | under 5 s (SC-002) | meets |
| 250k, 100 changed scenes | 8.1 s | 4.1 s | 1.7 s | 1.5 s | — | — |
| 250k, 1,000 changed scenes | 17.9 s | 4.9 s | 11.0 s | 1.1 s | under 60 s (SC-002) | meets |
| Production size (50,000 scenes, 29,000 images), 0 changes | 1.4 s, nothing published | 0.7 s | — | — | under 2 s (SC-003) | meets |
| Production size, 1 changed scene | 2.4 s | 0.5 s | 6 ms | 1.5 s | — | — |
| `graph-bench query` on format 2, 250k | p95 81.8 ms over all queries (slowest: role and action in one scene, p95 88.2 ms) | | | | under 200 ms (006 SC-003) | meets |

**What the times are made of:**
- **Reads** are the fixed cost of an update. They cover the change queries, Stash's id lists,
  and listing the store's owner graphs (575,000 at 250k).
- **Publish** is 0.6–1.5 s, not the ~40 ms estimated from 006's parts. Flush and checkpoint
  grow after many writes. It is still well within the budgets.
- **Peak memory of 16.8 GB** is the bench's own: it holds the synthetic library twice (building
  and updating). It is not the update's own cost.
- **The fallback share** (a third, with a floor of 500) was not changed. 1,000 changes at 250k
  took 17.9 s against a full build of about 16 minutes, so an update stays far cheaper well past
  1,000 changes.

**Full builds on format 2** (`results/build-synth-format2.json`): spec 007's synthetic
library of Production size now builds in **107.3 s** (40 s on format 1), with a peak of 1.55 GB.
That meets 007's budget of under 5 minutes.
- **The cost:** the named graphs' extra indexes, paid once per full build. Writing is 101 s of
  the 107.
- **At 250k:** the same build took 13 to 20 minutes in the update bench (spec 006's
  default-graph build took 77 s). There is no 250k full-build budget, but this is the first thing
  to look at if one is set: for example, the bulk loader's batch size, or one bulk load for the
  whole build.

**Test instance, from Stash** (T027; `localhost:9998`, Stash v0.31.1):

| Step | Outcome |
|---|---|
| First update after installing | rebuilt, reason `firstBuild`. The install script replaces the plugin folder, which removes the old format-1 store with it; `otherFormat` is covered by the offline fallback test |
| Second update, before the fix of research R13 | rebuilt with `tooManyChanges` (598 against 500): seeded items crowd each kind's newest second. Fixed by counting only changes after that second |
| Edit scene 2's roles, tag 19's categories and performer 7's tags; update | `update`, generation 2. The tag counted as changed. The scene and performer already held those values from the run before the fix, so their facts were unchanged, and nothing was written for them |
| Compare with "Build semantic graph" | every fact equal: 5,990 and 5,990 (SC-007, meets) |
| Update again | `current`; nothing published |
| Restore the three fields; update | `update`: 1 scene, 1 performer and 1 tag changed, and 90 performers re-checked as dependents of tag 19. Fields read back as before |

**Discovery bounds** (T028, FR-020; `results/discovery-test-instance.json`):

| Library | Semantic records | Requests | Requests per record | Full read |
|---|---|---|---|---|
| Test instance | 905 | 40 | 0.044 | 2.7 s |
| Production (spec 007, the owner's run) | 1,220 | not recorded then | — | 0.2 s |

- **Discovery** is one filtered query per key: 9 tag keys, 3 performer queries plus one per 500
  performer tags, and 8 keys per media kind. Its count does not depend on the library's size.
- **Records** are then read by id, 250 per request.
- **A Production rerun** of `build-bench stash` would add its request count. It is optional,
  since the formula above gives it.
