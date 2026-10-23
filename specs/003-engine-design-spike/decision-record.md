# Decision Record: Engine Design Spike

Outcome: proceed-with-changes
Date: 2026-10-19 · Stash: v0.31.1 · Engine commit: 2026f7d · Vocabulary: 1.1.0
Revised: 2026-10-20, for constitution v3.0.0 · Owner review: 2026-10-20 (T044)

Measured on the local test server (`http://localhost:9998`, Linux, 1,021 scenes and 1,327
images) and on synthetic graphs of 50,000, 100,000 and 250,000 scenes. Each result file records
the commit it was measured with. `spike-bench report` lists every result with its previous run
(SC-002).

## Summary

Proceed with the shared engine. Clients are user interfaces over it: they query the engine, and
hold no graph, snapshot, or engine of their own (constitution v3.0.0, Principles I and III).

The design works inside Stash's one-process-per-call model:
- **Queries are fast:** with the stored graph reopened on every call, a query answers in
  88.7 ms at p95 with 250,000 scenes, against the viewer's 200 ms budget.
- **Reads are safe during writes:** reading published snapshots while a write runs gave 6,570
  reads, none failed and none saw a partial write.
- **Hooks keep the graph fresh:** every save reached a hook, with none missed and none out of
  order.

Hooks run synchronously, so each one adds about 5 ms to every saved item: a 1,000-scene bulk
edit takes 5.6 s instead of 0.44 s. The owner accepts that cost, since bulk semantic edits are
already slow. To keep the cost at 5 ms, a hook only records the change. The next engine call
applies it, which takes about 0.2 s after a single save at 250k scenes.

The changes this outcome needs:
- **Constitution v3.0.0** (adopted 2026-10-19): clients depend on the engine, and Principle IV's
  open questions are settled.
- **ROADMAP Phase 4:** reworded so the viewer has no fallback engine.
- **Phase 2:** the rules below.

## For client maintainers

- **Depending on the engine:**
  - Your client's semantic features query the engine. When the engine is missing, say so
    clearly; don't compute answers yourself (Principle I).
  - There are no snapshots to download and no sync call to make.
- **Page size:**
  - Up to 1 MiB per answer, about 8,600 bindings of the shape item IRI plus label. The answer
    then takes 102 ms at p95, so with the query (p95 89 ms at 250k) the call stays under 200 ms.
  - 2 MiB (17,231 bindings, p95 192 ms) is the hard ceiling, with no room left for the query.
- **Expected freshness delay:**
  - **When a change shows:** it reaches the engine 4–9 ms after the save, as a hook. It shows in
    the answer to the first engine call after that. That call applies the change before
    answering: about 0.2 s for one item, 0.9 s for 100 items and 4 s for 1,000 items at 250k
    scenes. Later calls are back to normal speed.
  - **Basis:** an estimate built from measured parts (`hooks.json`, the `applyEdit` timings in
    `snapshot-delta-250k`). The end-to-end path is a Phase 2 build.
  - **Engine disabled or absent:** changes made then are caught up by the engine itself, from
    `updated_at` and an id comparison. Clients do nothing.
- **Cost the engine adds to every save:** about 5 ms per saved item, because hooks are
  synchronous.
  - A single save goes from about 1 ms to about 6 ms.
  - A 100-scene bulk edit goes from 45 ms to 0.52 s, and 1,000 scenes from 0.44 s to 5.6 s.

## Decisions

### 1. Process model

**Decision:** One process per call, as Stash runs it, reopening the published snapshot read-only
on every call. A long-lived helper process is dropped.

**Options:**

| Name | Key figures | Constitution | Verdict |
|---|---|---|---|
| Reopen per call | Call p95: 29.7 ms (test), 56.6 ms (50k), 87.1 ms (100k), 88.7 ms (250k). Store open: 6.8–7.9 ms median at every size. Process start: about 3 ms. First call after a reload: 13–19 ms | meets | chosen |
| Long-lived helper (file mailbox) | Would save about 10 ms per call (51–64% of the median, 11–34% of the p95). The p95 comes from query evaluation, which a helper would not change | needs-amendment: a process Stash does not manage; sockets and pipes violate Principle IV | rejected |

