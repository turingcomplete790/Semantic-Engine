#!/bin/sh
# Install the Semantic Engine Stash plugin on BSD, macOS, or Linux.
#
# POSIX sh only (no bash features), so it runs under dash, busybox sh, ash, and the BSD and
# macOS /bin/sh. Contract: specs/002-engine-plugin-foundation/contracts/install-scripts.md
#
# Exit codes: 0 installed, 2 bad arguments, 3 no package for this platform, 4 plugins folder not
# found or not writable, 5 download or checksum failure, 6 unpack or copy failure.

set -u

PLUGIN_DIR_NAME="semantic-engine"
# Where release packages are published. Empty until the project publishes releases; until then
# install a package you built or downloaded with --from-file.
RELEASE_URL="${SEMANTIC_ENGINE_RELEASE_URL:-}"
BUILD_DOCS="Build from source: install Rust, a C++17 compiler, and clang, then run
  scripts/package.sh in the source tree and install the result with --from-file."

usage() {
    cat <<'USAGE'
Install the Semantic Engine plugin into Stash's plugins folder.

usage: install.sh [--plugins-dir DIR] [--version X.Y.Z] [--from-file PACKAGE] [--yes]

  --plugins-dir DIR   Stash's plugins folder (default: from Stash's config.yml)
  --version X.Y.Z     version to download (default: latest)
  --from-file PKG     install a package already on disk; PKG.sha256 must sit beside it
  --yes               do not ask for confirmation
  -h, --help          show this help

Nothing outside <plugins>/semantic-engine is changed. Reload plugins in Stash afterwards
(Settings > Plugins > Reload plugins).
USAGE
}

say() { printf '%s\n' "$*"; }
die() { code=$1; shift; printf 'error: %s\n' "$*" >&2; exit "$code"; }

plugins_dir=""
version="latest"
from_file=""
assume_yes=0
while [ $# -gt 0 ]; do
    case "$1" in
        --plugins-dir) [ $# -ge 2 ] || die 2 "--plugins-dir needs a value"; plugins_dir=$2; shift 2 ;;
        --version) [ $# -ge 2 ] || die 2 "--version needs a value"; version=$2; shift 2 ;;
        --from-file) [ $# -ge 2 ] || die 2 "--from-file needs a value"; from_file=$2; shift 2 ;;
        --yes|-y) assume_yes=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; die 2 "unknown argument: $1" ;;
    esac
done

# ---- 1. Platform -> package target (research R7)
# SE_TEST_OS / SE_TEST_ARCH exist only for the test suite (busybox sh ignores a fake uname).
os=${SE_TEST_OS:-$(uname -s 2>/dev/null || echo unknown)}
arch=${SE_TEST_ARCH:-$(uname -m 2>/dev/null || echo unknown)}
case "$arch" in
    x86_64|amd64) arch_t=x86_64 ;;
    aarch64|arm64) arch_t=aarch64 ;;
    *) arch_t="" ;;
esac
target=""
case "$os" in
    Linux) [ -n "$arch_t" ] && target="$arch_t-unknown-linux-gnu" ;;
    Darwin) [ -n "$arch_t" ] && target="$arch_t-apple-darwin" ;;
esac
if [ -z "$target" ]; then
    say "No prebuilt Semantic Engine package exists for $os ($arch)." >&2
    say "Packages are published for Linux and macOS on x86_64 and aarch64, and for Windows x86_64." >&2
    [ -n "$RELEASE_URL" ] && say "Release page: $RELEASE_URL" >&2
    say "$BUILD_DOCS" >&2
    exit 3
fi

