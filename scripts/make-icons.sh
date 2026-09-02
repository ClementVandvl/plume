#!/usr/bin/env bash
# Rebuilds every application icon from `src-tauri/icons/icon-source.png`.
#
# macOS is the odd one out. Its dock draws icons at their full canvas size, so
# an artwork that fills 1024×1024 comes out visibly larger than every neighbour.
# Apple's grid puts the body at 824×824 inside a 1024×1024 canvas — measured on
# stock apps, the opaque area runs 858–880 px wide — and the remaining margin is
# transparent. Windows and Linux want the opposite: edge to edge.
#
# So the source stays full-bleed, every platform is generated from it, and the
# macOS `.icns` is then rebuilt on its own from a padded copy.
#
# Needs: ImageMagick (`magick`), macOS `sips` and `iconutil`, and `npx`.
set -euo pipefail

cd "$(dirname "$0")/.."
icons="src-tauri/icons"
source_png="$icons/icon-source.png"

[ -f "$source_png" ] || { echo "missing $source_png" >&2; exit 1; }

echo "→ every platform, edge to edge"
npx tauri icon "$source_png" >/dev/null

# Plume is a desktop application. `tauri icon` writes phone icon sets whatever
# the project targets, and they were removed once already.
rm -rf "$icons/android" "$icons/ios"

if [ "$(uname)" != "Darwin" ]; then
  echo "→ not macOS: leaving icon.icns as tauri generated it"
  exit 0
fi

echo "→ macOS: same artwork at 824 px inside a 1024 px canvas"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

magick "$source_png" -resize 824x824 \
  -background none -gravity center -extent 1024x1024 \
  "$work/padded.png"

set="$work/plume.iconset"
mkdir -p "$set"
for size in 16 32 128 256 512; do
  sips -z $size $size "$work/padded.png" --out "$set/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) "$work/padded.png" \
    --out "$set/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil -c icns "$set" -o "$icons/icon.icns"

opaque=$(magick "$work/padded.png" -alpha extract -threshold 1% -format "%@" info:)
echo "✓ icon.icns rebuilt — opaque area $opaque of 1024x1024"