SC-004 is met: the p95 is 29.7 ms on the test library and 88.7 ms at 250k, under 200 ms.

A rerun at the test scale on 2026-10-19 gave a median of 17.71 ms against the 15.70 ms recorded
(+12.8%), within SC-002's 25%. Feature 002 measured 1.3 s for the first call after a reload.
Here, that first call was 13–19 ms, and it is recorded separately.

**Evidence:**
- Per-call cost by scale: [process-reopen-test](results/process-reopen-test.json),
  [process-reopen-50k](results/process-reopen-50k.json),
  [process-reopen-100k](results/process-reopen-100k.json),
  [process-reopen-250k](results/process-reopen-250k.json)
- Rerun: [process-reopen-test (rerun)](results/process-reopen-test-20261019T211112Z.json)
- First call after a reload: [process-first-call-test](results/process-first-call-test.json),
  [process-first-call-250k](results/process-first-call-250k.json)
- Long-lived helper: [process-long-lived.md](results/process-long-lived.md)

**Reproduce:** `for s in test 50k 100k 250k; do spike-bench synth --scale $s; spike-bench call-cost --scale $s; done`

### 2. Persistence

**Decision:** Option B. One writer works on a private store and publishes numbered read-only
snapshot generations by atomic rename. Readers open only the published generation. A second
writer fails at once (`fail` lock mode) and retries later.

**Options:**

| Name | Key figures (reads during a 41–50 s write of 9.79M triples, 4 readers) | Constitution | Verdict |
|---|---|---|---|
| A: live store opened read-only | 5,187 reads: 3,729 (72%) saw a partial write, 9 failed with RocksDB `Corruption: Can't access …sst` errors | violates Principle IV, which forbids silently wrong results | rejected |
| B: published snapshot generations | 6,570 reads: 0 failed, 0 partial, p95 48.4 ms. Readers saw generations 1 and 2. Publishing took 8.2 ms | meets | chosen |
| C: writer lock for every call | 9 reads, 8 of them waited for the write; p95 41.5 s | meets, but blocks queries for the whole write | rejected |

Opening the writer costs 100–150 ms, including the lock, against 7–9 ms for a read-only open.
With two writers, the second gets "busy" in about 5 ms in `fail` mode. In `wait` mode it waits
for the first (535 ms lock wait in the test).

A crashed writer is not a risk to the lock: the advisory lock is the operating system's and is
released when the process exits. The spike did not test a killed writer.

**Evidence:** [persist-A-250k](results/persist-A-250k.json),
[persist-B-250k](results/persist-B-250k.json), [persist-C-250k](results/persist-C-250k.json)

**Reproduce:** `for o in A B C; do spike-bench concurrency --option $o --scale 250k; done`

### 3. Freshness

**Decision:**
- Stash's hooks keep the graph fresh (constitution Principle IV). The engine subscribes to the
  Create, Update, Destroy and Merge hooks of tags, scenes, images and performers.
- A hook handler only appends the event to a journal file and does no store work, so it adds
  about 5 ms to the save.
- The next engine call that finds journal entries applies them under the writer lock, publishes
  a new generation, and then answers.
- Changes made while the engine was absent or disabled are caught up from `updated_at`, and
  deletions by an id comparison.

**Options:**

| Name | Key figures | Constitution | Verdict |
|---|---|---|---|
| Hooks that apply the change themselves | Opening the writer costs 100–150 ms per item, and an edit about 190 ms at 250k, on top of the save, because hooks are synchronous. A 1,000-scene bulk edit would take minutes | meets IV, breaks Principle VII's budgets | rejected |
| Hooks that record; the next engine call applies | Save: +5 ms per item (0.7–1.3 ms becomes 5.8–7.5 ms; bulk 1,000 scenes 0.44 s becomes 5.57 s). Every save reached a hook, none missed, none out of order. The applying call pays about 0.2 s for 1 item, 0.9 s for 100 and 4 s for 1,000 at 250k | meets | chosen |
| No update hooks; catch up from `updated_at` | No cost on ordinary saves. Every semantic save bumped `updated_at` (15 of 15 checked, bulk included; play activity, which is not semantic, did not). The check costs 1.9–2.7 ms median | needs an amendment to Principle IV, which the owner declined (2026-10-19) | rejected; used only to catch up periods without hooks |

