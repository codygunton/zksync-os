#!/bin/bash
set -e

# Benchmark RISC-V cycles for Airbender (32-bit) execution
#
# This script builds the Airbender binary and runs eth_runner to measure
# cycle counts. It uses completely separate build artifacts from Zisk.
#
# Usage: ./bench-airbender-cycles.sh [block_dir]
#   block_dir: Path to block data (default: tests/instances/eth_runner/blocks/22244135)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BLOCK_DIR="${1:-tests/instances/eth_runner/blocks/22244135}"

echo "=============================================="
echo "  Airbender (RV32IM) Cycle Benchmark"
echo "=============================================="
echo ""
echo "Block: $BLOCK_DIR"
echo ""

# Step 1: Build the Airbender binary
echo "=== Step 1: Building Airbender binary ==="
cd zksync_os
./build.sh --machine airbender
cd ..

# Verify binary exists
if [[ ! -f "zksync_os/zksync_os_airbender.bin" ]]; then
    echo "ERROR: Airbender binary not found at zksync_os/zksync_os_airbender.bin"
    exit 1
fi
echo "Built: zksync_os/zksync_os_airbender.bin"
echo ""

# Step 2: Run eth_runner with Airbender
echo "=== Step 2: Running eth_runner with Airbender simulator ==="
echo ""

# Create symlinks for the runner to find (it looks for for_tests.{bin,elf})
ln -sf zksync_os_airbender.bin zksync_os/for_tests.bin
ln -sf zksync_os_airbender.elf zksync_os/for_tests.elf

# Run the benchmark from eth_runner directory (it has its own workspace)
# Set OVERRIDE_ZKSYNC_OS_PATH to point to our zksync_os directory
# Features:
#   - rig/no_print: Disable verbose printing
#   - rig/unlimited_native: Remove native gas limits
# NO zisk-witness feature - we want to use the airbender simulator
# Set VERBOSE_ORACLE=1 for detailed query logging
export OVERRIDE_ZKSYNC_OS_PATH="$SCRIPT_DIR/zksync_os"
export VERBOSE_ORACLE=1
cd tests/instances/eth_runner
RUST_LOG=eth_runner=info cargo run --release \
    --features "rig/no_print,rig/unlimited_native" \
    -- single-run --block-dir "$SCRIPT_DIR/$BLOCK_DIR" 2>&1 | tee /tmp/airbender_bench.log
cd ../../..

echo ""
echo "=== Airbender Benchmark Complete ==="
echo ""
echo "Results saved to: /tmp/airbender_bench.log"
echo ""

# Extract key metrics from log
echo "=== Key Metrics ==="
grep -E "(cycles|Cycles|ORACLE|Native used|effective)" /tmp/airbender_bench.log || true
