# Quickstart: validating the query operations

These checks prove spec 009 end to end. Sections 1 and 2 need no Stash.

## Prerequisites

- The workspace's Rust toolchain, as for specs 006 to 008.
- For sections 3 and 4: the test instance at `localhost:9998`, with the plugin installed, a
  server id generated, and the graph built. Credentials are read into environment variables
  only, never printed. Writes go only to the test instance.

## 1. Offline tests (SC-001, SC-002, SC-003, SC-005, SC-009)

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Expected:** all pass. In particular:

| Test | What it checks |
|---|---|
| `query_parse` (engine) | the grammar; each refused construct named with its position; reserved variables |
| `query_names` | built-in prefixes; tags by name (none, one, several); Planned and Derived terms refused; missing items |
| `query_fixture` | at least 30 known-answer queries on the compatibility fixture (SC-001) |
| `query_scope` | the scope fixture: short form equals graph form, and no Role is matched from another scene (SC-002); the ambiguous-scope refusal |
| `query_recursive` | 004 SC-004: Full Service recursively and not; Customer and Patient; a loop |
| `query_paging` | 30,000 bindings in pages: each within 1 MiB, every binding once, and a generation published between pages; expired and altered continuations (SC-005) |
| `hierarchy_links` | `Hierarchy::from_links` equals `Hierarchy::build` on 004's fixtures, loops included |
| plugin `ops_query` | the envelope, arguments, every error code, `CONTRACT_MISMATCH`, the 10 s limit (with a test-only limit) |
| plugin `ops_status` | the `graph` object in each state (SC-009) |
| plugin `ops_record_change`, `journal` | each hook type; the graph marker; one job per burst; restored marker after `BUSY` |
| plugin `api_agreement` | `docs/api.md`, `api/operations.json` and the manifest agree at 1.4.0 |

## 2. Query budget at 250,000 scenes (SC-004)

```sh
D=$HOME/.cache/se-query-250k
cargo build --release -p plugin
cargo run -p plugin --release --bin query-bench -- build --scenes 250000 --dir $D
cargo run -p plugin --release --bin query-bench -- run --dir $D --runs 50
```

`build` writes the synthetic library with a small Role and Relation hierarchy added, and the
generation's query index, as every build does (research R12). **Expected,** with results in
`specs/009-query-operations/results/query-250k.json`. Every call runs the plugin binary, as Stash
does:

| Query set | Target |
|---|---|
| plain, joins of two and three performers, filters | p95 under 200 ms |
| recursive Role and Relation | p95 under 200 ms (004 SC-007) |
| a full 1 MiB page | under 200 ms |

## 3. Hook cost on the test instance (SC-006)

1. Install the plugin build with `scripts/install.sh`, as the earlier specs did.
2. Run `spike-bench hooks` with the plugin installed, single and `--bulk 1000`.
3. Disable the plugin, and run them again.

**Expected:** at most 7 ms more per saved item with the plugin. Results go in
`results/hooks-*.json`.

## 4. Freshness and equivalence on the test instance (SC-007, SC-008)

1. Run `query-bench fresh --url http://localhost:9998`. It changes one scene's Roles, calls
   `query` every 0.5 s until the change shows, and restores the scene.
   **Expected:** under 30 s, with the job queue idle.
2. In Stash, by hand:
   - edit a tag's flags;
   - edit a performer's tags;
   - bulk-edit 100 scenes;
   - delete a scene;
   - merge two test tags.

   Do not run any task.
3. Wait for the queue to empty. Check that `status` reports `pending: false`.
4. Compare with a rebuild: copy the plugin folder, run "Build semantic graph" on it, and compare
   every fact of the two generations (as spec 008 T027 did).
   **Expected:** every fact equal.

## Results

### §2 Query budget at 250,000 scenes (2026-10-28)

The library: 250,000 scenes, 325,000 images, 5,000 performers and 48 tags; 24,311,499 facts.
The criteria are deliberately broad: one Role tag matches about a sixth of the library. Each
call is a fresh plugin process, as Stash runs it. These are 50 runs per query after one warm-up
(`results/query-250k.json`).

