#!/usr/bin/env bash
# Build ShiftPaste-<version>-x86_64.AppImage. Usage: packaging/linux/build-appimage.sh <version>
# Needs appimagetool on PATH (or set APPIMAGETOOL). Run from the repo root.
set -euo pipefail
VERSION="${1:?version}"
TOOL="${APPIMAGETOOL:-appimagetool}"
cargo build --release -p shiftpaste
APPDIR=target/appimage/ShiftPaste.AppDir
rm -rf target/appimage && mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp target/release/shiftpaste "$APPDIR/usr/bin/"
cp packaging/linux/shiftpaste.desktop "$APPDIR/" && cp packaging/linux/shiftpaste.desktop "$APPDIR/usr/share/applications/"
cp crates/app/assets/logo-256.png "$APPDIR/shiftpaste.png"
cp crates/app/assets/logo-256.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/shiftpaste.png"
ln -sf usr/bin/shiftpaste "$APPDIR/AppRun"
ARCH=x86_64 "$TOOL" --no-appstream "$APPDIR" "target/appimage/ShiftPaste-$VERSION-x86_64.AppImage"
echo "Built target/appimage/ShiftPaste-$VERSION-x86_64.AppImage"