The applying call's cost comes from `applyEdit`, which edits scenes already in the store. A
real apply also fetches the changed items from Stash, about 0.38 ms per item warm
([build-stash-test](results/build-stash-test.json)).

Findings that Phase 2 must handle:
- **Thresholds:** Stash stores `updated_at` as server-local text (`2026-10-17T17:57:53-06:00`) and
  compares filters as text, to the second. A UTC (`Z`) threshold matched 0 of 1,000 changed
  scenes. The filter is also ignored when `ids` is given.
- **Merges:** a merge fires only `Tag.Merge.Post`, for the destination tag. The merged-away
  source tags get no Destroy hook.
- **Bulk deletes:** `tagsDestroy` of 3 tags fires 3 `Tag.Destroy.Post` hooks.
- **Play activity:** `sceneSaveActivity`, the viewer's only write today, fires no hook and does
  not bump `updated_at`. It is not a semantic save.
- **Hook content:** web-UI forms send their whole field list in `inputFields`. A hook fires even
  when only an ignored field changed, so filtering by field is not possible.
- **Owner's manual saves:** five rows, from the web UI and the semantic-tagging plugin. Each fired
  exactly the expected hook.

**Hook table:** 12 entity × client pairs (SC-003), plus bulk and manual rows. Times are medians in
ms.

| Entity | Client | Mutation | Hooks received | Delay | Save with hook | Save without hook | `updated_at` bumped | Missed / out of order |
|---|---|---|---|---|---|---|---|---|
| tag | web-ui | tagUpdate | 1 Tag.Update.Post | 4.4 | 6.5 | 0.9 | yes | no / no |
| scene | web-ui | sceneUpdate | 1 Scene.Update.Post | 8.8 | 6.9 | 1.2 | yes | no / no |
| image | web-ui | imageUpdate | 1 Image.Update.Post | 5.5 | 6.8 | 1.1 | yes | no / no |
| performer | web-ui | performerUpdate | 1 Performer.Update.Post | 5.1 | 6.2 | 0.9 | yes | no / no |
| tag | semantic-tagging | tagUpdate | 1 Tag.Update.Post | 4.2 | 5.9 | 0.7 | yes | no / no |
| scene | semantic-tagging | sceneUpdate | 1 Scene.Update.Post | 4.9 | 6.7 | 1.2 | yes | no / no |
| image | semantic-tagging | imageUpdate | 1 Image.Update.Post | 4.9 | 6.3 | 1.3 | yes | no / no |
| performer | semantic-tagging | performerUpdate | 1 Performer.Update.Post | 5.3 | 6.1 | 1.2 | yes | no / no |
| tag | viewer | none sent yet | — | — | — | — | — | — |
| scene | viewer | sceneSaveActivity | none | — | 0.9 | 0.5 | no | — |
| image | viewer | none sent yet | — | — | — | — | — | — |
| performer | viewer | none sent yet | — | — | — | — | — | — |

The viewer is in progress and sends no semantic saves. The web UI's request was also run as the
viewer's planned equivalent. Those rows match the web-UI rows: 5.0–5.8 ms delay, 5.8–7.5 ms with
the hook, 0.7–1.1 ms without, and `updated_at` bumped.

| Bulk or manual row | Items | Hooks received | Delay | Save with hook | Save without hook | Missed / out of order |
|---|---|---|---|---|---|---|
| web-ui tagsMerge | 2 | 1 Tag.Merge.Post (destination only) | 7.1 | 8.6 | 1.9 | no / no |
| web-ui tagsDestroy | 3 | 3 Tag.Destroy.Post | 15.3 | 16.4 | 1.4 | no / no |
| semantic-tagging tagDestroy | 1 | 1 Tag.Destroy.Post | 5.5 | 6.7 | 1.0 | no / no |
| web-ui bulkSceneUpdate | 10 | 10 Scene.Update.Post | 54 | 55 | 5.9 | no / no |
| web-ui bulkSceneUpdate | 100 | 100 Scene.Update.Post | 521 | 517 | 45 | no / no |
| web-ui bulkSceneUpdate | 1,000 | 1,000 Scene.Update.Post | 5,565 | 5,569 | 444 | no / no |
| manual, owner (web-ui and semantic-tagging) | 5 saves | Performer.Create, 2 × Scene.Update, 2 × Tag.Update | — | — | — | no / no |

