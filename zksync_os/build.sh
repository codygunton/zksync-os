#!/bin/bash
set -e

# Build zksync-os for different machines
#
# Usage: ./build.sh --machine {airbender|qemu|zisk} [--clean]
#
# Machines:
#   airbender - RV32IM for airbender zkVM (entry point 0x01000000)
#   qemu      - RV64IM for QEMU debugging (RAM at 0x88000000)
#   zisk      - RV64IM for Zisk zkVM (RAM at 0xa0000000)
#
# Output files are named: zksync_os_{machine}.{bin,elf,text}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

MACHINE=""
CLEAN=false

# Parse arguments
while [ "$#" -gt 0 ]; do
    case "$1" in
        --machine)
            [ "$#" -ge 2 ] || {
                echo "Missing value for --machine"
                exit 2
            }
            MACHINE="$2"
            shift 2
            ;;
        --clean)
            CLEAN=true
            shift
            ;;
        # Backwards compatibility: positional arg for machine
        airbender | qemu | zisk)
            MACHINE="$1"
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 --machine {airbender|qemu|zisk} [--clean]"
            exit 2
            ;;
    esac
done

# Default machine
[ -n "$MACHINE" ] || {
    echo "Missing --machine argument"
    echo "Usage: $0 --machine {airbender|qemu|zisk} [--clean]"
    exit 2
}

# Configure machine-specific settings
case "$MACHINE" in
    airbender)
        # RV32IM for airbender zkVM (1022M RAM available)
        TARGET="riscv32i-unknown-none-elf"
        MEMORY_LAYOUT="memory-airbender.x"
        LINK_SCRIPT="link.x"
        TARGET_FEATURES="+m,-unaligned-scalar-mem,+relax"
        BUILD_STD_FLAGS=""
        echo "=== Building for AIRBENDER (RV32IM, entry 0x01000000) ==="
        ;;
    qemu)
        # RV64IM for QEMU debugging (512M RAM available)
        TARGET="riscv64im-unknown-none-elf.json"
        MEMORY_LAYOUT="memory-qemu.x"
        LINK_SCRIPT="link-512m.x"
        TARGET_FEATURES="+m,-unaligned-scalar-mem,+relax"
        BUILD_STD_FLAGS="-Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem"
        echo "=== Building for QEMU (RV64IM, RAM at 0x88000000) ==="
        ;;
    zisk)
        # RV64IM for Zisk zkVM (512M RAM available)
        TARGET="riscv64im-unknown-none-elf.json"
        MEMORY_LAYOUT="memory-zisk.x"
        LINK_SCRIPT="link-512m.x"
        TARGET_FEATURES="+m,-unaligned-scalar-mem,+relax"
        BUILD_STD_FLAGS="-Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem"
        echo "=== Building for ZISK (RV64IM, RAM at 0xa0000000) ==="
        ;;
    *)
        echo "Invalid --machine: $MACHINE"
        echo "Valid options: airbender, qemu, zisk"
        exit 1
        ;;
esac

# Features for the build
FEATURES="proving,eth_runner"

# Build output file names
BIN_NAME="zksync_os_${MACHINE}.bin"
ELF_NAME="zksync_os_${MACHINE}.elf"
TEXT_NAME="zksync_os_${MACHINE}.text"

echo "Memory layout: src/lds/$MEMORY_LAYOUT"
echo "Features: $FEATURES"
echo "Target: $TARGET"
echo ""

# Verify memory script exists
if [[ ! -f "src/lds/$MEMORY_LAYOUT" ]]; then
    echo "Error: Memory script not found: src/lds/$MEMORY_LAYOUT"
    exit 1
fi

# Construct RUSTFLAGS (overrides config.toml)
export RUSTFLAGS="-Awarnings \
  -C target-feature=$TARGET_FEATURES \
  -C link-arg=-Lsrc/lds \
  -C link-arg=-T$MEMORY_LAYOUT \
  -C link-arg=-T$LINK_SCRIPT \
  -C link-arg=--save-temps \
  -C force-frame-pointers \
  --remap-path-prefix=/=/src \
  -C link-arg=--build-id=sha1 \
  -C codegen-units=1 \
  -C debuginfo=0"

# Clean if requested
if $CLEAN; then
    echo "Cleaning..."
    cargo clean
fi

# Clean up only the artifacts for this build
rm -f "$BIN_NAME" "$ELF_NAME" "$TEXT_NAME"

echo "Building..."
cargo build --features "$FEATURES" --release --target "$TARGET" $BUILD_STD_FLAGS

echo "Creating output files..."
# Produce outputs
cargo objcopy --features "$FEATURES" --release --target "$TARGET" $BUILD_STD_FLAGS -- -O binary "$BIN_NAME"
# Strip .heap, .stack from ELF (Zisk tries to initialize large NOBITS sections which explodes instruction count)
cargo objcopy --features "$FEATURES" --release --target "$TARGET" $BUILD_STD_FLAGS -- -R .heap -R .stack "$ELF_NAME"
cargo objcopy --features "$FEATURES" --release --target "$TARGET" $BUILD_STD_FLAGS -- -O binary --only-section=.text "$TEXT_NAME"

echo ""
echo "=== Build complete ==="
echo "  $BIN_NAME  (full binary for execution)"
echo "  $ELF_NAME  (ELF without .text for profiling/symbols)"
echo "  $TEXT_NAME (code section only)"

# Show memory layout from the ELF
echo ""
echo "Memory sections:"
riscv64-elf-objdump -h "$ELF_NAME" 2> /dev/null | grep -E "^\s+[0-9]+" | head -10 || true
