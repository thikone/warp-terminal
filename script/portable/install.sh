#!/usr/bin/env bash
#
# Optional convenience setup for the portable terminal-only Warp build on Linux.
#
# The folder runs as-is -- this script is not required. It only adds what a real
# package would provide, all under $HOME so no root is needed and nothing outside
# your profile is touched:
#
#   * a desktop entry, so Warp appears in the application launcher
#   * the icon the entry points at
#   * optionally a ~/.local/bin symlink (--path) so `warp-oss` runs from any shell
#
# Run uninstall.sh to reverse it.
#
# Usage: ./install.sh [--path] [--quiet]

set -euo pipefail

ADD_TO_PATH=0
QUIET=0

while [ $# -gt 0 ]; do
    case "$1" in
        --path)    ADD_TO_PATH=1; shift ;;
        --quiet)   QUIET=1; shift ;;
        -h|--help) sed -n '2,17p' "$0" | sed 's/^# \?//'; exit 0 ;;
        *)         echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HERE/warp-oss"
[ -x "$BIN" ] || { echo "warp-oss not found or not executable next to this script." >&2; exit 1; }

APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps"
DESKTOP="$APPS_DIR/warp-oss.desktop"

mkdir -p "$APPS_DIR" "$ICON_DIR"

if [ -f "$HERE/icon.png" ]; then
    install -m 644 "$HERE/icon.png" "$ICON_DIR/warp-oss.png"
fi

# Exec is an absolute path rather than a bare name: the whole point of a portable
# folder is that it need not be on PATH, and the entry must keep working if it is
# not. StartupWMClass lets the launcher match the running window to this entry.
cat > "$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=Warp (terminal-only)
Comment=Stripped-down, portable build of the Warp terminal
Exec=$BIN
Icon=warp-oss
Terminal=false
Categories=System;TerminalEmulator;
Keywords=terminal;shell;console;
StartupWMClass=warp-oss
EOF
chmod 644 "$DESKTOP"

# Best-effort: the launcher usually notices on its own, and a missing tool here
# must not fail the install.
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" 2>/dev/null || true
command -v gtk-update-icon-cache  >/dev/null 2>&1 && \
    gtk-update-icon-cache -q -t "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor" 2>/dev/null || true

SYMLINK=""
if [ "$ADD_TO_PATH" -eq 1 ]; then
    BIN_DIR="$HOME/.local/bin"
    mkdir -p "$BIN_DIR"
    ln -sf "$BIN" "$BIN_DIR/warp-oss"
    SYMLINK="$BIN_DIR/warp-oss"
fi

if [ "$QUIET" -eq 0 ]; then
    echo "Installed:"
    echo "  desktop entry  $DESKTOP"
    [ -f "$ICON_DIR/warp-oss.png" ] && echo "  icon           $ICON_DIR/warp-oss.png"
    [ -n "$SYMLINK" ] && echo "  symlink        $SYMLINK"
    echo
    echo "The folder still runs directly: $BIN"
    if [ -n "$SYMLINK" ] && ! printf '%s' ":$PATH:" | grep -q ":$HOME/.local/bin:"; then
        echo
        echo "Note: $HOME/.local/bin is not on your PATH, so the symlink will not be found yet."
    fi
    echo "Reverse all of this with: $HERE/uninstall.sh"
fi
