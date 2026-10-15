# Contract: Install Scripts

## Invocation

```text
sh install.sh [--plugins-dir DIR] [--version X.Y.Z] [--from-file PACKAGE] [--yes]
pwsh -File install.ps1 [-PluginsDir DIR] [-Version X.Y.Z] [-FromFile PACKAGE] [-Yes]
```

- `install.sh` uses POSIX `sh` only: no arrays, `[[ ]]`, `local`, `source`, `pipefail`, or
  `$'…'`; utilities limited to `uname`, `mkdir`, `cp`, `mv`, `rm`, `tar`, `sed`, `grep`, and one
  of `curl`/`fetch`/`wget` plus one of `sha256sum`/`shasum -a 256`/`sha256`.
- `install.ps1` runs on Windows PowerShell 5.1 and PowerShell 7.

## Behaviour

1. Detect OS and architecture; map to a package target (research R7). No target → exit 3.
2. Find the plugins folder (research R8). Not found or not writable → exit 4.
3. Download the package and `SHA256SUMS` (or use `--from-file` with its `.sha256` beside it);
   verify; mismatch → exit 5.
4. Unpack to a temporary folder, then replace `<plugins>/semantic-engine/` contents; nothing
   outside it is touched.
5. Print the installed version, the folder, and "Reload plugins in Stash (Settings → Plugins)".

## Exit codes

| Code | Meaning |
|---|---|
| 0 | installed or already up to date |
| 2 | bad arguments |
| 3 | no package for this platform; message names OS, architecture, release page, and build-from-source steps |
| 4 | plugins folder not found or not writable |
| 5 | download or checksum failure |
| 6 | unpack or copy failure (previous install left intact) |