**Evidence:** [hooks.json](results/hooks.json),
[freshness-updated-at](results/freshness-updated-at.json),
[freshness-id-diff](results/freshness-id-diff.json),
[freshness-id-diff (rerun)](results/freshness-id-diff-20261018T001555Z.json)

The id-diff rerun's overall median is 44% lower than the first run's (25.2 ms against 14.1 ms).
This comes from the cold first run and the mix of entity types. Each type's median held steady:
scenes took 29.3 ms, then 28.4 ms.

**Reproduce:**
```sh
spike-bench hooks
for n in 10 100 1000; do spike-bench hooks --entity scene --bulk $n; done
spike-bench catch-up
spike-bench hooks --manual --client web-ui --seconds 120
```

### 4. Result limits

**Decision:** Pages of at most 1 MiB, about 8,600 bindings. 2 MiB is the hard ceiling.

**Options:**

| Name | Key figures | Constitution | Verdict |
|---|---|---|---|
| 1 MiB pages | p95 101.6 ms for the answer alone; with a 250k query (p95 88.7 ms) still under 200 ms | meets Principle VII | chosen |
| 2 MiB pages (largest with p95 under 200 ms) | 17,231 bindings, p95 192.1 ms with no time left for the query | meets, with no margin | rejected |
| Unpaged answers | Time grows by about 75 ms per MiB: a 4 MiB median is 312 ms, 16 MiB is 1.2 s. No call failed up to 16 MiB | violates Principle VII's query budget | rejected |

**Evidence:** [limits-payload](results/limits-payload.json)

**Reproduce:** `spike-bench payload`

### 5. Snapshot delivery

**Decision:** Clients do not take snapshots. They query the engine and hold no copy of its graph
(constitution v3.0.0, Principle III). Snapshot generations stay inside the engine, as the way its
own reads stay safe during a write (section 2). The measurements below are kept for reference.

**Options:**

| Name | Key figures (250k scenes, 9.79M quads) | Constitution | Verdict |
|---|---|---|---|
| Whole snapshot inside an operation result (base64) | 211 s per export, mostly the plugin's dump; 98.5 MB of base64 inside the GraphQL answer; 90 ms to decode | violates Principle III (client keeps a copy) | rejected |
| Whole snapshot as a file Stash serves | 186 s to dump, 105 ms to GET the 74 MB file. Answers 401 without a Stash login, 200 with one | violates Principle III | rejected |
| Change files per generation | One scene: 317 B, edit 190 ms, client apply 1.2 ms. 1,000 scenes: 126 KB, 4.0 s, 1.6 s. The client's copy matched the latest generation exactly (9,794,566 quads, 3,333 scenes checked) | violates Principle III | rejected |
| No client snapshots; clients query | Query p95 88.7 ms at 250k (section 1); answers paged at 1 MiB (section 4) | meets | chosen |

Findings kept for later specs:
- **Asset route:** the plugin's asset route is behind Stash's login.
- **Generation check:** `snapshotInfo` reports the current generation in 4.6–4.9 ms, which is
  the cost of such a check if the engine ever needs one.
- **Edit cost:** `applyEdit`'s timings measure what the engine pays to apply a change (section 3).
- **Small libraries:** at the test library's size, both delivery paths take about 0.52 s for
  40,083 quads (242 KiB gzip).
- **Compression level:** gzip level 1 cuts the full dump to 121 s but grows it to 113 MB.

**Evidence:** [snapshot-asset-250k](results/snapshot-asset-250k.json),
[snapshot-op-250k](results/snapshot-op-250k.json),
[snapshot-asset-test](results/snapshot-asset-test.json),
[snapshot-op-test](results/snapshot-op-test.json),
[snapshot-delta-250k](results/snapshot-delta-250k.json),
[snapshot-delta-test](results/snapshot-delta-test.json)

