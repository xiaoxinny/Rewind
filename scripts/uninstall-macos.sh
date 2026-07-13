#!/bin/sh
# uninstall-macos.sh — Manual escape-hatch uninstall for Rewind on macOS.
#
# Removes the .app bundle, user data directories, LaunchAgent plist, and
# autostart entry. Idempotent: safe to run multiple times.
#
# Usage:
#   ./scripts/uninstall-macos.sh
#
# This script is NOT called by the in-app uninstall button. It exists for
# users whose install is broken and can't reach the Settings page.

set -e

BUNDLE_ID="com.rewind.app"
APP_BUNDLE="/Applications/Rewind.app"
REMOVED=""

print_removed() {
    if [ -n "$1" ]; then
        REMOVED="$REMOVED\n  - $1"
    fi
}

# --- Kill running process ----------------------------------------------------

if pgrep -f "Rewind.app" >/dev/null 2>&1; then
    pkill -f "Rewind.app" 2>/dev/null || true
    print_removed "killed running Rewind process"
    sleep 1
fi

# --- Remove .app bundle ------------------------------------------------------

if [ -d "$APP_BUNDLE" ]; then
    rm -rf "$APP_BUNDLE"
    print_removed "$APP_BUNDLE"
fi

# Also check ~/Applications
LOCAL_APP="$HOME/Applications/Rewind.app"
if [ -d "$LOCAL_APP" ]; then
    rm -rf "$LOCAL_APP"
    print_removed "$LOCAL_APP"
fi

# --- Remove user data directories --------------------------------------------

DATA_DIR="$HOME/Library/Application Support/$BUNDLE_ID"
if [ -d "$DATA_DIR" ]; then
    rm -rf "$DATA_DIR"
    print_removed "$DATA_DIR"
fi

CACHE_DIR="$HOME/Library/Caches/$BUNDLE_ID"
if [ -d "$CACHE_DIR" ]; then
    rm -rf "$CACHE_DIR"
    print_removed "$CACHE_DIR"
fi

# --- Remove LaunchAgent plist (autostart) ------------------------------------

PLIST="$HOME/Library/Preferences/$BUNDLE_ID.plist"
if [ -f "$PLIST" ]; then
    rm -f "$PLIST"
    print_removed "$PLIST"
fi

LAUNCH_AGENT="$HOME/Library/LaunchAgents/$BUNDLE_ID.plist"
if [ -f "$LAUNCH_AGENT" ]; then
    # Unload the agent if it's running
    launchctl unload "$LAUNCH_AGENT" 2>/dev/null || true
    rm -f "$LAUNCH_AGENT"
    print_removed "$LAUNCH_AGENT"
fi

# --- Remove saved application state -----------------------------------------

SAVED_STATE="$HOME/Library/Saved Application State/$BUNDLE_ID.savedState"
if [ -d "$SAVED_STATE" ]; then
    rm -rf "$SAVED_STATE"
    print_removed "$SAVED_STATE"
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
