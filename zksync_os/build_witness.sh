#!/bin/bash
set -e

# Build evm_replay and generate witness
#
# Usage: ./build_witness.sh <block_dir> [witness_output_dir] [--zisk]
# Example: ./build_witness.sh blocks/22244135 /tmp/inputs
# Example: ./build_witness.sh blocks/22244135 /tmp/inputs --zisk
#
# Options:
#   --zisk    Use Zisk (64-bit) emulator for witness generation instead of airbender (32-bit)
#             This generates witness compatible with 64-bit Zisk execution.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse arguments
USE_ZISK=false
BLOCK_DIR=""
WITNESS_DIR="/tmp/inputs"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --zisk)
            USE_ZISK=true
            shift
            ;;
        *)
            if [[ -z "$BLOCK_DIR" ]]; then
                BLOCK_DIR="$1"
            else
                WITNESS_DIR="$1"
            fi
            shift
            ;;
    esac
done

if [[ -z "$BLOCK_DIR" ]]; then
    echo "Usage: $0 <block_dir> [witness_output_dir] [--zisk]"
    echo ""
    echo "Options:"
    echo "  --zisk    Use Zisk (64-bit) for witness generation"
    exit 1
fi

if $USE_ZISK; then
    echo "=== Building evm_replay for Zisk (64-bit) ==="
    cd "$SCRIPT_DIR"
    ./build.sh --machine zisk
    cp zksync_os_zisk.bin evm_replay.bin
    cp zksync_os_zisk.elf evm_replay.elf
    echo "Created evm_replay.bin and evm_replay.elf (64-bit Zisk)"

    echo ""
    echo "=== Generating witness using Zisk emulator ==="
    cd "$REPO_ROOT/tests/instances/eth_runner"
    # Intel oneAPI library path needed for liomp5 linker dependency
    LIBRARY_PATH="/opt/intel/oneapi/compiler/2025.0/lib:${LIBRARY_PATH:-}" \
    RUST_LOG=eth_runner=info,rig=info cargo run --release \
        --features rig/no_print,rig/unlimited_native,rig/zisk-witness -- \
        single-run --block-dir "$BLOCK_DIR" --randomized --witness-output-dir "$WITNESS_DIR"
else
    echo "=== Building evm_replay for airbender (32-bit) ==="
    cd "$SCRIPT_DIR"
    ./build.sh --machine airbender
    cp zksync_os_airbender.bin evm_replay.bin
    cp zksync_os_airbender.elf evm_replay.elf
    echo "Created evm_replay.bin and evm_replay.elf (32-bit airbender)"

    echo ""
    echo "=== Generating witness using airbender emulator ==="
    cd "$REPO_ROOT/tests/instances/eth_runner"
    RUST_LOG=eth_runner=info cargo run --release --features rig/no_print,rig/unlimited_native -- \
        single-run --block-dir "$BLOCK_DIR" --randomized --witness-output-dir "$WITNESS_DIR"
fi

echo ""
echo "=== Done ==="
echo "Witness written to: $WITNESS_DIR"
if $USE_ZISK; then
    echo "Mode: Zisk (64-bit) - witness compatible with Zisk execution"
else
    echo "Mode: airbender (32-bit) - witness compatible with airbender execution"
fi
