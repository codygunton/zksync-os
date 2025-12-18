#!/bin/bash
# Compare TX_SIZE responses between Airbender and Zisk runs
#
# This script extracts the NEXT_TX_SIZE query responses from both log files
# and shows them side-by-side for comparison.
#
# Usage: ./compare-tx-sizes.sh [--run-fresh]
#   --run-fresh: Run both benchmarks before comparing (default: use existing logs)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

AIR_LOG="/tmp/airbender_bench.log"
ZISK_LOG="/tmp/zisk_bench.log"

# Parse arguments
RUN_FRESH=false
if [[ "$1" == "--run-fresh" ]]; then
    RUN_FRESH=true
fi

echo "=============================================="
echo "  TX_SIZE Response Comparison"
echo "=============================================="
echo ""

# Run benchmarks if requested
if [[ "$RUN_FRESH" == "true" ]]; then
    echo "=== Running fresh benchmarks ==="
    echo ""
    echo "Running Airbender benchmark..."
    ./bench-airbender-cycles.sh > /dev/null 2>&1
    echo "Running Zisk benchmark..."
    ./bench-zisk-cycles.sh > /dev/null 2>&1
    echo ""
fi

# Check if logs exist
if [[ ! -f "$AIR_LOG" ]]; then
    echo "ERROR: Airbender log not found at $AIR_LOG"
    echo "Run ./bench-airbender-cycles.sh first"
    exit 1
fi
if [[ ! -f "$ZISK_LOG" ]]; then
    echo "ERROR: Zisk log not found at $ZISK_LOG"
    echo "Run ./bench-zisk-cycles.sh first"
    exit 1
fi

# Extract NEXT_TX_SIZE responses
echo "=== Airbender TX_SIZE responses ==="
grep -E "^\[oracle\] NEXT_TX_SIZE" "$AIR_LOG" | head -20
AIR_COUNT=$(grep -c "^\[oracle\] NEXT_TX_SIZE" "$AIR_LOG" 2>/dev/null || echo "0")
echo ""
echo "Total NEXT_TX_SIZE queries: $AIR_COUNT"
echo ""

echo "=== Zisk TX_SIZE responses ==="
grep -E "^\[oracle\] NEXT_TX_SIZE" "$ZISK_LOG" | head -20
ZISK_COUNT=$(grep -c "^\[oracle\] NEXT_TX_SIZE" "$ZISK_LOG" 2>/dev/null || echo "0")
echo ""
echo "Total NEXT_TX_SIZE queries: $ZISK_COUNT"
echo ""

# Side-by-side comparison
echo "=== Side-by-Side Comparison ==="
echo ""
paste <(grep -E "^\[oracle\] NEXT_TX_SIZE" "$AIR_LOG") \
      <(grep -E "^\[oracle\] NEXT_TX_SIZE" "$ZISK_LOG") 2>/dev/null | \
    awk -F'\t' '{
        if ($1 == $2) {
            printf "MATCH: %s\n", $1
        } else {
            printf "DIFF:\n  AIR:  %s\n  ZISK: %s\n", $1, $2
        }
    }' | head -40

echo ""
echo "=== Summary ==="
if [[ "$AIR_COUNT" -eq "$ZISK_COUNT" ]]; then
    echo "Both have $AIR_COUNT NEXT_TX_SIZE queries"
else
    echo "MISMATCH: Airbender has $AIR_COUNT, Zisk has $ZISK_COUNT"
fi
