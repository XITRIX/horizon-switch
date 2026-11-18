# Environment for the Switch build. Source it from the repository root: `. switch/scripts/env.sh`.
# DEVKITPRO defaults to /opt/devkitpro (devkitA64, libnx, switch-mesa, switch-tools).
export DEVKITPRO=${DEVKITPRO:-/opt/devkitpro}
SWITCH_DIR=$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)
export SWITCH_DIR
export SWITCH_BUILD=${SWITCH_BUILD:-$SWITCH_DIR/build}
export PATH=$DEVKITPRO/devkitA64/bin:$DEVKITPRO/tools/bin:$PATH
# build-std builds this patched copy of the toolchain's std (scripts/prepare-std.sh).
export __CARGO_TESTS_ONLY_SRC_ROOT=$SWITCH_BUILD/rust-src/library
# C code in crates (cc): devkitA64 with the target SDK's headers.
export CC_aarch64_nintendo_switch_newlib=$DEVKITPRO/devkitA64/bin/aarch64-none-elf-gcc
export AR_aarch64_nintendo_switch_newlib=$DEVKITPRO/devkitA64/bin/aarch64-none-elf-ar
SWITCH_SDK=$DEVKITPRO/libnx
SWITCH_RUST_CPU=cortex-a57
SWITCH_ARCH=-march=armv8-a+crc+crypto
SWITCH_GRAPHICS="\"-C\", \"link-arg=-L$SWITCH_BUILD/nvk/lib\", \"-C\", \"link-arg=-lvulkan_bh\", \"-C\", \"link-arg=-lEGL\", \"-C\", \"link-arg=-lglapi\", \"-C\", \"link-arg=-ldrm_nouveau\","
SWITCH_CPP="-D__SWITCH__"
if [ "${SWITCH2:-0}" = 1 ]; then
    SWITCH_SDK=$DEVKITPRO/libounce
    SWITCH_RUST_CPU=cortex-a78c
    SWITCH_ARCH=$(sed -n 's/^SWITCH2_CPU[[:blank:]]*:=[[:blank:]]*//p' "$SWITCH_SDK/switch2_cpu.mk")
    SWITCH_CPP="$SWITCH_CPP -D__SWITCH2__ -I$DEVKITPRO/portlibs/switch2/include"
    # OunceVK's EGL archive also exports the native Vulkan entry points and VI surfaces.
    SWITCH_GRAPHICS="\"-C\", \"link-arg=-L$DEVKITPRO/portlibs/switch2/lib\", \"-C\", \"link-arg=-lEGL\","
fi
export SWITCH_CFLAGS="-O2 $SWITCH_ARCH -mtp=soft -fPIE $SWITCH_CPP -I$SWITCH_SDK/include -I$DEVKITPRO/portlibs/switch/include"
export CFLAGS_aarch64_nintendo_switch_newlib="$SWITCH_CFLAGS"
