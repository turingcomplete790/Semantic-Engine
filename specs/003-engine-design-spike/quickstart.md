# Quickstart: Engine Design Spike

This guide runs every measurement against the local test server and checks the decision record.
Commands and result ids are defined in [contracts/spike-bench.md](contracts/spike-bench.md).

Last full run: 2026-10-17 to 2026-10-20, against the Stash v0.31.1 test instance. Every step
passed; the record is [decision-record.md](decision-record.md), and the figures are in
[results/](results/).

## Prerequisites

- Stash v0.31.1 test server at `http://localhost:9998`. Its credentials are in
  `Test-Instance/creds.txt`: export them into the environment and do not paste them anywhere.
- The real plugin installed from 002 (for side-by-side comparison), and Rust stable.
- About 10 GB free disk space, for the 250,000-scene synthetic stores and their snapshots.

```sh
export STASH_TEST_URL=http://localhost:9998
export STASH_TEST_API_KEY=...        # from creds.txt
export STASH_TEST_PLUGINS_DIR=/home/bbulder/Projects/Stash-Plugins/Test-Instance/plugins
scripts/spike-install.sh "$STASH_TEST_PLUGINS_DIR"   # then reload plugins in Stash
```

**Expected**: Stash's plugin list shows "Semantic Engine Spike" next to "Semantic Engine".

Result: as expected. The plugin was removed again at the end (step 8).

## 1. Seed test data (R8)

```sh
cargo run -p spike --bin spike-bench -- seed --scenes 500 --images 500
```

**Expected**: `results/seed-manifest.json` has `state: seeded`, and the stats are unchanged
except for the created tags.

Result (2026-10-17): as expected; 1,501 items and 46 created tags were recorded in the manifest.

## 2. Build and per-call cost (FR-003, FR-011, R5)

```sh
for s in test 50k 100k 250k; do spike-bench synth --scale $s; spike-bench call-cost --scale $s; done
spike-bench build --from-stash
```

**Expected**: `process-reopen-<scale>` files, each with at least 100 samples and an
`open`/`query`/`serialise` breakdown. `process-first-call` is a separate file.

Result:
- **Call p95:** 29.7, 56.6, 87.1 and 88.7 ms at test, 50k, 100k and 250k scenes.
- **Store open:** 6.8–7.9 ms median at every size.
- **First call after a reload:** 13–19 ms.
- **Synthetic build:** 0.14 ms per scene; 250k scenes in 34.6 s.
- **Build from Stash:** 408 ms for 1,060 semantic items over 21 requests.

## 3. Concurrency (FR-004, R2)

```sh
for o in A B C; do spike-bench concurrency --option $o --scale 250k; done
```

**Expected**: each file records read outcomes during the write (ok, waited, failed) and any
partial state seen.

Result:
- **A (live store read-only):** 72% of 5,187 reads saw a partial write, and 9 failed with
  RocksDB corruption errors.
- **B (published snapshots):** 6,570 reads, none failed, none partial; p95 48.4 ms.
- **C (writer lock for every call):** 8 of 9 reads waited for the write, up to 41.5 s.

## 4. Hooks and freshness (FR-006, FR-007, R4)

```sh
spike-bench hooks                          # 12 scripted entity × client rows
spike-bench hooks --entity scene --bulk 10 && spike-bench hooks --entity scene --bulk 100 && spike-bench hooks --entity scene --bulk 1000
spike-bench catch-up
```

Then confirm by hand, one client at a time. Start the listener, then save a tag, a scene, an
image, and a performer from that client within the window:

```sh
spike-bench hooks --manual --client web-ui --seconds 120
spike-bench hooks --manual --client semantic-tagging --seconds 120
spike-bench hooks --manual --client viewer --seconds 120   # the viewer sends no saves yet
```

**Expected**: `results/hooks.json` covers all 12 pairs plus one manual row per client (SC-003).
It also has save latency with and without hooks.

Result: 27 rows. All 12 pairs are covered, with no hook missed or out of order.
- **Manual rows:** four from the web UI and one from the semantic-tagging plugin. The viewer
  sends no saves yet, so it has no manual row.
- **Hook cost:** about 5 ms per saved item. A 1,000-scene bulk edit takes 5.6 s instead of
  0.44 s.
- **`updated_at`:** every semantic save bumped it. Thresholds must use the server's local-time
  format.

## 5. Limits and snapshots (FR-008, FR-009, R6, R7)

```sh
spike-bench payload
for s in test 250k; do spike-bench snapshot --path op --scale $s; spike-bench snapshot --path asset --scale $s; done
for s in test 250k; do spike-bench delta --scale $s; done   # after the snapshot runs: it adds generations
```

**Expected**: a size curve with the 1 s failure point and a proposed page size, and snapshot
times for both paths. The asset fetch must succeed only with the Stash login.
`snapshot-delta-<S>` must show `match.matches: true`. That is, the client's copy, after it
applies the change files, equals the latest generation.

Result:
- **Payload:** proposed page size 2 MiB (p95 192 ms); the decision record uses 1 MiB. No call
  failed up to 16 MiB.
- **Snapshots at 250k:** 186 s (asset) and 211 s (op) for 74 MB. The asset GET answers 401
  without a login.
- **Change files:** `match.matches: true` at both scales.
- **Clients:** since constitution v3.0.0 clients take no snapshots, so these figures are
  reference only.

## 6. Build time and size (FR-010, R10)

Read CI job durations for the five targets and the v0.1.0 asset sizes, and time three local
clean release builds:

```sh
scripts/spike-build-cost.sh        # writes results/build-release.json; needs gh logged in
```

Result: local clean release build 234 s; CI release jobs 46–60 s per target; v0.1.0 archives
1.8–2.2 MB.

## 7. Clean up and write the record

```sh
spike-bench cleanup        # SC-005: stats and custom fields back to statsBefore
spike-bench report > /tmp/spike-summary.md
```

Write `decision-record.md` in the shape of
[contracts/decision-record.md](contracts/decision-record.md).

**Checks**:
- All seven sections are filled (SC-001).
- Rerunning `call-cost --scale test` gives a median within 25% of the recorded one (SC-002).
- `seed-manifest.json` has `state: cleaned` (SC-005).
- The owner reads the "For client maintainers" section and can answer the SC-006 questions.

Result (2026-10-19 to 2026-10-20):
- **SC-001:** all seven sections are filled.
- **SC-002:** the `call-cost --scale test` rerun median was 17.71 ms against 15.70 ms (+12.8%).
  `report` also flags `freshness-id-diff` (a cold first run; each type's median held steady)
  and `process-first-call-test` (a single observation).
- **SC-005:** `state: cleaned`. The only stats difference is one performer the owner created by
  hand after the seed.
- **SC-006:** the owner reviewed the record on 2026-10-20.

## 8. Remove the spike plugin

```sh
rm -rf "$STASH_TEST_PLUGINS_DIR/semantic-engine-spike"   # then reload plugins in Stash
git diff --exit-code api docs                              # contract 1.0.0 unchanged
```

Result (2026-10-21): removed; Stash lists only Semantic Engine and Semantic Tagging. `api/` and
`docs/` are unchanged.