**Reproduce:**
```sh
for s in test 250k; do spike-bench snapshot --path op --scale $s; spike-bench snapshot --path asset --scale $s; done
for s in test 250k; do spike-bench delta --scale $s; done
```

### 6. Build time and size

**Decision:** The engine meets all three viewer 009 budgets. No build or packaging changes are
needed.

**Options:**

| Name | Key figures | Constitution | Verdict |
|---|---|---|---|
| Current build (bundled RocksDB, five targets) | Graph build: 0.14 ms per scene (250k scenes in 34.6 s, 8.3 KB stored per scene). From Stash: 408 ms for 1,060 semantic items over 21 requests (0.38 ms per item); the first, cold run took 4.2 s. Release archives: 1.8–2.2 MB; plugin binary 5.95 MB. CI: 46–60 s per release target, 78–121 s per check job. Local clean release build: 234 s | meets | chosen |

Against the viewer's 009 budgets:

| Budget | Result | Basis |
|---|---|---|
| SC-001: first build of the largest library within 5 minutes | estimated 30–40 s | estimate, see Estimates |
| SC-002: ready within 2 s from a stored copy | 7–9 ms to open the snapshot; 13–19 ms first call | measured |
| SC-003: Phase 1 queries' first results under 200 ms | p95 88.7 ms at 250k | measured |

**Evidence:** [build-release](results/build-release.json),
[build-stash-test](results/build-stash-test.json),
[build-synth-test](results/build-synth-test.json),
[build-synth-50k](results/build-synth-50k.json),
[build-synth-100k](results/build-synth-100k.json),
[build-synth-250k](results/build-synth-250k.json)

**Reproduce:** `scripts/spike-build-cost.sh`, `spike-bench build --from-stash`,
`spike-bench synth --scale 250k`

### 7. Go / no-go

**Decision:** proceed-with-changes. The shared engine meets every client budget measured.

The changes:
- **Constitution v3.0.0:** adopted 2026-10-19.
- **`ROADMAP.md`:** Phase 4 is reworded so the viewer queries the engine and has no fallback
  engine.
- **Phase 2:** the rules below.

**Options:**

| Name | Key figures | Constitution | Verdict |
|---|---|---|---|
| Proceed; hooks drive freshness, clients query | Query p95 88.7 ms at 250k. +5 ms per saved item (bulk 1,000 scenes 0.44 s → 5.6 s), accepted. First call after a change: about 0.2 s at 250k | meets v3.0.0 | chosen |
| Proceed with `updated_at` catch-up instead of update hooks | No cost on saves, but needs a client poll or a sync call | needs an amendment to Principle IV, declined | rejected |
| Fall back to in-process engines in each client | Each client builds its own graph; answers diverge | violates Principles I and III (v3.0.0) | rejected |

**Evidence:** sections 1–6.

**Reproduce:** the commands in sections 1–6, then `spike-bench report`.

## Rules for Phase 2 and Phase 3

1. **Persistence:** per Principle IV, the engine keeps one working store and publishes numbered
   snapshot generations under the plugin's folder, replacing `current` by atomic rename.
2. **Readers:** per Principle IV, every read operation opens the published generation read-only
   and never opens the working store.
3. **Concurrency:** per Principle IV, only the holder of `writer.lock` writes; any other call
   that wants to write uses `fail` mode, gets "busy" in about 5 ms, and leaves the work for a
   later call.
4. **Hook subscriptions:** the engine subscribes to the Create, Update, Destroy and Merge hooks
   of tags, scenes, images and performers.
5. **Hook work:** a hook handler appends the event to a journal and does no store work, because
   its time is added to the save.
6. **Applying changes:**
   - The first engine call that finds journal entries tries the writer lock in `fail` mode.
   - If it gets the lock, it applies the entries, publishes a new generation, and then answers.
   - If another call holds the lock, it answers from the current generation and reports that
     changes are pending.
   - **Superseded 2026-10-21** by `specs/004-relations-role-hierarchy/research.md` R6: the hook
     records the change and starts a Stash background job ("Apply relations"); no client call
     pays for applying. The owner judged that a 1,000-scene edit adding seconds to some later
     call is not acceptable.
