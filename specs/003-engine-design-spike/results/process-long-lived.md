# Evaluation: a process that outlives one call

Spec FR-005, research R3, tasks T026. Measured on 2026-10-17 against the test server (Stash
v0.31.1, localhost:9998), with the synthetic stores built by `spike-bench synth`.

## The question

Stash starts a fresh plugin process for every operation, task and hook (research R1). A helper
process that stays alive between calls could keep the graph open and save what each call
spends opening it. Is that both allowed and worth having?

## What Principle IV allows

A helper that outlives the call has to receive requests and send answers somehow:

- **TCP or Unix domain sockets.** These are listening sockets, which Principle IV forbids ("MUST
  NOT open its own listening socket").
- **Windows named pipes.** These are server endpoints, so the same rule applies.
- **A file-based mailbox.** The call writes a request file and the helper polls for it and
  writes an answer file. Nothing listens. But it costs polling latency and recovery logic for a
  crashed or stale helper on three operating systems. Stash also neither tracks nor stops a
  process it did not start: it would outlive plugin reloads, plugin disabling and Stash
  shutdown.

Only the file mailbox is technically possible, and it would still need the constitution to say
how a process Stash does not manage is started, stopped and cleaned up.

## What it could save

The most it could save is what a call spends starting the process and opening the stored
graph. A helper would still pay for Stash's own handling of `runPluginOperation` and for the
query itself. The figures below come from `results/process-reopen-<scale>.json`
(`spike-bench call-cost --scale <scale>`, 100 query calls each). All times are in ms.

| Scale | Scenes | Triples | Call median | Call p95 | Store open median (p95) | Process start median | Possible saving vs median | vs p95 |
|---|---|---|---|---|---|---|---|---|
| test | 1,021 | 40,083 | 15.7 | 29.7 | 6.8 (7.6) | 3.2 | ≈ 10 ms (64%) | ≈ 34% |
| 50k | 50,000 | 1,958,979 | 18.4 | 56.6 | 7.2 (8.5) | 3.1 | ≈ 10 ms (56%) | ≈ 18% |
| 100k | 100,000 | 3,917,874 | 19.5 | 87.1 | 6.9 (8.1) | 3.0 | ≈ 10 ms (51%) | ≈ 11% |
| 250k | 250,000 | 9,794,566 | 21.4 | 88.7 | 7.9 (8.9) | 2.9 | ≈ 11 ms (51%) | ≈ 12% |

- **Opening the store costs the same at every size.** Reopening a published snapshot read-only
  costs 7–9 ms whether it holds 40 thousand triples or nearly 10 million. RocksDB opens only its
  manifest and indexes, not the data.
- **The tail comes from the queries.** Query evaluation sets the p95, through the
  scene-scoped joins (query ids 4 and 5), and a long-lived process would not change that.
- **The budget is already met.** Every scale is well inside the viewer's 200 ms budget at p95
  with the store reopened on every call. The worst p95 is 88.7 ms at 250k scenes, 2.5 times the
  owner's 49,683 scenes.

## Verdict

**Dropped.** A long-lived helper would save about 10 ms per call, and it is not needed to meet
the 200 ms budget. In exchange it would add a process Stash does not manage, a file mailbox, and
a constitution amendment. Phase 2 keeps one process per call and reopens the published snapshot
read-only each time. No amendment is proposed (FR-015).

## A cost to carry forward

Opening the working store **read-write** (`write` op, `lock` timing in `persist-*-250k.json`)
takes 100–150 ms including the writer lock, much more than the 7–9 ms read-only open. Stash runs
hooks synchronously, so their time is added to each save (research R4). A hook handler that
opened the writer on every save would add that time to every save. The freshness design (US2)
has to take this into account: for example, a hook only records the change, and a separate
writer applies the recorded changes.
