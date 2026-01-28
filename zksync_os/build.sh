#!/bin/bash
set -e

# Build zksync-os for different machines
#
# Usage: ./build.sh --machine {airbender|zisk} [--clean] [--debug]
#
# Machines:
#   airbender - RV32IM for airbender zkVM (entry point 0x01000000)
#   zisk      - RV64IM for Zisk zkVM (RAM at 0xa0000000)
#
# Options:
#   --debug   Build with debug info at low addresses (for addr2line)
#             To convert addresses: debug_addr = prod_addr - 0x80000000 + 0x00100000
#
# Output files are named: zksync_os_{machine}.{bin,elf,text}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

MACHINE=""
CLEAN=false
DEBUG=false

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
        --debug)
            DEBUG=true
            shift
            ;;
        # Backwards compatibility: positional arg for machine
        airbender | zisk)
            MACHINE="$1"
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 --machine {airbender|zisk} [--clean] [--debug]"
            exit 2
            ;;
    esac
done

# Default machine
[ -n "$MACHINE" ] || {
    echo "Missing --machine argument"
    echo "Usage: $0 --machine {airbender|zisk} [--clean]"
    exit 2
}

# Configure machine-specific settings
case "$MACHINE" in
    airbender)
        # RV32IM for airbender zkVM (1020M RAM available)
        # Uses linker scripts from riscv_common submodule
        TARGET="riscv32i-unknown-none-elf"
        LDS_DIR="zksync-airbender/riscv_common/src/lds"
        MEMORY_LAYOUT="memory.x"
        LINK_SCRIPT="link.x"
        TARGET_FEATURES="+m,-unaligned-scalar-mem,+relax"
        BUILD_STD_FLAGS=""
        echo "=== Building for AIRBENDER (RV32IM, entry 0x01000000) ==="
        ;;
    zisk)
        # RV64IMAC for Zisk zkVM (512M RAM available)
        TARGET="riscv64imac-unknown-none-elf"
        LDS_DIR="src/lds"
        if $DEBUG; then
            MEMORY_LAYOUT="memory-zisk-debug.x"
            echo "=== Building for ZISK DEBUG (RV64IMAC, low addresses for debug info) ==="
        else
            MEMORY_LAYOUT="memory-zisk.x"
            echo "=== Building for ZISK (RV64IMAC, RAM at 0xa0000000) ==="
        fi
        LINK_SCRIPT="link-512m.x"
        TARGET_FEATURES="+m,+a,+c,-unaligned-scalar-mem,+relax"
        BUILD_STD_FLAGS=""
        ;;
    *)
        echo "Invalid --machine: $MACHINE"
        echo "Valid options: airbender, zisk"
        exit 1
        ;;
esac

# Features for the build (can be overridden via environment variable)
# Default features for EVM replay: proving + unlimited_native + disable_system_contracts + prevrandao + evm_refunds
FEATURES="${FEATURES:-proving,unlimited_native,disable_system_contracts,prevrandao,evm_refunds}"
echo "Building with FEATURES=$FEATURES"
# Build output file names
if $DEBUG; then
    SUFFIX="${MACHINE}_debug"
else
    SUFFIX="${MACHINE}"
fi
BIN_NAME="zksync_os_${SUFFIX}.bin"
ELF_NAME="zksync_os_${SUFFIX}.elf"
TEXT_NAME="zksync_os_${SUFFIX}.text"

echo "Memory layout: $LDS_DIR/$MEMORY_LAYOUT"
echo "Features: $FEATURES"
echo "Target: $TARGET"
if $DEBUG; then
    echo "Debug info: enabled (debuginfo=2)"
fi
echo ""

# Verify memory script exists
if [[ ! -f "$LDS_DIR/$MEMORY_LAYOUT" ]]; then
    echo "Error: Memory script not found: $LDS_DIR/$MEMORY_LAYOUT"
    exit 1
fi

# Set debuginfo level
if $DEBUG; then
    DEBUGINFO=2
else
    DEBUGINFO=0
fi

# Construct RUSTFLAGS (overrides config.toml)
export RUSTFLAGS="-Awarnings \
  -C target-feature=$TARGET_FEATURES \
  -C link-arg=-L$LDS_DIR \
  -C link-arg=-T$MEMORY_LAYOUT \
  -C link-arg=-T$LINK_SCRIPT \
  -C codegen-units=1 \
  -C debuginfo=$DEBUGINFO"

# Clean if requested
if $CLEAN; then
    echo "Cleaning..."
    cargo clean
fi

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

# Create demangled disassembly
DUMP_NAME="zksync_os_${SUFFIX}.dump"
echo ""
echo "Creating disassembly with demangled symbols..."
if $DEBUG; then
    riscv64-elf-objdump -d -C -S "$ELF_NAME" > "$DUMP_NAME" 2> /dev/null \
        || llvm-objdump -d --demangle -S "$ELF_NAME" > "$DUMP_NAME" 2> /dev/null \
        || echo "Warning: Could not create disassembly dump"
else
    riscv64-elf-objdump -d -C "$ELF_NAME" > "$DUMP_NAME" 2> /dev/null \
        || llvm-objdump -d --demangle "$ELF_NAME" > "$DUMP_NAME" 2> /dev/null \
        || echo "Warning: Could not create disassembly dump"
fi
echo "  $DUMP_NAME (disassembly with demangled symbols)"

# Print addr2line helper for debug builds
if $DEBUG; then
    echo ""
    echo "=== Debug Build Info ==="
    echo "To map production addresses to source lines, use the helper script:"
    echo ""
    echo "  ./addr2line.sh 0x8006e388"
    echo ""
    echo "This finds the function by symbol name (not linear offset) to handle"
    echo "layout differences between production and debug builds."
fi
