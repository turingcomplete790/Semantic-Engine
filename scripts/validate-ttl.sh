#!/usr/bin/env sh
# Second-parser check: parse each vocabulary file with Raptor's rapper (FR-001, SC-002).
# Skips with a notice when rapper is not installed.
set -eu
dir="${1:-vocabulary}"
if ! command -v rapper >/dev/null 2>&1; then
    echo "notice: rapper not installed; second-parser check skipped" >&2
    exit 0
fi
status=0
for f in "$dir"/*.ttl; do
    out=$(rapper -i turtle -c "$f" 2>&1) || { echo "$out" >&2; status=1; continue; }
    if echo "$out" | grep -Eqi "warning|error"; then
        echo "$f: $out" >&2
        status=1
    else
        echo "$f: ok ($(echo "$out" | sed -n 's/.*returned \([0-9]*\) triples.*/\1/p') triples)"
    fi
done
exit $status
