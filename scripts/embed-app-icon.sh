#!/usr/bin/env bash
# Compile assets/macos/AppIcon.icon into Assets.car and set CFBundleIconName
# so macOS 26 / Raycast use the Icon Composer icon instead of letterboxing the ICNS.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
icon="$root/assets/macos/AppIcon.icon"
app="${1:-$root/target/release/bundle/osx/Angry Hub.app}"

if [[ "$(uname -s)" != Darwin ]]; then
    echo "embed-app-icon: skipping (not macOS)"
    exit 0
fi
if [[ ! -d "$icon" ]]; then
    echo "error: missing Icon Composer file: $icon" >&2
    exit 1
fi
if [[ ! -d "$app" ]]; then
    echo "error: missing app bundle: $app" >&2
    exit 1
fi
if ! xcrun --find actool >/dev/null 2>&1; then
    echo "error: actool is required (install Xcode)" >&2
    exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

xcrun actool "$icon" \
    --compile "$tmp" \
    --output-format human-readable-text \
    --notices --warnings --errors \
    --output-partial-info-plist "$tmp/partial.plist" \
    --app-icon AppIcon \
    --include-all-app-icons \
    --enable-on-demand-resources NO \
    --development-region en \
    --target-device mac \
    --minimum-deployment-target 13.0 \
    --platform macosx \
    --enable-icon-stack-fallback-generation=disabled

if [[ ! -f "$tmp/Assets.car" ]]; then
    echo "error: actool did not produce Assets.car" >&2
    exit 1
fi

cp "$tmp/Assets.car" "$app/Contents/Resources/Assets.car"

plist="$app/Contents/Info.plist"
if /usr/libexec/PlistBuddy -c 'Print :CFBundleIconName' "$plist" >/dev/null 2>&1; then
    /usr/libexec/PlistBuddy -c 'Set :CFBundleIconName AppIcon' "$plist"
else
    /usr/libexec/PlistBuddy -c 'Add :CFBundleIconName string AppIcon' "$plist"
fi

echo "Embedded AppIcon (Assets.car) into $app"
