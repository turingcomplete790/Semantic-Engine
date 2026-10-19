#!/bin/sh
# Install the measurement-only spike plugin (semantic-engine-spike) into a Stash plugins folder.
#
# POSIX sh only. Builds from the workspace; there is no package (specs/003-engine-design-spike,
# research R9). Refuses any plugins folder outside the test instance unless --force is given.
#
# Exit codes: 0 installed, 2 bad arguments, 3 refused (not the test instance), 4 plugins folder
# not found or not writable, 5 build failure, 6 copy failure.

set -u

PLUGIN_DIR_NAME="semantic-engine-spike"
TEST_INSTANCE="/home/bbulder/Projects/Stash-Plugins/Test-Instance/"

usage() {
    cat <<'USAGE'
Install the Phase 1 spike plugin into a Stash plugins folder (test instance only).

usage: spike-install.sh [--force] PLUGINS_DIR

  --force       allow a plugins folder outside the test instance
  -h, --help    show this help

Only <plugins>/semantic-engine-spike is changed. Reload plugins in Stash afterwards.
USAGE
}

say() { printf '%s\n' "$*"; }
die() { code=$1; shift; printf 'error: %s\n' "$*" >&2; exit "$code"; }

force=0
plugins_dir=""
while [ $# -gt 0 ]; do
    case "$1" in
        --force) force=1; shift ;;
        -h|--help) usage; exit 0 ;;
        -*) usage >&2; die 2 "unknown argument: $1" ;;
        *) [ -z "$plugins_dir" ] || die 2 "only one plugins folder"; plugins_dir=$1; shift ;;
    esac
done
[ -n "$plugins_dir" ] || { usage >&2; die 2 "PLUGINS_DIR is required"; }
[ -d "$plugins_dir" ] || die 4 "plugins folder not found: $plugins_dir"
[ -w "$plugins_dir" ] || die 4 "plugins folder not writable: $plugins_dir"

abs_dir=$(cd "$plugins_dir" && pwd -P) || die 4 "cannot resolve $plugins_dir"
case "$abs_dir/" in
    "$TEST_INSTANCE"*) ;;
    *) [ "$force" -eq 1 ] || die 3 "refusing $abs_dir: not under $TEST_INSTANCE (use --force)" ;;
esac

root=$(cd "$(dirname "$0")/.." && pwd -P) || die 5 "cannot find the workspace"
say "Building spike-plugin ..."
(cd "$root" && cargo build --release -p spike --bin spike-plugin) || die 5 "build failed"

bin="$root/target/release/spike-plugin"
[ -f "$bin" ] || bin="$bin.exe"
[ -f "$bin" ] || die 5 "built binary not found under $root/target/release"

dest="$abs_dir/$PLUGIN_DIR_NAME"
mkdir -p "$dest" || die 6 "cannot create $dest"
command cp -f "$bin" "$dest/" || die 6 "copy of the binary failed"
command cp -f "$root/spike/semantic-engine-spike.yml" "$dest/" || die 6 "copy of the manifest failed"
chmod +x "$dest/$(basename "$bin")" || die 6 "chmod failed"

say "Installed $PLUGIN_DIR_NAME into $dest"
say "Reload plugins in Stash (Settings > Plugins > Reload plugins)."
