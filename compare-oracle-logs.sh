#!/bin/bash
# Compare oracle query logs between Airbender and Zisk runs
#
# Usage: ./compare-oracle-logs.sh [airbender_log] [zisk_log]
#
# Default logs are /tmp/airbender_bench.log and /tmp/zisk_bench.log

AIR_LOG="${1:-/tmp/airbender_bench.log}"
ZISK_LOG="${2:-/tmp/zisk_bench.log}"

echo "=============================================="
echo "  Oracle Query Log Comparison"
echo "=============================================="
echo ""
echo "Airbender log: $AIR_LOG"
echo "Zisk log: $ZISK_LOG"
echo ""

# Extract query summaries
echo "=== Query Count Summary ==="
echo ""
echo "AIRBENDER:"
grep -E "^\[ORACLE\]|^  [A-Z]" "$AIR_LOG" | head -30
echo ""
echo "ZISK:"
grep -E "^\[ORACLE\]|^  [A-Z]" "$ZISK_LOG" | head -30
echo ""

# Extract per-transaction summaries
echo "=== Per-Transaction Queries ==="
echo ""
echo "--- AIRBENDER ---"
grep "^  TX " "$AIR_LOG" | head -20
echo ""
echo "--- ZISK ---"
grep "^  TX " "$ZISK_LOG" | head -20
echo ""

# Side-by-side diff of first divergence
echo "=== First Divergence (side-by-side diff) ==="
echo ""

# Extract just query lines and diff
grep "^\[oracle\] query" "$AIR_LOG" > /tmp/air_queries.txt
grep "^\[oracle\] query" "$ZISK_LOG" > /tmp/zisk_queries.txt

# Show diff
diff --side-by-side --width=160 /tmp/air_queries.txt /tmp/zisk_queries.txt | head -60

echo ""
echo "=== Query Count Difference ==="
echo "Airbender queries: $(wc -l < /tmp/air_queries.txt)"
echo "Zisk queries: $(wc -l < /tmp/zisk_queries.txt)"