# ---- 2. Plugins folder (research R8)
config_plugins_dir() {
    for cfg in "./config.yml" "${HOME:-}/.stash/config.yml"; do
        [ -f "$cfg" ] || continue
        cfg_dir=$(cd "$(dirname "$cfg")" && pwd)
        p=$(sed -n 's/^plugins_path:[[:space:]]*//p' "$cfg" | sed 's/^["'\'']//; s/["'\'']$//' | head -n 1)
        if [ -n "$p" ]; then
            case "$p" in
                /*) printf '%s\n' "$p" ;;
                *) printf '%s/%s\n' "$cfg_dir" "$p" ;;
            esac
        else
            printf '%s/plugins\n' "$cfg_dir"
        fi
        return 0
    done
    return 1
}
if [ -z "$plugins_dir" ]; then
    plugins_dir=$(config_plugins_dir) || die 4 "cannot find Stash's config.yml; pass --plugins-dir"
fi
[ -d "$plugins_dir" ] || die 4 "plugins folder not found: $plugins_dir (pass --plugins-dir)"
[ -w "$plugins_dir" ] || die 4 "plugins folder is not writable: $plugins_dir"
plugins_dir=$(cd "$plugins_dir" && pwd)

# ---- 3. Package and checksum
work=$(mktemp -d 2>/dev/null) || die 6 "cannot create a temporary folder"
trap 'rm -rf "$work"' EXIT INT TERM

fetch() {
    if command -v curl >/dev/null 2>&1; then curl -fsSL -o "$2" "$1"
    elif command -v fetch >/dev/null 2>&1; then fetch -q -o "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then wget -q -O "$2" "$1"
    else return 1; fi
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
    elif command -v sha256 >/dev/null 2>&1; then sha256 -q "$1"
    else return 1; fi
}

if [ -n "$from_file" ]; then
    [ -f "$from_file" ] || die 5 "package not found: $from_file"
    [ -f "$from_file.sha256" ] || die 5 "checksum file not found: $from_file.sha256"
    package=$from_file
    expected=$(cut -d' ' -f1 < "$from_file.sha256")
else
    [ -n "$RELEASE_URL" ] || die 5 "downloads are not configured yet; install a package with --from-file"
    base="$RELEASE_URL/$version"
    sums="$work/SHA256SUMS"
    fetch "$base/SHA256SUMS" "$sums" || die 5 "cannot download $base/SHA256SUMS"
    file=$(grep "semantic-engine-.*-$target\.tar\.gz\$" "$sums" | head -n 1 | sed 's/^[0-9a-f]*[[:space:]]*\**//')
    [ -n "$file" ] || die 3 "no package for $target in release $version"
    expected=$(grep " \**$file\$" "$sums" | cut -d' ' -f1)
    package="$work/$file"
    fetch "$base/$file" "$package" || die 5 "cannot download $base/$file"
fi
actual=$(sha256_of "$package") || die 5 "no SHA-256 tool found (sha256sum, shasum, or sha256)"
[ "$actual" = "$expected" ] || die 5 "checksum mismatch for $package (expected $expected, got $actual)"

# ---- 4. Unpack and replace <plugins>/semantic-engine only
mkdir "$work/unpack" || die 6 "cannot create $work/unpack"
gzip -dc "$package" | (cd "$work/unpack" && tar -xf -) || die 6 "cannot unpack $package"
new="$work/unpack/$PLUGIN_DIR_NAME"
[ -f "$new/semantic-engine.yml" ] || die 6 "package has no $PLUGIN_DIR_NAME/semantic-engine.yml"

if [ "$assume_yes" -ne 1 ] && [ -t 0 ]; then
    printf 'Install into %s/%s? [y/N] ' "$plugins_dir" "$PLUGIN_DIR_NAME"
    read -r answer
    case "$answer" in y|Y|yes|YES) ;; *) say "cancelled"; exit 0 ;; esac
fi

dest="$plugins_dir/$PLUGIN_DIR_NAME"
staged="$plugins_dir/.$PLUGIN_DIR_NAME.new.$$"
old="$plugins_dir/.$PLUGIN_DIR_NAME.old.$$"
cp -R "$new" "$staged" || { rm -rf "$staged"; die 6 "cannot copy into $plugins_dir"; }
if [ -e "$dest" ]; then
    mv "$dest" "$old" || { rm -rf "$staged"; die 6 "cannot move the existing install aside"; }
fi
if ! mv "$staged" "$dest"; then
    [ -e "$old" ] && mv "$old" "$dest"
    rm -rf "$staged"
    die 6 "cannot move the new install into place; the previous install was restored"
fi
rm -rf "$old"
chmod +x "$dest/semantic-engine" 2>/dev/null || true

# ---- 5. Report
installed=$(sed -n 's/.*"engineVersion"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$dest/VERSIONS.json" 2>/dev/null | head -n 1)
say "Installed Semantic Engine ${installed:-(unknown version)} for $target into $dest"
say "Reload plugins in Stash: Settings > Plugins > Reload plugins."
exit 0
