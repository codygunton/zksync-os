#!/bin/bash
set -e

# Benchmark RISC-V cycles for Zisk (64-bit) execution
#
# This script builds the Zisk binary and runs eth_runner with the Zisk
# emulator to measure cycle counts. It uses completely separate build
# artifacts from Airbender.
#
# Usage: ./bench-zisk-cycles.sh [block_dir]
#   block_dir: Path to block data (default: tests/instances/eth_runner/blocks/22244135)
#
# Requirements:
#   - Zisk emulator must be available at ../../../zisk/emulator (relative to tests/rig)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BLOCK_DIR="${1:-tests/instances/eth_runner/blocks/22244135}"

echo "=============================================="
echo "  Zisk (RV64IM) Cycle Benchmark"
echo "=============================================="
echo ""
echo "Block: $BLOCK_DIR"
echo ""

# Check if zisk emulator exists
ZISK_PATH="$(dirname "$SCRIPT_DIR")/../zisk/emulator"
if [[ ! -d "$ZISK_PATH" ]]; then
    echo "WARNING: Zisk emulator not found at expected path: $ZISK_PATH"
    echo "The zisk-witness feature requires the Zisk emulator."
    echo "Please ensure it's available or update tests/rig/Cargo.toml"
    echo ""
fi

# Step 1: Build the Zisk binary
echo "=== Step 1: Building Zisk binary ==="
cd zksync_os
./build.sh --machine zisk
cd ..

# Verify binary exists
if [[ ! -f "zksync_os/zksync_os_zisk.bin" ]]; then
    echo "ERROR: Zisk binary not found at zksync_os/zksync_os_zisk.bin"
    exit 1
fi
echo "Built: zksync_os/zksync_os_zisk.bin"
echo ""

# Step 2: Run eth_runner with Zisk emulator
echo "=== Step 2: Running eth_runner with Zisk emulator ==="
echo ""

# Create symlinks for the runner to find (it looks for for_tests.{bin,elf})
ln -sf zksync_os_zisk.bin zksync_os/for_tests.bin
ln -sf zksync_os_zisk.elf zksync_os/for_tests.elf

# Run the benchmark from eth_runner directory (it has its own workspace)
# Set OVERRIDE_ZKSYNC_OS_PATH to point to our zksync_os directory
# Set LIBRARY_PATH for Intel OneAPI (required by Zisk's iomp5 dependency)
# Features:
#   - rig/zisk-witness: Use Zisk emulator for execution
#   - rig/no_print: Disable verbose printing
#   - rig/unlimited_native: Remove native gas limits
# Set VERBOSE_ORACLE=1 for detailed query logging
# Set ZISK_QUIET=1 to suppress per-step emulator logging (12M+ lines)
export OVERRIDE_ZKSYNC_OS_PATH="$SCRIPT_DIR/zksync_os"
export LIBRARY_PATH="/opt/intel/oneapi/compiler/2025.0/lib:$LIBRARY_PATH"
export VERBOSE_ORACLE=1
export ZISK_QUIET=1
cd tests/instances/eth_runner
RUSTFLAGS="-Awarnings" RUST_LOG=eth_runner=info,rig=info cargo run \
    --features "rig/zisk-witness,rig/no_print,rig/unlimited_native" \
    -- single-run --block-dir "$SCRIPT_DIR/$BLOCK_DIR" --randomized 2>&1 | tee /tmp/zisk_bench.log
cd ../../..

echo ""
echo "=== Zisk Benchmark Complete ==="
echo ""
echo "Results saved to: /tmp/zisk_bench.log"
echo ""

# Extract key metrics from log
echo "=== Key Metrics ==="
grep -E "(cycles|Cycles|ORACLE|steps|Steps|Native used|effective)" /tmp/zisk_bench.log || true
