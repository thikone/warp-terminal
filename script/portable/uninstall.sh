#!/usr/bin/env bash
#
# Reverse script/portable/install.sh.
#
# Removes only what install.sh created: the desktop entry, its icon, and the
# ~/.local/bin symlink if it points at this folder. The portable folder itself and
# anything under data/ are left alone -- deleting your settings and history is not
# this script's business.
#
# Usage: ./uninstall.sh [--quiet]

set -euo pipefail

QUIET=0
while [ $# -gt 0 ]; do
    case "$1" in
        --quiet)   QUIET=1; shift ;;
        -h|--help) sed -n '2,11p' "$0" | sed 's/^# \?//'; exit 0 ;;
        *)         echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP="$DATA_HOME/applications/dev.warp.WarpOss.desktop"
# Name used by earlier versions of install.sh.
LEGACY_DESKTOP="$DATA_HOME/applications/warp-oss.desktop"
ICON="$DATA_HOME/icons/hicolor/256x256/apps/warp-oss.png"
SYMLINK="$HOME/.local/bin/warp-oss"

removed=0
for f in "$DESKTOP" "$LEGACY_DESKTOP" "$ICON"; do
    if [ -f "$f" ]; then
        rm -f "$f"
        [ "$QUIET" -eq 0 ] && echo "removed  $f"
        removed=1
    fi
done

# Only remove a symlink that actually points into this folder; another install
# elsewhere has its own claim on that name.
if [ -L "$SYMLINK" ] && [ "$(readlink -f "$SYMLINK")" = "$(readlink -f "$HERE/warp-oss")" ]; then
    rm -f "$SYMLINK"
    [ "$QUIET" -eq 0 ] && echo "removed  $SYMLINK"
    removed=1
fi

command -v update-desktop-database >/dev/null 2>&1 && \
    update-desktop-database "$DATA_HOME/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && \
    gtk-update-icon-cache -q -t "$DATA_HOME/icons/hicolor" 2>/dev/null || true

if [ "$QUIET" -eq 0 ]; then
    [ "$removed" -eq 0 ] && echo "Nothing to remove."
    echo
    echo "The portable folder is untouched, including data/."
    echo "Per-user state, if you were not running portable, lives in:"
    echo "  ${XDG_CONFIG_HOME:-$HOME/.config}/warp-terminal  and  $HOME/.warp-oss"
fi
