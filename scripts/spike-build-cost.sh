#!/bin/sh
# Build time and package size for the Phase 1 spike (specs/003-engine-design-spike, FR-010, R10).
#
# POSIX sh. Reads the v0.1.0 release asset sizes and the per-job durations of the latest green
# `ci` and `release` runs with the GitHub CLI, times RUNS local clean release builds of the
# plugin, and writes results/build-release.json as a Measurement (question `build`). A rerun
# writes build-release-<timestamp>.json; earlier results are never overwritten.
#
# Exit codes: 0 written, 2 bad arguments, 4 gh missing or not logged in, 5 build failure.

set -u

RUNS=3
OUT="specs/003-engine-design-spike/results"
RELEASE="v0.1.0"

usage() {
    cat <<'USAGE'
Measure build time and package size (spike FR-010).

usage: spike-build-cost.sh [--runs N] [--out DIR] [--release TAG]

Run from the workspace root. Needs `gh` (logged in) and cargo.
USAGE
}

say() { printf '%s\n' "$*" >&2; }
die() { code=$1; shift; printf 'error: %s\n' "$*" >&2; exit "$code"; }

while [ $# -gt 0 ]; do
    case "$1" in
        --runs) [ $# -ge 2 ] || die 2 "--runs needs a value"; RUNS=$2; shift 2 ;;
        --out) [ $# -ge 2 ] || die 2 "--out needs a value"; OUT=$2; shift 2 ;;
        --release) [ $# -ge 2 ] || die 2 "--release needs a value"; RELEASE=$2; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; die 2 "unknown argument: $1" ;;
    esac
done
case "$RUNS" in ''|*[!0-9]*) die 2 "--runs needs a number" ;; esac
[ "$RUNS" -ge 3 ] || die 2 "--runs must be at least 3 (build figures need 3 runs)"
[ -f Cargo.toml ] || die 2 "run from the workspace root"
command -v gh >/dev/null 2>&1 || die 4 "gh (GitHub CLI) is not installed"
gh auth status >/dev/null 2>&1 || die 4 "gh is not logged in"

# Release assets as a JSON array of { name, bytes } (checksum files left out).
assets=$(gh release view "$RELEASE" --json assets \
    --jq '[.assets[] | select(.name | endswith(".sha256") | not) | {name, bytes: .size}]') \
    || die 4 "could not read release $RELEASE"

# Per-job durations of the latest green run of a workflow, as { workflow, runId, jobs: [...] }.
jobs_of() {
    id=$(gh run list --workflow "$1" --status success --limit 1 --json databaseId \
        --jq '.[0].databaseId') || return 1
    [ -n "$id" ] || { printf 'null'; return 0; }
    gh run view "$id" --json jobs --jq "{workflow: \"$1\", runId: $id, jobs: [.jobs[] |
        {name, seconds: ((.completedAt | fromdateiso8601) - (.startedAt | fromdateiso8601))}]}"
}
ci=$(jobs_of ci.yml) || die 4 "could not read the ci workflow runs"
release=$(jobs_of release.yml) || die 4 "could not read the release workflow runs"

# Local clean release builds of the plugin, in milliseconds.
# GNU date gives milliseconds; elsewhere `%3N` is not expanded, so fall back to whole seconds.
now_ms() {
    t=$(date +%s%3N)
    case "$t" in *N*) t=$(($(date +%s) * 1000)) ;; esac
    printf '%s' "$t"
}
samples=""
run=1
while [ "$run" -le "$RUNS" ]; do
    cargo clean -p plugin --release >/dev/null 2>&1
    start=$(now_ms)
    cargo build --release -p plugin >/dev/null 2>&1 || die 5 "cargo build --release -p plugin failed"
    end=$(now_ms)
    ms=$((end - start))
    say "build run $run/$RUNS: ${ms} ms"
    samples="${samples}${samples:+,}$ms"
    run=$((run + 1))
done

# Summary figures (the reader recomputes them from samples as well).
stats=$(printf '%s\n' "$samples" | tr ',' '\n' | sort -n | awk '
    { v[NR] = $1 }
    END {
        n = NR
        if (n % 2) med = v[(n + 1) / 2]; else med = (v[n / 2] + v[n / 2 + 1]) / 2
        r = int(0.95 * n); if (r < 0.95 * n) r++; if (r < 1) r = 1
        printf "\"runs\": %d, \"median\": %s, \"p95\": %s, \"min\": %s, \"max\": %s", n, med, v[r], v[1], v[n]
    }')

binary="target/release/semantic-engine"
[ -f "$binary" ] || binary="target/release/semantic-engine.exe"
binary_bytes=$(wc -c <"$binary" 2>/dev/null | tr -d ' ')
[ -n "$binary_bytes" ] || binary_bytes=null

commit=$(git rev-parse HEAD 2>/dev/null || printf unknown)
[ -z "$(git status --porcelain --untracked-files=no 2>/dev/null)" ] || commit="$commit-dirty"
taken=$(date -u +%Y-%m-%dT%H:%M:%S.000Z)
host_target=$(rustc -vV | sed -n 's/^host: //p')

mkdir -p "$OUT"
path="$OUT/build-release.json"
[ -e "$path" ] && path="$OUT/build-release-$(date -u +%Y%m%dT%H%M%SZ).json"

cat >"$path" <<JSON
{
  "id": "build-release",
  "question": "build",
  "variant": "release",
  "kind": "build",
  "command": "scripts/spike-build-cost.sh --runs $RUNS --release $RELEASE",
  "server": { "stashVersion": null, "host": "local" },
  "library": { "kind": "test", "scenes": 0, "images": 0, "performers": 0, "tags": 0, "triples": 0 },
  "samples": [$samples],
  $stats,
  "failures": [],
  "takenAt": "$taken",
  "engineCommit": "$commit",
  "extra": {
    "samplesAre": "local clean release builds of the plugin (cargo clean -p plugin), ms",
    "hostTarget": "$host_target",
    "pluginBinaryBytes": $binary_bytes,
    "releaseAssets": { "tag": "$RELEASE", "assets": $assets },
    "ciJobs": $ci,
    "releaseJobs": $release
  }
}
JSON
say "wrote $path"
