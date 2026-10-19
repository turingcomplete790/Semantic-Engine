# Quickstart: Engine Design Spike

This guide runs every measurement against the local test server and checks the decision record.
Commands and result ids are defined in [contracts/spike-bench.md](contracts/spike-bench.md).

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

## 1. Seed test data (R8)

```sh
cargo run -p spike --bin spike-bench -- seed --scenes 500 --images 500
```

**Expected**: `results/seed-manifest.json` has `state: seeded`, and the stats are unchanged
except for the created tags.

## 2. Build and per-call cost (FR-003, FR-011, R5)

```sh
for s in test 50k 100k 250k; do spike-bench synth --scale $s; spike-bench call-cost --scale $s; done
spike-bench build --from-stash
```

**Expected**: `process-reopen-<scale>` files, each with at least 100 samples and an
`open`/`query`/`serialise` breakdown. `process-first-call` is a separate file.

## 3. Concurrency (FR-004, R2)

```sh
for o in A B C; do spike-bench concurrency --option $o --scale 250k; done
```

**Expected**: each file records read outcomes during the write (ok, waited, failed) and any
partial state seen.

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

## 5. Limits and snapshots (FR-008, FR-009, R6, R7)

```sh
spike-bench payload
for s in test 250k; do spike-bench snapshot --path op --scale $s; spike-bench snapshot --path asset --scale $s; done
```

**Expected**: a size curve with the 1 s failure point and a proposed page size, and snapshot
times for both paths. The asset fetch must succeed only with the Stash login.

## 6. Build time and size (FR-010, R10)

Read CI job durations for the five targets and the v0.1.0 asset sizes, and time one local clean
release build:

```sh
cargo clean -p plugin && time cargo build --release -p plugin
```

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
