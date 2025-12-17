#!/bin/bash
set -e

# Build evm_replay with proving feature and generate witness
#
# Usage: ./build_witness.sh <block_dir> [witness_output_dir]
# Example: ./build_witness.sh blocks/22244135 /tmp/inputs

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BLOCK_DIR="${1:?Usage: $0 <block_dir> [witness_output_dir]}"
WITNESS_DIR="${2:-/tmp/inputs}"

echo "=== Building evm_replay with proving feature ==="
cd "$SCRIPT_DIR"
./build.sh --machine airbender
cp zksync_os_airbender.bin evm_replay.bin
cp zksync_os_airbender.elf evm_replay.elf
echo "Created evm_replay.bin and evm_replay.elf"

echo ""
echo "=== Generating witness ==="
cd "$REPO_ROOT/tests/instances/eth_runner"
RUST_LOG=eth_runner=info cargo run --release --features rig/no_print,rig/unlimited_native -- \
    single-run --block-dir "$BLOCK_DIR" --randomized --witness-output-dir "$WITNESS_DIR"

echo ""
echo "=== Done ==="
echo "Witness written to: $WITNESS_DIR"
