#!/bin/bash
# Map production addresses to source lines using debug ELF
#
# Usage: ./addr2line.sh <production_address>
# Example: ./addr2line.sh 0x8006e388
#
# Requires both zksync_os_zisk.elf (production) and zksync_os_zisk_debug.elf (debug)

set -e

# Get the directory where this script lives
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PROD_ELF="$SCRIPT_DIR/zksync_os_zisk.elf"
DEBUG_ELF="$SCRIPT_DIR/zksync_os_zisk_debug.elf"

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <production_address>"
    echo "Example: $0 0x8006e388"
    exit 1
fi

# Normalize address to have 0x prefix
PROD_ADDR="$1"
if [[ ! "$PROD_ADDR" =~ ^0[xX] ]]; then
    PROD_ADDR="0x$PROD_ADDR"
fi
PROD_ADDR_DEC=$((PROD_ADDR))

if [[ ! -f "$PROD_ELF" ]]; then
    echo "Error: Production ELF not found: $PROD_ELF"
    echo "Run: ./build.sh --machine zisk"
    exit 1
fi

if [[ ! -f "$DEBUG_ELF" ]]; then
    echo "Error: Debug ELF not found: $DEBUG_ELF"
    echo "Run: ./build.sh --machine zisk --debug"
    exit 1
fi

# Find the function containing this address in production build
# Get all symbols sorted by address, find the one just before our target
read -r FUNC_ADDR_DEC FUNC_NAME < <(riscv64-elf-nm -n "$PROD_ELF" 2>/dev/null | \
    awk -v target="$PROD_ADDR_DEC" '
    {
        addr = strtonum("0x" $1)
        if (addr <= target) {
            last_addr = addr
            last_name = $3
        }
        if (addr > target && last_name != "") {
            print last_addr, last_name
            exit
        }
    }
    END {
        if (last_name != "") print last_addr, last_name
    }')

if [[ -z "$FUNC_NAME" ]]; then
    echo "Error: Could not find symbol containing address $PROD_ADDR"
    exit 1
fi

OFFSET=$((PROD_ADDR_DEC - FUNC_ADDR_DEC))

# Demangle for display
FUNC_DEMANGLED=$(echo "$FUNC_NAME" | c++filt -_ 2>/dev/null || echo "$FUNC_NAME")

echo "Production address: $PROD_ADDR"
echo "Function: $FUNC_DEMANGLED"
printf "Offset within function: +0x%x\n" "$OFFSET"
echo ""

# Find the same symbol in debug build
# Escape special regex characters in symbol name
FUNC_NAME_ESCAPED=$(printf '%s' "$FUNC_NAME" | sed 's/[][$.*^\\]/\\&/g')

# First try exact match
DEBUG_FUNC_ADDR=$(riscv64-elf-nm "$DEBUG_ELF" 2>/dev/null | grep -E "^[0-9a-f]+ [tT] ${FUNC_NAME_ESCAPED}$" | awk '{print $1}')

# If not found, try matching without the Rust hash suffix (17h...E)
if [[ -z "$DEBUG_FUNC_ADDR" ]]; then
    # Strip the hash suffix from the production symbol
    FUNC_NAME_NO_HASH=$(echo "$FUNC_NAME" | sed 's/17h[0-9a-fA-F]*E$//')
    if [[ "$FUNC_NAME_NO_HASH" != "$FUNC_NAME" ]]; then
        # Escape and search for a symbol with the same prefix (different hash)
        FUNC_NAME_NO_HASH_ESCAPED=$(printf '%s' "$FUNC_NAME_NO_HASH" | sed 's/[][$.*^\\]/\\&/g')
        DEBUG_FUNC_ADDR=$(riscv64-elf-nm "$DEBUG_ELF" 2>/dev/null | grep -E "^[0-9a-f]+ [tT] ${FUNC_NAME_NO_HASH_ESCAPED}17h[0-9a-fA-F]+E$" | head -1 | awk '{print $1}')
    fi
fi

if [[ -z "$DEBUG_FUNC_ADDR" ]]; then
    echo "Error: Symbol '$FUNC_NAME' not found in debug build"
    echo "The function may have been inlined or renamed"
    exit 1
fi

DEBUG_FUNC_ADDR_DEC=$((16#$DEBUG_FUNC_ADDR))
DEBUG_TARGET=$((DEBUG_FUNC_ADDR_DEC + OFFSET))
DEBUG_TARGET_HEX=$(printf '0x%x' "$DEBUG_TARGET")

echo "Debug function address: 0x$DEBUG_FUNC_ADDR"
echo "Debug target address: $DEBUG_TARGET_HEX"
echo ""
echo "=== Source Location (innermost first) ==="
addr2line -e "$DEBUG_ELF" -f -C -i "$DEBUG_TARGET_HEX" 2>/dev/null || \
    riscv64-elf-addr2line -e "$DEBUG_ELF" -f -C -i "$DEBUG_TARGET_HEX" 2>/dev/null || \
    echo "addr2line not available or failed"
