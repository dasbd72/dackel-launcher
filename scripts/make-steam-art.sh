#!/bin/sh
# Draws the launcher's Steam artwork into assets/steam/ from the app icon's paw print.
# Needs rsvg-convert and ImageMagick; the PNGs are committed, so this only reruns when the
# artwork changes. Font: Noto Sans Bold.
set -eu
cd "$(dirname "$0")/.."
out=assets/steam
font=${STEAM_ART_FONT:-/usr/share/fonts/noto/NotoSans-Bold.ttf}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# The paw print alone, white on transparent, from the icon's path.
paw() { # size
  rsvg-convert -w "$1" -h "$1" - <<SVG
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><g fill="#fff"><path d="M240,108a28,28,0,1,1-28-28A28,28,0,0,1,240,108ZM72,108a28,28,0,1,0-28,28A28,28,0,0,0,72,108ZM92,88A28,28,0,1,0,64,60,28,28,0,0,0,92,88Zm72,0a28,28,0,1,0-28-28A28,28,0,0,0,164,88Zm23.12,60.86a35.3,35.3,0,0,1-16.87-21.14,44,44,0,0,0-84.5,0A35.25,35.25,0,0,1,69,148.82,40,40,0,0,0,88,224a39.48,39.48,0,0,0,15.52-3.13,64.09,64.09,0,0,1,48.87,0,40,40,0,0,0,34.73-72Z"/></g></svg>
SVG
}
background() { # size
  magick -size "$1" gradient:'#5bbcff-#1d5d96' "$tmp/bg.png"
}
label() { # pointsize file
  magick -background none -fill white -font "$font" -pointsize "$1" label:'Dackel Launcher' "$2"
}

# Icon: the app icon itself.
rsvg-convert -w 256 -h 256 packaging/io.github.dasbd72.DackelLauncher.svg -o "$out/icon.png"

# Portrait capsule, 600x900.
background 600x900; cp "$tmp/bg.png" "$tmp/p.png"
paw 380 >"$tmp/paw.png"; label 64 "$tmp/text.png"
magick "$tmp/p.png" "$tmp/paw.png" -gravity center -geometry +0-70 -composite \
  "$tmp/text.png" -gravity south -geometry +0+110 -composite -depth 8 "$out/portrait.png"

# Wide capsule, 920x430.
background 920x430
paw 230 >"$tmp/paw.png"; label 54 "$tmp/text.png"
magick "$tmp/bg.png" "$tmp/paw.png" -gravity west -geometry +70+0 -composite \
  "$tmp/text.png" -gravity west -geometry +320+0 -composite -depth 8 "$out/wide.png"

# Hero, 3840x1240: a soft field of paw prints, no text (Steam draws the logo over it).
background 3840x1240
paw 700 >"$tmp/paw.png"
magick "$tmp/paw.png" -channel A -evaluate multiply 0.12 +channel "$tmp/faint.png"
magick "$tmp/bg.png" \
  "$tmp/faint.png" -gravity center -geometry +900-40 -composite \
  "$tmp/faint.png" -gravity center -geometry -900+60 -composite \
  "$tmp/faint.png" -gravity center -geometry +0+0 -composite -depth 8 "$out/hero.png"

# Logo, 1280x360 on transparent: the paw and the name.
paw 220 >"$tmp/paw.png"; label 112 "$tmp/text.png"
magick -size 1280x360 xc:none "$tmp/paw.png" -gravity west -geometry +60+0 -composite \
  "$tmp/text.png" -gravity west -geometry +320+0 -composite "$out/logo.png"
