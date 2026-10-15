# Quickstart: Engine Plugin Foundation

Prerequisites: Rust stable, a C++17 compiler, clang; a Stash v0.31.1+ server you can reload
plugins on (the local test instance works); `shellcheck` and `dash` for script checks; `pwsh` for
the PowerShell check.

## 1. Build and test

```sh
cargo build --release
cargo test --workspace
```

Expected: all tests pass, including the operations/API-document agreement test (FR-012).

## 2. Package and install locally

```sh
scripts/package.sh                      # writes dist/semantic-engine-<ver>-<target>.tar.gz + .sha256
sh scripts/install.sh --from-file dist/semantic-engine-<ver>-<target>.tar.gz \
   --plugins-dir /path/to/stash/plugins
```

Expected: exit 0 and the install summary. In Stash, Settings → Plugins → Reload: "Semantic
Engine" appears with the task "Generate server identifier" (spec US1 scenario 1).

## 3. Call the operations

```graphql
mutation { runPluginOperation(plugin_id: "semantic-engine", args: {op: "status"}) }
mutation { runPluginOperation(plugin_id: "semantic-engine", args: {op: "nope"}) }
```

Expected: `status` returns `ok: true` with the versions in
[contracts/operations.md](contracts/operations.md); the second returns `ok: false` with
`UNKNOWN_OPERATION`. Record the status round-trip time (SC-005 baseline).

## 4. Server identifier

Run the task twice:

```graphql
mutation { runPluginTask(plugin_id: "semantic-engine", task_name: "Generate server identifier") }
query { configuration { plugins(include: ["semantic-engine"]) } }
```

Expected: the first run logs `generated`, the second `existing`; the configuration query shows
the same UUID both times, without the plugin running (FR-008).

## 5. Script portability

```sh
shellcheck -s sh scripts/install.sh
dash scripts/install.sh --help
pwsh -NoProfile -File scripts/install.ps1 -Help
```

Expected: no shellcheck findings; both print usage and exit 0.

## 6. Fixtures

```sh
STASH_URL=http://localhost:9998 STASH_API_KEY=... cargo run -p stash-adapter --bin fixture-capture -- --out /tmp/fixtures
grep -ri "apikey\|cookie" /tmp/fixtures || echo clean
```

Expected: fixture files and `index.json`; `clean` printed; library counts unchanged before and
after (SC-006).
