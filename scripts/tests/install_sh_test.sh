#!/bin/sh
# Tests for scripts/install.sh. POSIX sh; run under dash and busybox sh:
#   dash scripts/tests/install_sh_test.sh   |   busybox sh scripts/tests/install_sh_test.sh
set -u
here=$(cd "$(dirname "$0")" && pwd)
script="$here/../install.sh"
shell=${TEST_SHELL:-sh}
work=$(mktemp -d)
trap 'chmod -R u+w "$work" 2>/dev/null; rm -rf "$work"' EXIT INT TERM
pass=0
fail=0

ok() { pass=$((pass + 1)); printf 'ok   %s\n' "$1"; }
bad() { fail=$((fail + 1)); printf 'FAIL %s\n' "$1"; }

sha() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
    else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# A fake package for this machine's target.
mkdir -p "$work/pkg/semantic-engine/vocabulary"
printf 'name: Semantic Engine\nversion: 9.9.9\n' > "$work/pkg/semantic-engine/semantic-engine.yml"
printf '#!/bin/sh\necho fake\n' > "$work/pkg/semantic-engine/semantic-engine"
chmod +x "$work/pkg/semantic-engine/semantic-engine"
printf '{"engineVersion":"9.9.9"}\n' > "$work/pkg/semantic-engine/VERSIONS.json"
pkg="$work/semantic-engine-9.9.9-test.tar.gz"
(cd "$work/pkg" && tar -czf "$pkg" semantic-engine)
printf '%s  %s\n' "$(sha "$pkg")" "$(basename "$pkg")" > "$pkg.sha256"

# 1. --help
if $shell "$script" --help >/dev/null 2>&1; then ok "--help exits 0"; else bad "--help exits 0"; fi

# 2. install from file
mkdir -p "$work/plugins"
echo keep > "$work/plugins/other-plugin.yml"
$shell "$script" --from-file "$pkg" --plugins-dir "$work/plugins" --yes >"$work/out1" 2>&1
code=$?
if [ "$code" -eq 0 ] && [ -f "$work/plugins/semantic-engine/semantic-engine.yml" ] \
    && [ -x "$work/plugins/semantic-engine/semantic-engine" ]; then
    ok "installs from file"
else
    bad "installs from file (exit $code)"; cat "$work/out1"
fi

# 3. re-run replaces files and leaves unrelated files alone
echo stale > "$work/plugins/semantic-engine/stale-file"
$shell "$script" --from-file "$pkg" --plugins-dir "$work/plugins" --yes >/dev/null 2>&1
if [ ! -e "$work/plugins/semantic-engine/stale-file" ] && [ "$(cat "$work/plugins/other-plugin.yml")" = keep ]; then
    ok "re-run replaces plugin files only"
else
    bad "re-run replaces plugin files only"
fi

# 4. checksum mismatch -> 5, nothing changed
cp "$pkg" "$work/bad.tar.gz"
printf '%s  bad.tar.gz\n' 0000000000000000000000000000000000000000000000000000000000000000 > "$work/bad.tar.gz.sha256"
mkdir -p "$work/plugins2"
$shell "$script" --from-file "$work/bad.tar.gz" --plugins-dir "$work/plugins2" --yes >/dev/null 2>&1
code=$?
if [ "$code" -eq 5 ] && [ -z "$(ls -A "$work/plugins2")" ]; then ok "bad checksum exits 5"; else bad "bad checksum exits 5 (exit $code)"; fi

# 5. unwritable plugins dir -> 4
mkdir -p "$work/ro"
chmod a-w "$work/ro"
if [ -w "$work/ro" ]; then
    ok "unwritable dir exits 4 (skipped: running as root)"
else
    $shell "$script" --from-file "$pkg" --plugins-dir "$work/ro" --yes >/dev/null 2>&1
    code=$?
    if [ "$code" -eq 4 ]; then ok "unwritable dir exits 4"; else bad "unwritable dir exits 4 (exit $code)"; fi
fi

# 6. missing plugins dir -> 4
$shell "$script" --from-file "$pkg" --plugins-dir "$work/does-not-exist" --yes >/dev/null 2>&1
code=$?
if [ "$code" -eq 4 ]; then ok "missing dir exits 4"; else bad "missing dir exits 4 (exit $code)"; fi

# 7. unsupported platform -> 3, message names platform and build-from-source
SE_TEST_OS=FreeBSD SE_TEST_ARCH=amd64 $shell "$script" --from-file "$pkg" --plugins-dir "$work/plugins" --yes >"$work/out7" 2>&1
code=$?
if [ "$code" -eq 3 ] && grep -q FreeBSD "$work/out7" && grep -qi "build from source" "$work/out7"; then
    ok "unsupported platform exits 3 with guidance"
else
    bad "unsupported platform exits 3 (exit $code)"; cat "$work/out7"
fi

# 8. bad arguments -> 2
$shell "$script" --bogus >/dev/null 2>&1
code=$?
if [ "$code" -eq 2 ]; then ok "bad arguments exit 2"; else bad "bad arguments exit 2 (exit $code)"; fi

printf '%s passed, %s failed (%s)\n' "$pass" "$fail" "$shell"
[ "$fail" -eq 0 ]
