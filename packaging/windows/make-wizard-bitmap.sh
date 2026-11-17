#!/usr/bin/env bash
# Regenerates wizard.bmp, the picture on the installer's and uninstaller's
# welcome/finish pages. Drawn at 2x (328x628) so it stays sharp on high-DPI
# screens. Needs rsvg-convert, ImageMagick and the DejaVu fonts.
set -euo pipefail
cd "$(dirname "$0")"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
rsvg-convert -w 176 -h 176 ../../crates/app/assets/logo.svg -o "$tmp/logo.png"
convert -size 328x628 gradient:'#EEF2FF'-'#C7D2FE' \
  "$tmp/logo.png" -gravity north -geometry +0+168 -composite \
  -font DejaVu-Sans-Bold -pointsize 34 -fill '#312E81' -gravity north -annotate +0+372 'Shift Paste' \
  -font DejaVu-Sans -pointsize 19 -fill '#4F46E5' -gravity north -annotate +0+420 'Clipboard history & snippets' \
  -type TrueColor BMP3:wizard.bmp
echo "wrote $(pwd)/wizard.bmp"
