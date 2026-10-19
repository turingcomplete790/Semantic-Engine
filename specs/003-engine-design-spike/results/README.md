# Spike results

Every file here is written by `spike-bench` (or `scripts/spike-build-cost.sh`). Each one is a
[Measurement](../data-model.md#measurement) record, plus `hooks.json` (hook observations) and
`seed-manifest.json` (what the seed changed, so cleanup can reverse it). The decision record
links each figure it cites to a file in this folder.

**No credentials.** Results never contain the Stash API key or session cookie: `spike-bench`
strips them from commands and failure messages before writing (spec FR-002, research R9).

## Naming

`<id>[-<timestamp>].json`, where `<id>` is `<question>-<variant>-<scale>` (for example
`persist-B-250k`, `process-reopen-test`). The first run of an id is written as `<id>.json`.
Reruns never overwrite it: they are written as `<id>-<timestamp>.json` (RFC 3339 basic format,
UTC), and `spike-bench report` uses the newest and compares it with the previous one (SC-002).
See [contracts/spike-bench.md](../contracts/spike-bench.md) for the subcommands and ids.
