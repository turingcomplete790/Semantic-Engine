# Research: Engine Design Spike

Sources:
- the Stash v0.31.1 source (`Stash-Source/stash-0.31.1/`)
- the Oxigraph 0.4.11 crate source (Cargo registry)
- the two clients' source, for the mutations they send
- the viewer's 009 spec
- feature 002's measured baseline

Every decision here sets how a measurement is taken. The measured answers belong in
`decision-record.md`, not here.

## R1. How Stash runs plugin code

- **Decision**: Treat every call as a fresh process, and measure the per-call cost as process
  start + store open + work.
- **Findings**:
  - **Raw interface:** `pkg/plugin/raw.go` starts the executable for each operation, task and
    hook. It reads all of stdout with `io.ReadAll` once the process exits.
  - **RPC interface:** `pkg/plugin/rpc.go` also starts a new process per task
    (`pie.StartProviderCodec` in `Start()`) and closes it when the call returns.
  - **JS interface:** runs inside Stash's own embedded JavaScript runtime, so it cannot host the
    Rust engine.
  - **Conclusion:** no interface keeps a process alive between calls.
- **Baseline (002)**: process start ~4 ms; `ping` ~9 ms and `status` ~21 ms through
  `runPluginOperation`; first call after a plugin reload ~1.3 s (Stash re-reading plugin
  configs, which is not the plugin's cost).
- **Alternatives considered**:
  - Switching to the RPC interface: no gain, because the process lifetime is the same.

## R2. Sharing the stored graph between overlapping calls

- **Decision**: Measure three options.
  - **A. One writer, read-only readers on the live store.** The writer uses `Store::open`;
    readers use `Store::open_read_only` on the same directory.
  - **B. Published snapshots.**
    - The writer alone opens the working store.
    - After each successful write it publishes a generation with `Store::backup`, a RocksDB
      checkpoint made with hard links, into `snapshots/<n>/`.
    - It then replaces a `current` file atomically (write to a temporary file, then rename).
    - Readers open `current`'s generation read-only. Nobody writes to it after publication.
    - Old generations are removed once they are older than a grace period longer than the
      longest read.
  - **C. Writer-only.** Every call, reads included, takes the writer lock and opens the store
    read-write, which serialises everything. This is the simplest baseline.
- **Findings**:
  - **Only one read-write store per directory.** Oxigraph allows one read-write `Store` per
    directory at a time; RocksDB's `LOCK` file makes a second `open` fail at once.
  - **Option A is documented as undefined behaviour.** Oxigraph's own docs for
    `open_read_only` say: "Opening as read-only while having an other process writing the
    database is undefined behavior". The spike still runs it, to record what actually happens
    (stale reads, errors, crashes), but it can only be chosen if the measurements show it is
    safe and the record says why the documented warning does not apply. The expected result is
    rejection.
  - **Oxigraph 0.4 has no RocksDB secondary-instance mode.**
- **Writer serialisation**: an advisory lock file (the standard library's `File::try_lock`, stable
  since Rust 1.89, which works on Linux, macOS and Windows) held for the length of the write. The spike measures both behaviours when the lock
  is taken: waiting up to a bound, and failing at once with a "busy" error.
- **What is measured per option**:
  - read latency during a long write (a 250,000-scene bulk load)
  - reads that fail or wait
  - any partial state a reader sees
  - write throughput
  - publish cost and disk space per generation (B only)
- **Alternatives considered**:
  - SQLite-style WAL readers: not available in Oxigraph.
  - Copying the whole store per snapshot: rejected in favour of hard-link checkpoints, which
    are near-instant on the same filesystem. The spike checks that on Windows/NTFS through CI.

## R3. A process that outlives one call

- **Decision**: Decide this on paper first, and measure only the saving it would bring.
- **Findings**:
  - **Sockets are ruled out.** Any long-lived helper must be told about work and must answer.
    A Unix domain socket or TCP port is a listening socket, which Principle IV forbids, and
    Windows named pipes are server endpoints too.
  - **A file-based mailbox is possible but slow.** It needs no listener, but it adds polling
    latency and recovery logic for crashed helpers, on three operating systems.
  - **Stash would not know about the helper.** The helper would outlive the call that started
    it, and Stash would neither track it nor stop it on plugin reload or shutdown.
- **What is measured**: the upper bound on the saving, which is the store-open share of a call
  from R5. If opening a 250,000-scene snapshot read-only costs far less than the 200 ms budget,
  the option is dropped as not worth its risk. If it does not, the record proposes the file
  mailbox together with the constitution amendment it needs (spec FR-015).
- **Alternatives considered**:
  - A daemon with a socket: forbidden.
  - Keeping state in Stash plugin settings: too small, and not a graph.

## R4. Hooks: what fires, and its cost

- **Decision**:
  - Record hooks with the spike plugin declaring `Tag.*`, `Scene.*`, `Image.*` and
    `Performer.*` Create/Update/Destroy/Merge triggers.
  - Each hook call appends one JSON line (receive time, hook type, `id`, `inputFields`, size of
    `input`) and exits.
  - Measure the delay from the save request to the received hook, and the save's own latency
    with the hook installed and without it.
- **Findings**:
  - **Hooks run synchronously.** `pkg/plugin/plugins.go` `executePostHooks` starts each hook
    task and waits for it (`waitForTask`) before returning. It runs inside the resolver after
    the transaction commits (`internal/api/resolver_mutation_*.go`), so **every hook adds its
    full run time to the client's save**. This adds a spike measurement: the hook-handler
    budget Phase 3 must meet.
  - **Bulk edits fire one hook per item.** `bulkSceneUpdate` runs `ExecutePostHooks` once per
    scene (`resolver_mutation_scene.go` around L447), so a bulk edit of N scenes means N
    sequential plugin processes. It is measured at N = 10, 100, 1,000.
  - **Hook arguments.**
    - The hook context carries `id`, `type`, the mutation `input`, and `inputFields`, which
      lists the fields the client sent.
    - `Tag.Merge.Post` carries the destination id and the input.
    - `Tag.Destroy.Post` for `tagsDestroy` fires once per id.
  - **No loop on hooks.** Stash prevents hook loops only after 10 nested calls
    (`maxCyclicLoopDepth`). The engine's hook will never write to Stash, so loops cannot arise.
  - **Mutations the clients send** (counted from their source):
    - **semantic-tagging plugin:** `tagCreate`, `tagUpdate`, `tagDestroy`, `sceneUpdate`,
      `imageUpdate`, `performerUpdate`.
    - **viewer:** none yet. Corrected 2026-10-17: the counts above came from the viewer's
      copy of the Stash schema, not its code. The viewer is still in progress and its only
      Stash writes are play activity (`sceneSaveActivity`) and saved filter colours, which fire
      no hooks. Its rows in the hook table record that, plus the equivalent web-UI request as
      "viewer (planned)".
    - **web UI:** sends the same single and bulk mutations.
- **Driving the clients**:
  - **Scripted runs:** the harness sends each client's real mutations with that client's
    payload shape, as recorded from their source. For the semantic-tagging plugin this includes
    custom-field updates. Hooks depend on the mutation, not on which program sent it, because
    the resolver does not distinguish callers.
  - **Manual confirmation:** the owner confirms one row per client by saving from the real web
    UI, the plugin's UI and the viewer, so the scripted rows are checked against the real
    clients (spec Assumptions).
- **Alternatives considered**:
  - Polling only, without hooks: kept as the catch-up path (R7), not as the primary signal.

## R5. Per-call cost with the store reopened

- **Decision**: An operation `query` on the spike plugin:
  - Opens the published snapshot read-only and runs a fixed set of five SPARQL `SELECT`s of the
    documents' Phase 1 kind (one to five triple patterns).
  - Returns the timings for open, query and serialise inside its output.
  - The harness calls it 100+ times through `runPluginOperation` and reports median, p95 and
    spread.
  - Runs at each synthetic scale and on the seeded test library.
  - Records first-call-after-reload separately (spec Edge Cases).
- **Rationale**: Splitting the cost into process start, open, query and serialise shows which
  part dominates, which decides R3.

## R6. Delivering a whole snapshot

- **Decision**: Compare two paths:
  - **(a)** an operation that returns the snapshot serialised as N-Quads, compressed and
    base64-encoded, inside the envelope;
  - **(b)** a file the plugin writes under its folder and Stash serves.
- **Findings**:
  - **How Stash serves the file.** Plugin configs may declare `ui.assets`, a URL-prefix to
    folder map (`pkg/plugin/config.go`). Stash serves those under
    `/plugin/{pluginId}/assets/...` (`internal/api/routes_plugin.go`). The route is mounted
    behind Stash's global authentication middleware (`internal/api/server.go`), so a client
    uses its normal Stash login and no new credential exists.
  - **Path (a) size limit.** Stash reads plugin stdout fully into memory and returns it in the
    GraphQL response, so (a) is bounded by memory and by GraphQL response size. R7's limit test
    finds where it fails.
- **Freshness**: the snapshot file carries a manifest with its generation number, its build
  time, and the vocabulary and engine versions. A client compares the generation with what it
  holds. The manifest is a small file, or a tiny `snapshotInfo` operation.
- **Measured**: time and size for each path at each scale.

## R7. Largest answer and catch-up signals

- **Answer size**: the spike's `payload` operation returns a generated JSON array of a requested
  number of bindings. The harness doubles the size from 1 KiB until a call fails or takes more
  than 1 s, and records the curve. The proposed Phase 3 page size is the largest size whose p95
  stays under 200 ms.
- **Catch-up**:
  - For every save in the hook table, the harness then checks whether Stash's `updated_at`
    filters (`updated_at > t` on scenes, images, performers and tags) return the item.
  - Deletions leave nothing to filter on, so they are found by comparing ids. The harness
    measures the cost of listing every id of each type at the test library's size and
    extrapolates to Production.
  - It also checks whether a custom-field-only change bumps `updated_at`.
  - Found while measuring (2026-10-17): Stash stores `updated_at` as text in the server's local
    time (`2026-10-17T17:57:53-06:00`) and compares filter values as text, to the second. A UTC
    (`Z`) threshold silently matches the wrong set, and the filter is ignored when `ids` is also
    given. The harness sends thresholds in the server's own format (offset read from Stash).

## R8. Test data on the test server

- **Decision**: `spike-bench seed` adds semantic data to the test library.
  - **What it writes:** flagged tags, for example 6 per type, and role, action and pluggable
    assignments in the custom fields of a chosen number of existing scenes and images, using
    the documented keys (`stash-mapping.ttl`).
  - **Seed manifest:** before writing, it saves every item's previous custom fields and lists
    every created tag.
  - **Cleanup:** `spike-bench cleanup` restores those values and deletes the created tags.
  - **Server check:** both refuse to run unless the target is localhost:9998 and its stats
    match the counts recorded when seeding.
- **Rationale**: The test library has 1,021 scenes but almost no semantic data (the 002
  fixtures: 12 flagged tags, 1 scene with assignments). Real Stash fetch timings need real
  flagged data. This keeps SC-005 checkable: item counts are unchanged after cleanup.

## R9. Keeping the spike out of releases

- **Decision**:
  - The spike crate is `publish = false` and absent from `scripts/package.sh`, which builds
    only `-p plugin`. CI builds and tests it with `cargo test --workspace`, so it cannot rot,
    but never packages it.
  - The spike plugin uses its own id, `semantic-engine-spike`, and manifest, installed by
    `scripts/spike-install.sh`.
  - The adapter's mutations are behind a `spike` cargo feature that only the spike crate turns
    on.
- **Rationale**: Spec FR-013. The real plugin's contract, settings and `serverId` stay untouched.

## R10. Build time and package size

- **Decision**:
  - Take clean-build wall time from CI job durations for each of the five targets, plus one
    local clean `cargo build --release -p plugin` on the test host.
  - Take package sizes from the v0.1.0 release assets. Compare them with the viewer's
    expectations and record them as a budget for Phase 2.
- **Known now** (v0.1.0 assets): packages are 1.8 to 2.2 MB compressed (Windows zip 1.84 MB;
  Linux x86_64 tar.gz 2.18 MB).
