#!/bin/sh
# Headless demo reel: scan, hover crossfades, confirm both clears.
# Rune writes H.264 yuv420p; ffmpeg turns that into a README-sized GIF.
set -eu
cd "$(dirname "$0")/.."
mkdir -p docs
if [ -z "${VK_ICD_FILENAMES:-}" ] && [ -f /usr/share/vulkan/icd.d/lvp_icd.json ]; then
  export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
fi

# Scale 1 is the card's point size. 60 fps keeps the hover fades smooth.
cargo run --release --quiet -- --demo --scale 1 --fps 60 \
  --script scripts/demo.json --video docs/demo.mp4

# Palette GIF, small enough for GitHub to render inline. The mp4 stays the sharper copy.
PALETTE="$(mktemp --suffix=.png)"
ffmpeg -y -i docs/demo.mp4 -vf "fps=12,scale=640:-1:flags=lanczos,palettegen=stats_mode=diff:max_colors=128" "$PALETTE"
ffmpeg -y -i docs/demo.mp4 -i "$PALETTE" -lavfi "fps=12,scale=640:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3" -loop 0 docs/demo.gif
rm -f "$PALETTE"

echo "mp4 $(du -h docs/demo.mp4 | awk '{print $1}')  gif $(du -h docs/demo.gif | awk '{print $1}')"