7. **Catch-up:** each apply also asks Stash for items with `updated_at` after the last apply.
   This catches saves whose hooks were lost, for example while the plugin was disabled. The
   threshold is sent in the server's local-time format, one second early, and never together
   with `ids`.
8. **Deletions:**
   - Destroy hooks report deletions.
   - A merge reports only its destination tag, so the engine re-reads that tag and compares tag
     ids to remove the merged-away sources.
   - Deletions while the engine was absent are found by an id comparison, run as a Stash task.
9. **Paging:** answers are paged at 1 MiB, about 8,600 bindings, and never exceed 2 MiB.
10. **Clients:** no operation hands a client the engine's graph, a snapshot, or change files
    (Principle III).
11. **Hook budget:** a hook must add no more than 7 ms to a save (4.6–6.5 ms measured), checked
    by `spike-bench hooks`.
12. **Query budget:** query p95 stays under 200 ms at 250k scenes, measured by
    `spike-bench call-cost --scale 250k`.
13. **Apply budget:**
    - The call that applies changes may exceed the query budget by the apply time.
    - Phase 2 states that time per item and measures it: the spike's `applyEdit` gives about
      0.2 s for 1 item and 4 s for 1,000 items at 250k.

## Constitution amendments proposed

Adopted in constitution v3.0.0 (2026-10-19). No further amendment is proposed.

- **Principle I:**
  - **Before:** the engine is an optimisation.
  - **Now:** clients' semantic features depend on it, and clients report when it is missing.
- **Principle III:** clients query the engine and keep no graph, snapshot, or engine of their own.
- **Principle IV:** the spike's answers are recorded.
  - **Persistence:** an on-disk store with published generations.
  - **Concurrency:** a single writer; readers open only a published generation, never the store
    being written. Option A showed why: reading the live store hit partial writes in 72% of
    reads and RocksDB corruption errors.
  - **Freshness:** from hooks, with `updated_at` and id comparison for periods without hooks.
  - Hook cost must be measured.
- **Principles V–VII:** the client fallback wording is removed, and hook cost is added to the
  budgets.

A freshness amendment, catching up from `updated_at` without update hooks, was proposed in the
first draft of this record (2026-10-19) and declined by the owner.

The process model needed no amendment: the long-lived helper was dropped.

## Estimates

Each figure below is an estimate, scaled from the per-item basis shown.

| Figure | Production (49,683 scenes, 29,057 images) | 500k scenes | Basis |
|---|---|---|---|
| Graph build from a store (estimate) | 7 s | 69 s | 0.138 ms per scene ([build-synth-250k](results/build-synth-250k.json)) |
| First build from Stash (estimate) | 30 s; 5.3 min if every item cost what the one cold run did | — | 0.38 ms per item over about 79,000 items, warm. Cold: 4.0 ms per item, from one sample ([build-stash-test](results/build-stash-test.json)) |
| Stored size (estimate) | 410 MB | 4.2 GB | 8.3 KB per scene at 250k ([build-synth-250k](results/build-synth-250k.json)) |
| Full snapshot dump, reference only: clients take no snapshots (estimate) | 37 s, 15 MB gzip | 6.2 min, 148 MB | 0.74 ms and 296 B per scene at 250k ([snapshot-asset-250k](results/snapshot-asset-250k.json)) |
| Deletion check by id comparison (estimate) | 2.1 s (1.4 s scenes, 0.7 s images) | 14 s for scenes | 0.028 ms per id ([freshness-id-diff](results/freshness-id-diff.json)) |
| Extra save time from the engine's hooks (estimate) | 5.1 ms per item, at any library size | same | bulk 1,000 scenes: 5.13 s extra ([hooks.json](results/hooks.json)) |

Query p95 is not extrapolated. It rose from 29.7 ms (1k scenes) to 87.1 ms (100k) and then
stayed flat to 250k (88.7 ms), so it must be re-measured, not scaled, beyond 250k.

The cold first build from Stash is the one estimate near a budget: 5.3 min against SC-001's
5 minutes. It rests on a single cold sample, which mostly reflects fixed start-up cost. Phase 2
measures a first build on a library of Production size.
