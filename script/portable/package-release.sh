#!/usr/bin/env bash
#
# Stage and archive a portable terminal-only release for Linux or macOS.
#
# The counterpart of script/windows/package-release.ps1, and the same contract:
# the tag is read out of the built binary rather than taken as an argument, so an
# archive can never claim a version its binary does not have.
#
# Unlike the Windows script this does not write release notes. A release spans
# three platforms and the notes belong to the release, not to one artifact, so
# they are generated once -- see script/windows/package-release.ps1.
#
# No git tag is created and nothing is pushed.
#
# Usage:
#   script/portable/package-release.sh [--build] [--tag <tag>] [--out <dir>]
#
#   --build   build first, deriving the next unused tag (or --tag)
#   --out     output directory, default build/pkg-<MMdd>

set -euo pipefail

DO_BUILD=0
TAG=""
OUT_DIR=""

while [ $# -gt 0 ]; do
    case "$1" in
        --build)   DO_BUILD=1; shift ;;
        --tag)     TAG="${2:?--tag needs a value}"; shift 2 ;;
        --out)     OUT_DIR="${2:?--out needs a value}"; shift 2 ;;
        -h|--help) sed -n '2,19p' "$0" | sed 's/^# \?//'; exit 0 ;;
        *)         echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# shellcheck source=script/portable/release-tag.sh
. "$REPO_ROOT/script/portable/release-tag.sh"

case "$(uname -s)" in
    Linux)  PLATFORM=linux ;;
    Darwin) PLATFORM=macos ;;
    *)      echo "Unsupported platform: $(uname -s)." >&2; exit 1 ;;
esac

case "$(uname -m)" in
    x86_64)          ARCH=x64 ;;
    aarch64|arm64)   ARCH=arm64 ;;
    *)               echo "Unsupported architecture: $(uname -m)." >&2; exit 1 ;;
esac

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"

if [ "$DO_BUILD" -eq 1 ]; then
    if [ -n "$TAG" ]; then
        "$REPO_ROOT/script/portable/build-portable.sh" --tag "$TAG"
    else
        "$REPO_ROOT/script/portable/build-portable.sh"
    fi
fi

BIN="$CARGO_TARGET_DIR/rlto/warp-oss"
[ -f "$BIN" ] || {
    echo "No binary at $BIN. Run build-portable.sh first, or pass --build." >&2
    exit 1
}

BINARY_TAG="$(binary_release_tag "$BIN")"
if ! is_release_tag "$BINARY_TAG"; then
    echo "Could not read a release tag from '$BIN --version'. It was built without GIT_RELEASE_TAG -- rebuild with build-portable.sh." >&2
    exit 1
fi

# An unsigned arm64 Mach-O cannot execute, so --version having worked already
# proves it is signed. Check anyway on macOS: a signature that is present but
# invalid fails later, at launch, on the user's machine.
if [ "$PLATFORM" = macos ]; then
    codesign --verify --strict "$BIN" || {
        echo "Binary is not validly signed. Rebuild with build-portable.sh." >&2
        exit 1
    }
fi

date_part="$(printf '%s' "$BINARY_TAG" | cut -d. -f3,4 | tr -d '.')"
[ -n "$OUT_DIR" ] || OUT_DIR="$REPO_ROOT/build/pkg-$date_part"
NAME="warp-terminal-$BINARY_TAG-$PLATFORM-$ARCH"
STAGE="$OUT_DIR/$NAME"

echo
echo "Packaging $BINARY_TAG"
echo "  platform  $PLATFORM-$ARCH"
echo "  binary    $(( $(wc -c < "$BIN") / 1048576 )) MB"

rm -rf "$STAGE"
mkdir -p "$STAGE/data"

install -m 755 "$BIN" "$STAGE/warp-oss"

cat > "$STAGE/data/README.txt" <<'EOF'
This folder makes Warp portable: its presence next to the warp-oss binary
redirects settings, state, cache and logs here instead of your home directory.

Delete this folder to use the normal per-user locations instead.
EOF

# Desktop integration is a Linux notion; on macOS the embedded Info.plist in the
# binary already supplies the app identity, so there is nothing to install.
if [ "$PLATFORM" = linux ]; then
    install -m 755 "$REPO_ROOT/script/portable/install.sh"   "$STAGE/install.sh"
    install -m 755 "$REPO_ROOT/script/portable/uninstall.sh" "$STAGE/uninstall.sh"
    install -m 644 "$REPO_ROOT/app/channels/stable/icon/no-padding/256x256.png" "$STAGE/icon.png"
fi

# Nothing is copied from a developer's own state directories, so there is no
# settings file, database or log to leak. Assert it rather than assume it.
leaked="$(find "$STAGE" -type f \( -name 'settings.toml' -o -name 'keybindings.yaml' \
    -o -name 'warp.sqlite*' -o -name '*.log' -o -name '*telemetry*' \) -print)"
if [ -n "$leaked" ]; then
    rm -rf "$STAGE"
    echo "User data would have shipped; staging discarded:" >&2
    printf '  %s\n' "$leaked" >&2
    exit 1
fi

ARCHIVE="$OUT_DIR/$NAME.tar.gz"
rm -f "$ARCHIVE"
# Deterministic member order, and paths relative to the staging parent so the
# archive unpacks into a single directory.
tar -czf "$ARCHIVE" -C "$OUT_DIR" "$NAME"

SHA="$(sha256_of "$ARCHIVE")"

echo "  staged    $(find "$STAGE" -type f | wc -l | tr -d ' ') files, no user data"
echo
echo "Done"
echo "  archive   $ARCHIVE"
echo "  size      $(( $(wc -c < "$ARCHIVE") / 1048576 )) MB"
echo "  sha256    $SHA"
echo "  tag       $BINARY_TAG"
echo "  next      $(next_release_tag)   (for the release after this one)"

branch="$(git rev-parse --abbrev-ref HEAD)"
if git rev-parse --verify --quiet "origin/$branch" >/dev/null 2>&1; then
    unpushed="$(git log --oneline "origin/$branch..HEAD" || true)"
    if [ -n "$unpushed" ]; then
        echo
        echo "WARNING: $(printf '%s\n' "$unpushed" | wc -l | tr -d ' ') commit(s) not on origin/$branch."
        echo "Push before publishing, or the release tag will point at a commit nobody can fetch:"
        printf '    %s\n' "$unpushed"
    fi
else
    echo
    echo "WARNING: no origin/$branch ref locally, so unpushed commits could not be checked."
fi
