#!/bin/bash
# Local host-architecture bundle, not a notarized distribution.
set -euo pipefail
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS is required.' >&2; exit 1; }
cd "$(dirname "$0")/.."
for tool in cargo rustc xcrun iconutil plutil codesign otool swift; do
    command -v "$tool" >/dev/null || { echo "Missing prerequisite: $tool" >&2; exit 1; }
done
xcrun --show-sdk-path >/dev/null
mkdir -p target/macos
if [[ "${1:-}" == --icon-only ]]; then
    work=$(mktemp -d "$PWD/target/macos/icon.XXXXXX")
    mkdir "$work/AppIcon.iconset"
    swift assets/macos/generate-icon.swift "$work/AppIcon.iconset"
    iconutil -c icns "$work/AppIcon.iconset" -o assets/macos/AppIcon.icns
    echo 'Generated assets/macos/AppIcon.icns; intermediates retained in target/macos.'
    exit 0
fi
[[ $# == 0 ]] || { echo "Usage: $0 [--icon-only]" >&2; exit 1; }
# Conservative floor: build host version, not an untested older macOS.
minimum=$(/usr/bin/sw_vers -productVersion)
export MACOSX_DEPLOYMENT_TARGET="$minimum"
host=$(rustc -vV | sed -n 's/^host: //p')
version=$(sed -n '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' Cargo.toml)
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Expected numeric package version.' >&2; exit 1; }
cargo build --release --locked --target "$host" --target-dir "$PWD/target"
work=$(mktemp -d "$PWD/target/macos/build.XXXXXX")
app="$work/Storage Scanner.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$host/release/storage-scanner" "$app/Contents/MacOS/"
cp assets/macos/AppIcon.icns "$app/Contents/Resources/"
cp packaging/macos/Info.plist "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :LSMinimumSystemVersion $minimum" "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
otool -L "$app/Contents/MacOS/storage-scanner"
while IFS= read -r library; do
    case "$library" in /System/Library/*|/usr/lib/*) ;; *) echo "Unbundled dependency: $library" >&2; exit 1 ;; esac
done < <(otool -L "$app/Contents/MacOS/storage-scanner" | tail -n +2 | awk '{print $1}')
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
# Retain previous builds rather than delete or overwrite bundles.
output="$PWD/target/macos/Storage Scanner.app"
if [[ -e "$output" ]]; then
    output="$PWD/target/macos/Storage Scanner-$(date +%Y%m%d-%H%M%S)-$$.app"
fi
mv "$app" "$output"
echo "Built: $output"
echo "Architecture: $host; minimum macOS: $minimum; ad-hoc signed (not notarized)."
