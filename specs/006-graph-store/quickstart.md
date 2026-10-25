# Quickstart: validating the graph store

These checks prove spec 006 end to end. Everything here runs without Stash; the store is a local
folder. The interface is in [contracts/graph-store-interface.md](contracts/graph-store-interface.md).

## Prerequisites

- The workspace's Rust toolchain (1.89 or newer for `File::try_lock`), with the bundled RocksDB
  build tools (clang and a C++17 compiler, as for every build since constitution v2.0.0).
- About 5 GB free disk for the 250,000-scene measurement (8.3 KB per scene, up to 3 generations).

## 1. Offline tests (SC-001, SC-004, SC-005, SC-006, SC-008)

```sh
cargo test -p engine
cargo clippy --workspace --all-targets -- -D warnings
```

**Expected:** all pass. In particular:

| Test | What it checks |
|---|---|
| `graph_write_publish` | publish, the busy writer, a crash before publish, all-or-nothing changes |
| `graph_read` | SELECT answers, a reader keeping its generation, invalid queries with positions, "not built yet" |
| `graph_health` | the four rebuild reasons, clearing, cleanup that keeps current, previous and recent generations |
| `graph_summary` | every field, and `writing` while a writer holds the lock |
| `graph_concurrency` | 4 reader processes and a writer publishing 50 generations: 0 partial reads out of at least 5,000 (SC-001) |
| `graph_interface` | no `oxigraph` type in the module's public items (G9) |

On Windows and macOS, CI's `platforms` job runs the same suite (SC-008).

## 2. Budgets at 250,000 scenes (SC-002, SC-003, SC-007)

```sh
D=/tmp/se-graph-250k
cargo run -p engine --release --bin graph-bench -- build --scenes 250000 --dir $D
cargo run -p engine --release --bin graph-bench -- open  --dir $D --runs 50
cargo run -p engine --release --bin graph-bench -- query --dir $D --runs 50
cargo run -p engine --release --bin graph-bench -- disk  --dir $D
cargo run -p engine --release --bin graph-bench -- stress --dir $D --readers 4 --seconds 60
```

The same commands with `--scenes 1000` give the test-library figures.

**Expected**, with results written to `specs/006-graph-store/results/`:

| Command | Target |
|---|---|
| `open` | median under 15 ms (SC-002; the spike measured 7–8 ms) |
| `query` | p95 under 200 ms for a whole call, including the open (SC-003) |
| `disk` | the whole store, after cleanup, at most 3 times one generation (SC-007); about 8.3 KB per scene per generation |
| `stress` | 0 partial reads and 0 errors (SC-001 at scale) |

## 3. Build time

`graph-bench build` reports its own time, for reference against the next feature's first-build
budget: 5 minutes at Production size (003 decision record). Nothing in this feature is judged
against it.

## 4. The package stays clean

```sh
scripts/package.sh --out /tmp/se-dist
tar -tzf /tmp/se-dist/semantic-engine-*.tar.gz | grep -E 'graph-bench|spike' && echo "FAIL" || echo "ok"
```

**Expected:** `ok`. Only the plugin binary is packaged.

## Results

### 2026-10-25, Linux, release build, on a disk-backed folder (T023)

The synthetic graph has the spike's shape, built through `engine::graph`. Raw results are in
`results/graph-*-1000.json` and `results/graph-*-250000.json`. The first 1,000-scene run, on a
RAM-backed `/tmp`, was discarded so that timings reflect a real disk.

| Check | 1,000 scenes (39,257 facts) | 250,000 scenes (9,794,566 facts) | Target | Verdict |
|---|---|---|---|---|
| Open the current generation (SC-002) | median 5.5 ms, p95 7.9 ms | median 6.3 ms, p95 10.2 ms | median under 15 ms | meets |
| Whole query call: open plus `SELECT` (SC-003) | p95 30.2 ms over all queries | p95 71.7 ms over all queries; the slowest query (role and action in one scene) has p95 79.6 ms | p95 under 200 ms | meets |
| Disk after cleanup (SC-007) | store 1.72 times one generation | store 1.73 times one generation (2.50 GB, against a 1.45 GB generation); 5.8 KB per scene | at most 3 times | meets |
| Stress, 4 reader processes (SC-001) | 7,969 reads, 0 partial, 0 errors; 85 publishes | 16,492 reads, 0 partial, 0 errors; 14 publishes | 0 partial | meets |
| Build, for reference | 0.9 s | 77 s (load 73.7 s, publish 3.3 s); peak memory 1.7 GB | — | — |
| Publish, for reference | median 51 ms | median 3.8 s (3.7–10.4 s) | — | see below |

**Comparison with the spike:** the spike measured a 7–8 ms open and an 88.7 ms query p95 at
250k, with the same layout. The store here is in line with both.

**Publish time at 250k:** a publish takes about 3.8 s. It has not yet been split into its
parts. The likely main cost is the fact count, which reads the whole store at each publish
(research R6), but that is unmeasured. It does not affect any criterion of this feature. For
incremental updates (the next feature), where each apply ends in a publish, it should be
measured, and the count kept incrementally if it dominates.

