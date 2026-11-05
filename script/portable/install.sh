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

# Libraries the binary dlopens rather than links. They never appear in `ldd`, so a
# missing one shows up only when the window fails to open -- which is how
# libxkbcommon-x11-0 was found the hard way. Names on the right are Debian/Ubuntu;
# other distributions ship the same sonames under their own package names.
RUNTIME_LIBS="
libX11.so.6:libx11-6
libX11-xcb.so.1:libx11-xcb1
libxcb.so.1:libxcb1
libXi.so.6:libxi6
libXcursor.so.1:libxcursor1
libxkbcommon.so.0:libxkbcommon0
libxkbcommon-x11.so.0:libxkbcommon-x11-0
libwayland-client.so.0:libwayland-client0
libwayland-egl.so.1:libwayland-egl1
libEGL.so.1:libegl1
libvulkan.so.1:libvulkan1
libfontconfig.so.1:fontconfig
"

# Warn rather than fail: adding shortcuts is still useful on a machine whose
# libraries are about to be installed.
check_runtime_libs() {
    command -v ldconfig >/dev/null 2>&1 || return 0

    local cache missing="" entry soname pkg
    cache="$(ldconfig -p 2>/dev/null || true)"
    [ -n "$cache" ] || return 0

    while IFS= read -r entry; do
        [ -n "$entry" ] || continue
        soname="${entry%%:*}"
        pkg="${entry##*:}"
        # Pure-bash match on purpose. `printf ... | grep -q` looks equivalent but
        # races: grep exits on the first hit, printf dies of SIGPIPE, and pipefail
        # reports 141 for the pipeline -- marking a library that IS present as
        # missing, depending on where in the cache it happened to appear.
        case "$cache" in
            *"$soname "*) ;;
            *) missing="$missing $pkg" ;;
        esac
    done <<EOF
$RUNTIME_LIBS
EOF

    [ -n "$missing" ] || return 0

    echo "Warning: warp-oss loads these at runtime and they are not installed:" >&2
    # shellcheck disable=SC2086
    printf '  %s
' $missing >&2
    echo >&2
    # shellcheck disable=SC2086
    echo "  sudo apt install$missing" >&2
    echo >&2
    echo "A Vulkan driver is needed too (mesa-vulkan-drivers on Mesa hardware); a" >&2
    echo "present libvulkan with no driver behind it fails the same way." >&2
    echo >&2
}

check_runtime_libs

APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps"
# The window identifies itself by the app ID from app/src/bin/oss.rs (Wayland
# app_id, X11 WM_CLASS). GNOME on Wayland matches a window to the entry whose file
# name is that ID, so both the file name and StartupWMClass must use it -- with
# anything else the running window falls back to a generic icon.
APP_ID="dev.warp.WarpOss"
DESKTOP="$APPS_DIR/$APP_ID.desktop"
# Name used by earlier versions of this script; removed so the launcher does not
# list Warp twice.
LEGACY_DESKTOP="$APPS_DIR/warp-oss.desktop"

mkdir -p "$APPS_DIR" "$ICON_DIR"
rm -f "$LEGACY_DESKTOP"

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
StartupWMClass=$APP_ID
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
    case ":$PATH:" in *":$HOME/.local/bin:"*) on_path=1 ;; *) on_path=0 ;; esac
    if [ -n "$SYMLINK" ] && [ "$on_path" -eq 0 ]; then
        echo
        echo "Note: $HOME/.local/bin is not on your PATH, so the symlink will not be found yet."
    fi
    echo "Reverse all of this with: $HERE/uninstall.sh"
fi
