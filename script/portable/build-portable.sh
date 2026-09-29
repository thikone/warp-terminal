#!/usr/bin/env bash
#
# Build the portable terminal-only binary for Linux or macOS.
#
# The counterpart of script/windows/build-portable.ps1, and deliberately the
# same shape: it owns the flags that are easy to get wrong, then verifies them
# against the linked binary so a bad build fails here rather than in someone's
# hands.
#
# Notable differences from the Windows build:
#
#   * --features release_bundle, without `gui`. `gui = ["voice_input"]` does not
#     gate the GUI front-end, so a terminal-only build has no use for it and
#     dropping it keeps ALSA out of the artifact. release_bundle still matters:
#     it enables rust-embed's debug-embed, without which assets load from
#     absolute build-machine paths and the binary is not portable.
#
#   * CARGO_PROFILE_RLTO_DEBUG=0. ThinLTO with debuginfo needs more than 16GB
#     and the final rustc gets OOM-killed -- on a 16GB VM and on GitHub's
#     runners alike. Line tables are the dominant consumer. The cost is that
#     panics carry addresses rather than file:line; Windows keeps debug=1.
#
#   * On macOS the binary is ad-hoc codesigned. An unsigned arm64 Mach-O is
#     killed by the kernel, so this is required, not cosmetic. Only the binary
#     is signed -- no bundle resources are sealed -- so the app writing into a
#     sibling data/ directory at runtime cannot invalidate the signature.
#
# No .app bundle is produced: app/src/bin/oss.rs embeds its own Info.plist via
# embed_plist, so the bare binary already carries CFBundleIdentifier and friends.
#
# Set CARGO_TARGET_DIR if the checkout lives on a network or shared filesystem.
# Building into a VM share is slow and, if the share is also a Windows checkout,
# mixes artifacts of two platforms in one profile directory.
#
# Usage:
#   script/portable/build-portable.sh [--tag <tag>] [--skip-verify]

set -euo pipefail

TAG=""
SKIP_VERIFY=0

while [ $# -gt 0 ]; do
    case "$1" in
        --tag)         TAG="${2:?--tag needs a value}"; shift 2 ;;
        --skip-verify) SKIP_VERIFY=1; shift ;;
        -h|--help)     sed -n '2,36p' "$0" | sed 's/^# \?//'; exit 0 ;;
        *)             echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# shellcheck source=script/portable/release-tag.sh
. "$REPO_ROOT/script/portable/release-tag.sh"

case "$(uname -s)" in
    Linux)  PLATFORM=linux ;;
    Darwin) PLATFORM=macos ;;
    *)      echo "Unsupported platform: $(uname -s). Windows uses script/windows/build-portable.ps1." >&2; exit 1 ;;
esac

[ -n "$TAG" ] || TAG="$(next_release_tag)"
if ! is_release_tag "$TAG"; then
    echo "Tag '$TAG' does not match v0.<yyyy>.<MM>.<dd>.<HH>.<MM>.terminal_<NN>." >&2
    exit 1
fi

for tool in cargo protoc cc cmake pkg-config; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "Missing build dependency: $tool" >&2
        [ "$PLATFORM" = linux ] && echo "Run script/linux/install_build_deps." >&2
        exit 1
    }
done

export CARGO_FULL_PROFILE=rlto
export CARGO_BIN_NAME=stable
export WARP_APP_NAME=WarpOss
export GIT_RELEASE_TAG="$TAG"
export CARGO_PROFILE_RLTO_DEBUG=0
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"

echo "Building $TAG"
echo "  platform    $PLATFORM"
echo "  target dir  $CARGO_TARGET_DIR"
echo "  debuginfo   off (ThinLTO exceeds 16GB with it on)"
echo

started=$(date +%s)
cargo build -p warp --profile rlto --bin warp-oss --features release_bundle
elapsed=$(( $(date +%s) - started ))

BIN="$CARGO_TARGET_DIR/rlto/warp-oss"
[ -f "$BIN" ] || { echo "Build reported success but $BIN is missing." >&2; exit 1; }

# Signing precedes verification: on Apple Silicon an unsigned binary cannot be
# executed at all, so --version would fail before it could be checked.
if [ "$PLATFORM" = macos ]; then
    codesign --force --sign - --timestamp=none "$BIN"
    codesign --verify --strict "$BIN"
    echo "  ad-hoc signed"
fi

if [ "$SKIP_VERIFY" -eq 0 ]; then
    reported="$("$BIN" --version 2>&1 || true)"
    case "$reported" in
        *"$TAG"*) ;;
        *) echo "Binary reports '$reported', which does not contain '$TAG'. GIT_RELEASE_TAG did not reach the build." >&2; exit 1 ;;
    esac
fi

size_mb=$(( $(wc -c < "$BIN") / 1048576 ))
echo
echo "Build OK"
echo "  $BIN"
echo "  ${size_mb} MB   $TAG   $((elapsed / 60))m$((elapsed % 60))s"
[ "$SKIP_VERIFY" -eq 0 ] && echo "  verified: --version reports the tag"
echo
echo "Package it with:"
echo "  script/portable/package-release.sh"
