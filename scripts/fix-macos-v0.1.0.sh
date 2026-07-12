#!/usr/bin/env bash
# fix-macos-v0.1.0.sh
# One-shot hotfix for Rewind v0.1.0 macOS installs that crash with
# "App quit unexpectedly" on launch.
#
# Root cause: the v0.1.0 .app ships with zero entitlements, which causes
# the WKWebView/JIT initialization to fail on macOS Sequoia 15+. v0.1.1
# fixes this at build time. This script patches the v0.1.0 install in
# place: strips the quarantine xattr and ad-hoc re-signs the .app with
# the entitlements required to run on Sequoia.
#
# Note: this script CANNOT fix the second v0.1.0 failure mode (a relative
# tray-icon path that resolves against the launch CWD). That bug lives in
# the binary and requires v0.1.1. The entitlement re-sign is enough to
# get v0.1.0 past the Setup crashed panic on most systems, but if you
# still see "App quit unexpectedly" after running this script, install
# v0.1.1 from the releases page.
#
# Usage:
#   ./fix-macos-v0.1.0.sh /Applications/Rewind.app
#   ./fix-macos-v0.1.0.sh                    # default: /Applications/Rewind.app
set -euo pipefail

APP="${1:-/Applications/Rewind.app}"

ENTITLEMENTS='<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
</dict>
</plist>'

if [ ! -d "$APP" ]; then
    echo "Error: $APP is not a directory." >&2
    echo "Usage: $0 /path/to/Rewind.app" >&2
    exit 1
fi

echo "Patching $APP ..."

# Strip quarantine + extended attributes so Gatekeeper does not re-block
# the freshly-signed binary before the next launch.
xattr -cr "$APP"

# Write the entitlements plist to a temp file (codesign --entitlements
# only accepts paths, not stdin).
TMP_ENT=$(mktemp -t rewind-entitlements.XXXXXX.plist)
trap 'rm -f "$TMP_ENT"' EXIT
printf '%s\n' "$ENTITLEMENTS" > "$TMP_ENT"

# Ad-hoc sign with entitlements. --deep walks the .app bundle and re-signs
# every nested binary (Rewind.app/Contents/MacOS/rewind, helper processes,
# bundled dylibs). --sign - uses the ad-hoc identity, which is enough for
# local use on the user's own machine.
codesign --force --deep --sign - --entitlements "$TMP_ENT" "$APP"

echo ""
echo "OK Fixed $APP"
echo "Try launching it: open \"$APP\""
echo ""
echo "If Rewind still crashes after this, the binary itself is broken"
echo "(tray-icon path bug in v0.1.0). Install v0.1.1 from"
echo "https://github.com/xiaoxinny/Rewind/releases instead."