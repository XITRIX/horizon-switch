#!/usr/bin/env bash
# Build a kaiHEN NRO using devkitA64, libounce and the OunceVK portlibs.
set -euo pipefail
export SWITCH2=1
. "$(dirname "$0")/env.sh"
export SWITCH_BUILD=${SWITCH2_BUILD:-$SWITCH_DIR/build/switch2}
export CARGO_TARGET_DIR=$SWITCH_BUILD/target
export RUST_TOOLCHAIN=${RUST_TOOLCHAIN:-nightly-2026-10-26}

"$SWITCH_DIR/scripts/build.sh"
nacptool --create "Horizon" "Horizon contributors" "0.1.1" "$SWITCH_BUILD/Horizon.nacp"
nro_assets=(--nacp="$SWITCH_BUILD/Horizon.nacp")
logo="$SWITCH_DIR/../.local/horizon-client/26.3/assets/minecraft/textures/gui/title/minecraft.png"
if [[ -f "$logo" ]] && command -v magick > /dev/null; then
    icon="$SWITCH_BUILD/Horizon.jpg"
    magick "$logo" -trim +repage -resize 236x236 \
        -background '#20242a' -alpha remove -alpha off -gravity center -extent 256x256 \
        -colorspace sRGB -strip -interlace none -quality 95 "$icon"
    nro_assets+=(--icon="$icon")
fi
elf2nro "$SWITCH_BUILD/horizon.elf" "$SWITCH_BUILD/Horizon.nro" "${nro_assets[@]}"
echo "built $SWITCH_BUILD/Horizon.nro"