| Query | Oxigraph (first T042 run) | Query index (R12): median | p95 | Bindings in the first page |
|---|---|---|---|---|
| plain: a Role in a scene | over 10 s, stopped | 115 ms | 127 ms | 8,122 (1 MiB) |
| two performers, `!=` | over 10 s | 113 ms | 127 ms | 2,704 (1 MiB) |
| three performers (empty answer) | over 10 s | 366 ms | 385 ms | 0 |
| Trait and Role | over 10 s | 142 ms | 153 ms | 4,051 (1 MiB) |
| time comparison on one performer's scenes | 142 ms | 18 ms | 21 ms | 100 |
| recursive Role | over 10 s | 118 ms | 125 ms | 8,122 (1 MiB) |
| recursive Relation | 1,611 ms | 143 ms | 158 ms | 8,027 (1 MiB) |
| every scene, first 1 MiB page | 2,592 ms | 112 ms | 124 ms | 8,474 (1 MiB) |

**Verdict:**
- **SC-004, the 1 MiB page:** met for every query with an answer (124–158 ms p95). Recursive
  Role and Relation are met too (004 SC-007).
- **SC-004, 95% under 200 ms:** met by seven of the eight query kinds, 87.5% of all calls (overall
  median 119 ms).
- **The exception** is the three-performer join. Its answer is empty, because every synthetic
  scene has two performers, so it must examine every scene (385 ms p95). Overall p95: 370 ms.

How it got there (research R12):
1. **Oxigraph:** every bench query except the anchored one ran past the 10 s limit with the
   answer ordered. Streamed with `LIMIT`, a two-performer join took 142 s for 8,000 bindings.
   Lookups cost 40–640 µs.
2. **The index prototype,** sorted whole answers: 150–690 ms, with no timeouts.
3. **The planner,** with per-predicate statistics and object-first lookups: the whole answer of
   the plain query fell from 2.1 s to 140 ms.
4. **Streamed pages** in evaluation order (owner's decision), filled straight from evaluation,
   with sizes counted without serializing twice and the envelope built without copying the
   page: the figures above.

The build with its index (`results/build-250k-index.json`) took 924 s. Writing the index added
about 19 s at publish (1.71 GB file; the store is 12 GB). Peak memory was 13.0 GB, against 16.8 GB
in spec 008's 250k build and update.

### §3 Hook cost on the test instance (2026-10-29)

Stash v0.31.1 on the test instance (1,021 scenes, 1,327 images, 1,107 performers). Each save
writes back a field's current value, so the hook fires and nothing changes. Medians are of 20
saves, with the plugin enabled and then disabled from an idle queue. Measured by a one-off
GraphQL timing script (its method is recorded in `results/hooks-cost.json`) rather than
`spike-bench hooks`, whose enable and disable switch the spike plugin, not this one.

| Save | Plugin disabled | Plugin enabled | Added per item |
|---|---|---|---|
| `sceneUpdate` | 2.87 ms | 8.32 ms | 5.4 ms |
| `imageUpdate` | 2.71 ms | 7.95 ms | 5.2 ms |
| `performerUpdate` | 2.66 ms | 7.58 ms | 4.9 ms |
| `tagUpdate` | 2.31 ms | 7.92 ms | 5.6 ms |
| `bulkSceneUpdate`, 1,000 scenes | 0.42 ms per item | 5.52 ms per item | 5.1 ms |

**Verdict:** SC-006 is met, at most 5.6 ms per saved item against 7 ms.

**Found on the way:** an "Update semantic graph" job queued just before the plugin was disabled
fails without running ("plugin semantic-engine is disabled"). Its `graph.pending` then stays
until it goes stale (15 minutes), and saves in that window start no update. Disabling the
plugin with a job queued is the only way to reach this. The next save after the stale point,
or a manual "Update semantic graph", recovers it.

### §4 Freshness and equivalence on the test instance (2026-10-29)

- **Freshness (SC-007):** `query-bench fresh` added a Role to a performer in scene 2. `query`
  showed it 1.05 s after the save, polled every 0.5 s with no task run by hand. The restore showed
  in 1.05 s too (`results/fresh.json`). **Met** (under 30 s).
- **Equivalence (SC-008):** five edits were made through GraphQL on throwaway items, with no
  task run:
  - a tag's flags;
  - a performer's tags;
  - 100 scenes in one bulk edit;
  - a scene deleted;
  - two Trait tags merged, one of them held by another performer.

  The hook-started updates were done 1.9 s later, and `status` reported `pending: false` at
  generation 9. Every fact of that generation, owners included, equals a full rebuild of the
  same library (5,995 facts each, no difference; `results/equivalence-test-instance.json`).
  **Met.** Every published generation had its query index beside it, and a query through
  Stash answered on it in 8.2 ms.
- **Hook triggers (T026):** every trigger fires except the performer and scene merges, which
  Stash v0.31.1 never fires; both were removed (`results/hook-triggers.json`).
