#!/bin/bash
# Analyze oracle query divergence between Airbender and Zisk runs
#
# This script:
# 1. Runs both benchmark scripts (or uses existing logs)
# 2. Compares query logs to find the first divergence
# 3. Uses addr2line to resolve PC addresses to function names
#
# Usage: ./analyze-oracle-divergence.sh [--run-fresh]
#   --run-fresh: Run both benchmarks before comparing (default: use existing logs)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

AIR_LOG="/tmp/airbender_bench.log"
ZISK_LOG="/tmp/zisk_bench.log"
AIR_ELF="zksync_os/zksync_os_airbender.elf"
ZISK_ELF="zksync_os/zksync_os_zisk.elf"

# Parse arguments
RUN_FRESH=false
if [[ "$1" == "--run-fresh" ]]; then
    RUN_FRESH=true
fi

echo "=============================================="
echo "  Oracle Query Divergence Analyzer"
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

# Extract query lines with PC information
echo "=== Extracting query data ==="
grep -E "^\[oracle\] query .* complete" "$AIR_LOG" | \
    sed 's/\[oracle\] query //' | \
    awk '{print NR, $0}' > /tmp/air_queries_full.txt

grep -E "^\[oracle\] query .* complete" "$ZISK_LOG" | \
    sed 's/\[oracle\] query //' | \
    awk '{print NR, $0}' > /tmp/zisk_queries_full.txt

AIR_COUNT=$(wc -l < /tmp/air_queries_full.txt)
ZISK_COUNT=$(wc -l < /tmp/zisk_queries_full.txt)

echo "Airbender queries: $AIR_COUNT"
echo "Zisk queries: $ZISK_COUNT"
echo ""

# Find first divergence
echo "=== Finding first divergence ==="

# Compare query types (ignoring PC for comparison)
grep -E "^\[oracle\] query .* complete" "$AIR_LOG" | \
    sed 's/\[oracle\] query //' | \
    sed 's/pc=0x[0-9a-f]* //' > /tmp/air_queries_no_pc.txt

grep -E "^\[oracle\] query .* complete" "$ZISK_LOG" | \
    sed 's/\[oracle\] query //' | \
    sed 's/pc=0x[0-9a-f]* //' > /tmp/zisk_queries_no_pc.txt

# Find line number of first difference
DIFF_LINE=$(diff -n /tmp/air_queries_no_pc.txt /tmp/zisk_queries_no_pc.txt 2>/dev/null | head -1 | grep -oP '^\d+' || echo "")

if [[ -z "$DIFF_LINE" ]]; then
    # Check if one has more queries than the other
    if [[ "$AIR_COUNT" -ne "$ZISK_COUNT" ]]; then
        MIN_COUNT=$((AIR_COUNT < ZISK_COUNT ? AIR_COUNT : ZISK_COUNT))
        DIFF_LINE=$((MIN_COUNT + 1))
        echo "Queries match up to line $MIN_COUNT, but counts differ"
        echo ""
    else
        echo "No divergence found! All $AIR_COUNT queries match."
        exit 0
    fi
fi

echo "First divergence at query #$DIFF_LINE"
echo ""

# Show context around divergence
echo "=== Context around divergence ==="
echo ""
echo "--- Airbender (queries $((DIFF_LINE-2)) to $((DIFF_LINE+2))) ---"
sed -n "$((DIFF_LINE > 2 ? DIFF_LINE-2 : 1)),$((DIFF_LINE+2))p" /tmp/air_queries_full.txt
echo ""
echo "--- Zisk (queries $((DIFF_LINE-2)) to $((DIFF_LINE+2))) ---"
sed -n "$((DIFF_LINE > 2 ? DIFF_LINE-2 : 1)),$((DIFF_LINE+2))p" /tmp/zisk_queries_full.txt
echo ""

# Extract PC from divergent query
AIR_PC=$(sed -n "${DIFF_LINE}p" /tmp/air_queries_full.txt | grep -oP 'pc=0x[0-9a-f]+' | sed 's/pc=//')

if [[ -n "$AIR_PC" && "$AIR_PC" != "0x0" ]]; then
    echo "=== Resolving Airbender PC to function name ==="
    echo "PC: $AIR_PC"

    if [[ -f "$AIR_ELF" ]]; then
        # Use addr2line to resolve PC to function name
        FUNC_INFO=$(addr2line -f -e "$AIR_ELF" "$AIR_PC" 2>/dev/null)
        if [[ -n "$FUNC_INFO" ]]; then
            echo "Function:"
            echo "$FUNC_INFO" | head -1  # Function name
            echo "Location:"
            echo "$FUNC_INFO" | tail -1  # File:line
        else
            # Try with llvm-addr2line
            FUNC_INFO=$(llvm-addr2line -f -e "$AIR_ELF" "$AIR_PC" 2>/dev/null)
            if [[ -n "$FUNC_INFO" ]]; then
                echo "Function:"
                echo "$FUNC_INFO" | head -1
                echo "Location:"
                echo "$FUNC_INFO" | tail -1
            else
                echo "Could not resolve PC (try: addr2line -f -e $AIR_ELF $AIR_PC)"
            fi
        fi
    else
        echo "ELF file not found at $AIR_ELF"
        echo "Run ./bench-airbender-cycles.sh to build it"
    fi
    echo ""
fi

# Summary
echo "=== Summary ==="
echo ""
if [[ "$AIR_COUNT" -gt "$ZISK_COUNT" ]]; then
    echo "Airbender makes $((AIR_COUNT - ZISK_COUNT)) MORE queries than Zisk"
    echo "First extra/different query is #$DIFF_LINE"
elif [[ "$AIR_COUNT" -lt "$ZISK_COUNT" ]]; then
    echo "Zisk makes $((ZISK_COUNT - AIR_COUNT)) MORE queries than Airbender"
    echo "First extra/different query is #$DIFF_LINE"
else
    echo "Both have $AIR_COUNT queries but they differ at query #$DIFF_LINE"
fi
echo ""

# Show the specific divergent queries
echo "=== Divergent queries ==="
echo ""
echo "Airbender query #$DIFF_LINE:"
sed -n "${DIFF_LINE}p" /tmp/air_queries_full.txt | cut -d' ' -f2-
echo ""
echo "Zisk query #$DIFF_LINE (if exists):"
sed -n "${DIFF_LINE}p" /tmp/zisk_queries_full.txt | cut -d' ' -f2- || echo "(no query at this position)"
