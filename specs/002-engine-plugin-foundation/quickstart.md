# Quickstart: Engine Plugin Foundation

Prerequisites: Rust stable, a C++17 compiler, clang; a Stash v0.31.1+ server you can reload
plugins on (the local test instance works); `shellcheck`, `dash`, and `busybox` for script
checks; `pwsh` for the PowerShell check.

Last full run: 2026-10-16 against the Stash v0.31.1 test instance; every step passed (results
noted under each step).

## 1. Build and test

```sh
cargo build --release
cargo test --workspace
```

Expected: all tests pass, including the operations/API-document agreement test (FR-012).

## 2. Package and install locally

```sh
sh scripts/package.sh                   # writes dist/semantic-engine-<ver>-<target>.tar.gz + .sha256
sh scripts/install.sh --from-file dist/semantic-engine-<ver>-<target>.tar.gz \
   --plugins-dir /path/to/stash/plugins
# or, from the published release:
sh scripts/install.sh --version 0.1.0 --plugins-dir /path/to/stash/plugins
```

Then reload plugins (Settings → Plugins → Reload plugins, or `mutation { reloadPlugins }`).

Expected: exit 0 and the install summary; "Semantic Engine" appears with the task "Generate
server identifier" (spec US1 scenario 1). Result: as expected, both from a local package and from
the v0.1.0 release.

## 3. Call the operations

```graphql
mutation { runPluginOperation(plugin_id: "semantic-engine", args: {op: "status"}) }
mutation { runPluginOperation(plugin_id: "semantic-engine", args: {op: "nope"}) }
```

Expected: `status` returns `ok: true` with the versions in
[contracts/operations.md](contracts/operations.md); the second returns `ok: false` with
`UNKNOWN_OPERATION`. Record the status round-trip time (SC-005 baseline).

Result (SC-005 baseline, local server, release build): `status` 24 to 26 ms warm through
`runPluginOperation`, `ping` about 9 ms, process start about 4 ms; the first call right after a
plugin reload took about 1.3 s.

## 4. Server identifier

Run the task twice:

```graphql
mutation { runPluginTask(plugin_id: "semantic-engine", task_name: "Generate server identifier") }
query { configuration { plugins(include: ["semantic-engine"]) } }
```

Expected: the first run logs `generated`, the second `existing`; the configuration query shows
the same UUID both times, without the plugin running (FR-008). The outcome is in Stash's log:

```graphql
query { logs { level message } }
```

Result: both jobs `FINISHED`; the log shows `Info` lines
`[Plugin / Semantic Engine] serverId=<uuid> (existing)` (the identifier was generated on an
earlier run); the configuration holds `serverId` under `semantic-engine`.

## 5. Script portability

```sh
shellcheck -s sh scripts/install.sh
dash scripts/install.sh --help
pwsh -NoProfile -File scripts/install.ps1 -Help
```

Expected: no shellcheck findings; both print usage and exit 0. The full script test suites are
`dash scripts/tests/install_sh_test.sh`, `busybox sh scripts/tests/install_sh_test.sh`, and
`pwsh -File scripts/tests/install_ps1_test.ps1`. Result: as expected; CI also runs them, and the
PowerShell suite on Windows.

## 6. Fixtures

```sh
STASH_URL=http://localhost:9998 STASH_API_KEY=... cargo run -p stash-adapter --bin fixture-capture -- --out /tmp/fixtures
grep -ri "apikey\|cookie" /tmp/fixtures || echo clean
```

Expected: fixture files and `index.json`; `clean` printed; library counts unchanged before and
after (SC-006). Result: 5 files, `clean`, counts unchanged; the committed fixtures are in
`crates/stash-adapter/tests/fixtures/test-instance/`.

## 7. Live integration tests (optional)

```sh
STASH_TEST_URL=http://localhost:9998 STASH_TEST_API_KEY=... \
STASH_TEST_PLUGINS_DIR=/path/to/stash/plugins STASH_TEST_PACKAGE=dist/<package>.tar.gz \
  cargo test -p plugin --test integration_install --test integration_server_id -- --nocapture
```

They install the package, reload plugins, call `status`, and run the identifier task twice.
