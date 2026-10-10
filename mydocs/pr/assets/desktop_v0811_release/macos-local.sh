#!/bin/bash
# HanPage Desktop 초안 자산의 macOS 로컬 서명·공증 확인(실행하지 않음).
# usage: macos-local.sh <assetDir> <version>
set -u
D="$1"; V="$2"
W=$(mktemp -d "${TMPDIR:-/tmp}/hp-mac-XXXXXX")
trap 'hdiutil detach -quiet "$W/mnt" 2>/dev/null; rm -rf "$W"' EXIT
check_app() {
  local label="$1" app="$2"
  echo "== $label"
  codesign --verify --deep --strict --verbose=2 "$app" 2>&1 | tail -2
  codesign -dv --verbose=4 "$app" 2>&1 | grep -E '^(Authority=Developer ID Application|TeamIdentifier=|Identifier=)'
  spctl -a -vvv -t exec "$app" 2>&1
  xcrun stapler validate "$app" 2>&1 | tail -1
  echo "version=$(plutil -extract CFBundleShortVersionString raw "$app/Contents/Info.plist") id=$(plutil -extract CFBundleIdentifier raw "$app/Contents/Info.plist")"
  local exe; exe=$(plutil -extract CFBundleExecutable raw "$app/Contents/Info.plist")
  echo "arch=$(lipo -archs "$app/Contents/MacOS/$exe")"
}
mkdir -p "$W/upd" && tar -xzf "$D/HanPage_aarch64.app.tar.gz" -C "$W/upd"
check_app "updater .app" "$W/upd/HanPage.app"
echo "== DMG"
hdiutil verify "$D/HanPage_${V}_aarch64.dmg" 2>&1 | tail -1
mkdir -p "$W/mnt" && hdiutil attach -quiet -nobrowse -readonly -mountpoint "$W/mnt" "$D/HanPage_${V}_aarch64.dmg"
check_app "DMG HanPage.app" "$W/mnt/HanPage.app"
