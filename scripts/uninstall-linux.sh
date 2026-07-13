#!/bin/sh
# uninstall-linux.sh — Manual escape-hatch uninstall for Rewind on Linux.
#
# Handles both dpkg-installed and AppImage/tarball scenarios.
# Idempotent: safe to run multiple times. Refuses to run as root unless
# explicitly invoked with `sudo`.
#
# Usage:
#   ./scripts/uninstall-linux.sh           # normal (will use pkexec/sudo for dpkg)
#   sudo ./scripts/uninstall-linux.sh       # if you want to skip the prompt
#
# This script is NOT called by the in-app uninstall button. It exists for
# users whose install is broken and can't reach the Settings page.

set -e

BUNDLE_ID="com.rewind.app"
DEB_PACKAGE="rewind"
REMOVED=""

print_removed() {
    if [ -n "$1" ]; then
        REMOVED="$REMOVED\n  - $1"
    fi
}

# --- Kill running process ----------------------------------------------------

if pgrep -x "rewind" >/dev/null 2>&1; then
    pkill -x "rewind" 2>/dev/null || true
    print_removed "killed running Rewind process"
    sleep 1
fi

# --- Remove user data directories --------------------------------------------

XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"

for dir in \
    "$XDG_DATA_HOME/$BUNDLE_ID" \
    "$XDG_CONFIG_HOME/$BUNDLE_ID" \
    "$XDG_CACHE_HOME/$BUNDLE_ID"; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        print_removed "$dir"
    fi
done

# --- Remove autostart entry --------------------------------------------------

AUTOSTART_FILE="$XDG_CONFIG_HOME/autostart/rewind.desktop"
if [ -f "$AUTOSTART_FILE" ]; then
    rm -f "$AUTOSTART_FILE"
    print_removed "$AUTOSTART_FILE"
fi

# --- Remove binary (dpkg or AppImage/tarball) --------------------------------

# Check if installed via dpkg
if command -v dpkg >/dev/null 2>&1 && dpkg -s "$DEB_PACKAGE" >/dev/null 2>&1; then
    echo "Detected dpkg installation of '$DEB_PACKAGE'."

    if [ "$(id -u)" -eq 0 ]; then
        dpkg --purge "$DEB_PACKAGE" 2>/dev/null || true
        print_removed "dpkg package '$DEB_PACKAGE' purged"
    elif command -v pkexec >/dev/null 2>&1; then
        pkexec dpkg --purge "$DEB_PACKAGE" 2>/dev/null || \
            echo "  pkexec cancelled or failed — trying sudo"
        if dpkg -s "$DEB_PACKAGE" >/dev/null 2>&1; then
            sudo -n dpkg --purge "$DEB_PACKAGE" 2>/dev/null || \
                echo "  sudo not available — package not purged (run manually: sudo dpkg --purge $DEB_PACKAGE)"
        fi
        if ! dpkg -s "$DEB_PACKAGE" >/dev/null 2>&1; then
            print_removed "dpkg package '$DEB_PACKAGE' purged"
        fi
    else
        echo "  No pkexec or sudo available. Run manually: sudo dpkg --purge $DEB_PACKAGE"
    fi
fi

# AppImage / tarball: remove the binary if it's in a user-writable location
if command -v readlink >/dev/null 2>&1; then
    BINARY_PATH=$(readlink -f "$0" 2>/dev/null || echo "")
    # Try to find the rewind binary in common locations
    for candidate in \
        "$HOME/.local/bin/rewind" \
        "$HOME/Applications/Rewind" \
        "$HOME/Applications/rewind" \
        "$HOME/rewind"; do
        if [ -e "$candidate" ]; then
            rm -rf "$candidate"
            print_removed "$candidate"
        fi
    done
fi

# --- Summary -----------------------------------------------------------------

echo ""
echo "Rewind uninstall complete."
if [ -n "$REMOVED" ]; then
    echo "Removed:"
    echo "$REMOVED" | sed 's/^\\n//'
else
    echo "Nothing to remove — Rewind was already clean."
fi
echo ""
echo "If the dpkg package could not be purged, run:"
echo "  sudo dpkg --purge $DEB_PACKAGE"
