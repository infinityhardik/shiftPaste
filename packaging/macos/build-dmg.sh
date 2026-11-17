#!/usr/bin/env bash
# Build "Shift Paste.app" (universal: Apple Silicon + Intel) and a .dmg.
# Usage: packaging/macos/build-dmg.sh <version>     (run on macOS from the repo root)
set -euo pipefail
VERSION="${1:?version}"
OUT=target/macos
rm -rf "$OUT" && mkdir -p "$OUT"

for t in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target add "$t" >/dev/null
  cargo build --release -p shiftpaste --target "$t"
done
APP="$OUT/Shift Paste.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/shiftpaste" \
  target/aarch64-apple-darwin/release/shiftpaste target/x86_64-apple-darwin/release/shiftpaste
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

# App icon from the 1024 px master.
ICONSET="$OUT/shiftpaste.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s crates/app/assets/logo-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d crates/app/assets/logo-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/shiftpaste.icns"

# Ad-hoc signature (required on Apple Silicon; not notarized).
codesign --force --deep --sign - "$APP"

STAGE="$OUT/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="$OUT/ShiftPaste-$VERSION-macOS-universal.dmg"
# hdiutil sometimes fails with "Resource busy" on CI machines: retry.
for attempt in 1 2 3 4; do
  hdiutil create -volname "Shift Paste" -srcfolder "$STAGE" -ov -format UDZO "$DMG" && break
  [ "$attempt" = 4 ] && exit 1
  sleep $((attempt * 5))
done
echo "Built $OUT/ShiftPaste-$VERSION-macOS-universal.dmg"
