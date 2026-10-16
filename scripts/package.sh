#!/bin/sh
# Build a release package of the plugin for this machine (or --target TRIPLE).
# Output: dist/semantic-engine-<version>-<target>.tar.gz and a matching .sha256 file.
# POSIX sh. Windows packages are built with scripts/package.ps1.
set -eu

usage() {
    echo "usage: scripts/package.sh [--target TRIPLE] [--out DIR]"
}

target=""
out="dist"
while [ $# -gt 0 ]; do
    case "$1" in
        --target) target="${2:?}"; shift 2 ;;
        --out) out="${2:?}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if [ -z "$target" ]; then
    target=$(rustc -vV | sed -n 's/^host: //p')
fi

cargo build --release --locked -p plugin --target "$target"
bin="target/$target/release/semantic-engine"
[ -x "$bin" ] || { echo "build did not produce $bin" >&2; exit 1; }

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/plugin/Cargo.toml | head -n 1)
name="semantic-engine-$version-$target"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT INT TERM

dest="$stage/semantic-engine"
mkdir -p "$dest/vocabulary"
sed "s/^version: .*/version: $version/" plugin/semantic-engine.yml > "$dest/semantic-engine.yml"
cp "$bin" "$dest/semantic-engine"
cp vocabulary/*.ttl vocabulary/CHANGELOG.md vocabulary/TERMS.md "$dest/vocabulary/"
# A cross-compiled binary may not run here (for example x86_64 macOS built on arm64 without
# Rosetta). The versions are the same for every target except build.target, so fall back to a
# host build and substitute the target.
if ! "$bin" --versions > "$dest/VERSIONS.json" 2>/dev/null; then
    host=$(rustc -vV | sed -n 's/^host: //p')
    cargo build --release --locked -p plugin
    "target/release/semantic-engine" --versions | sed "s/\"$host\"/\"$target\"/" > "$dest/VERSIONS.json"
fi

mkdir -p "$out"
archive="$out/$name.tar.gz"
(cd "$stage" && tar -czf - semantic-engine) > "$archive"

if command -v sha256sum >/dev/null 2>&1; then
    sum=$(sha256sum "$archive" | cut -d' ' -f1)
elif command -v shasum >/dev/null 2>&1; then
    sum=$(shasum -a 256 "$archive" | cut -d' ' -f1)
else
    sum=$(sha256 -q "$archive")
fi
printf '%s  %s\n' "$sum" "$name.tar.gz" > "$archive.sha256"
echo "$archive"
