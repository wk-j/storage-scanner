#!/bin/sh
# Headless demo frames. Lavapipe is fine: point Vulkan at it when there is no GPU.
set -eu
cd "$(dirname "$0")/.."
mkdir -p out
if [ -z "${VK_ICD_FILENAMES:-}" ] && [ -f /usr/share/vulkan/icd.d/lvp_icd.json ]; then
  export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
fi

run() {
  cargo run --release --quiet -- --demo --fps 30 --snap "$1" --scene "$2" --at "$3" $4
}

run out/scanning.png scanning 0.08 ""
run out/idle-apps.png idle 0.7 "--script scripts/hover-apps.json"
run out/clearing-caches.png clearing 0.8 ""
run out/after-trash.png emptied 0.08 ""
