# Contract: `spike-bench`

`spike-bench` is the measurement harness, run from the workspace with
`cargo run -p spike --bin spike-bench -- <subcommand>`. Every figure in the decision record comes
from one of these subcommands (spec FR-002).

## Connection

- Reads `STASH_TEST_URL` and `STASH_TEST_API_KEY` from the environment, the same variables as 002.
- Refuses to run if `STASH_TEST_URL` is not `http://localhost:9998`, unless `--allow-host <url>`
  is given. Writes are refused on any host other than the test server, even with that flag.
- Never prints, logs, or writes the API key to `results/`.
- Talks to Stash only through `stash-adapter` with the `spike` feature.

## Global options

| Option | Default | Meaning |
|---|---|---|
| `--runs N` | 100 (latency), 3 (build/size) | samples per figure |
| `--scale S` | `test` | `test`, `50k`, `100k`, `250k`, or a scene count |
| `--out DIR` | `specs/003-engine-design-spike/results` | where result JSON goes |

## Subcommands

| Subcommand | Spec | Writes to Stash | Result ids |
|---|---|---|---|
| `seed [--scenes N --images N]` | R8 | yes (test server) | `seed-manifest.json` |
| `cleanup` | R8, SC-005 | yes (reverses seed) | updates `seed-manifest.json` |
| `synth --scale S` | FR-011 | no | `build-synth-<S>` (build time, store size, triples) |
| `build --from-stash` | FR-011 | no (flag-filtered reads) | `build-stash-test` (fetch + build time per item) |
| `call-cost --scale S` | FR-003, R5 | no | `process-reopen-<S>`, `process-first-call-<S>` |
| `concurrency --option A\|B\|C --scale S` | FR-004, R2 | no | `persist-<option>-<S>` |
| `hooks [--client C] [--entity E] [--bulk N]` | FR-006, FR-007, R4 | yes (test data only) | `hooks.json` rows |
| `catch-up` | FR-007, R7 | no | `freshness-updated-at`, `freshness-id-diff` |
| `payload` | FR-008, R7 | no | `limits-payload` (size curve) |
| `snapshot --path op\|asset --scale S` | FR-009, R6 | no | `snapshot-<path>-<S>` |
| `report` | FR-001 | no | prints a Markdown summary of `results/` to paste into the decision record |

Every subcommand writes [Measurement](../data-model.md#measurement) JSON. A rerun writes a file
with a timestamp suffix and never overwrites an earlier one. `report` uses the newest run and
shows how far it differs from the previous one (SC-002).

## Exit codes

| Code | Meaning |
|---|---|
| 0 | measurement completed and was written |
| 1 | measurement ran, but a run failed (failures recorded in the result) |
| 2 | bad arguments or environment |
| 3 | refused: not the test server, or no seed manifest for `cleanup` |
| 4 | Stash unreachable, or the spike plugin is not installed |
