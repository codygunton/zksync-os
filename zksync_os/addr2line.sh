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

# Demangle for display (try without -_ first for Rust symbols)
FUNC_DEMANGLED=$(echo "$FUNC_NAME" | c++filt 2>/dev/null || echo "$FUNC_NAME")

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

# Try addr2line first
ADDR2LINE_OUTPUT=$(addr2line -e "$DEBUG_ELF" -f -C -i "$DEBUG_TARGET_HEX" 2>/dev/null || \
    riscv64-elf-addr2line -e "$DEBUG_ELF" -f -C -i "$DEBUG_TARGET_HEX" 2>/dev/null || \
    echo "")

# Check if output is useful (not ??:? and not musl/libc fallback)
if [[ -n "$ADDR2LINE_OUTPUT" ]] && ! echo "$ADDR2LINE_OUTPUT" | grep -qE '\?\?:\?|musl_|:0$'; then
    echo "$ADDR2LINE_OUTPUT"
else
    echo "(addr2line returned no useful info, using line table fallback)"
    echo ""

    # Extract module path from demangled function for context
    # Function looks like: crate::module::submodule::function_name::hash
    # Strip the hash suffix first, then the function name (last segment starting with lowercase)
    MODULE_PATH=$(echo "$FUNC_DEMANGLED" | sed 's/::h[0-9a-f]*$//' | sed 's/::[a-z_][a-z0-9_]*$//')
    CRATE_NAME=$(echo "$MODULE_PATH" | sed -n 's/^\([a-zA-Z_][a-zA-Z0-9_]*\).*/\1/p')

    if [[ -n "$MODULE_PATH" ]]; then
        echo "Module: $MODULE_PATH"
        # Try to infer source file path from module path
        # crate::foo::bar -> crate/src/foo/bar.rs or crate/src/foo/bar/mod.rs
        INFERRED_PATH=$(echo "$MODULE_PATH" | sed 's/::/\//g')
        echo "Likely source: ${INFERRED_PATH}.rs or ${INFERRED_PATH}/mod.rs"

        # Try to find the source directory for this crate
        if [[ -n "$CRATE_NAME" ]]; then
            CRATE_DIR=$(readelf --debug-dump=line "$DEBUG_ELF" 2>/dev/null | grep -oE "/[^ ]*/${CRATE_NAME}(/src|$)" | head -1 | sed 's/\/src$//')
            if [[ -n "$CRATE_DIR" ]]; then
                echo "Crate dir: $CRATE_DIR"
            fi
        fi
        echo ""
    fi

    # Parse raw line info to get full paths (filename -> directory mapping)
    echo "Line table context around target:"

    # Build file->directory mapping and find entries near target
    readelf --debug-dump=rawline "$DEBUG_ELF" 2>/dev/null | \
        awk -v target="$DEBUG_TARGET_HEX" '
        BEGIN {
            target_dec = strtonum(target)
            in_dir_table = 0
            in_file_table = 0
            dir_count = 0
            file_count = 0
            n = 0
        }

        /The Directory Table/ { in_dir_table = 1; next }
        /The File Name Table/ { in_dir_table = 0; in_file_table = 1; next }
        /Line Number Statements/ { in_file_table = 0; next }

        in_dir_table && /^  [0-9]+/ {
            idx = $1
            # Get everything after the index as directory path
            dir = $0
            sub(/^  [0-9]+\t/, "", dir)
            dirs[idx] = dir
            dir_count++
        }

        in_file_table && /^  [0-9]+/ {
            # Format: Entry Dir Time Size Name
            entry = $1
            dir_idx = $2
            # Name is the last field
            name = $NF
            if (dir_idx in dirs) {
                files[entry] = dirs[dir_idx] "/" name
            } else {
                files[entry] = name
            }
            file_count++
        }

        END {
            # Now we have dirs and files arrays
            # Print them for debugging
            # for (i in files) print "FILE", i, files[i]
        }
        ' > /dev/null

    # Use a simpler approach: parse decodedline but try to identify files by context
    readelf --debug-dump=rawline "$DEBUG_ELF" 2>/dev/null | \
        awk -v target="$DEBUG_TARGET_HEX" '
        BEGIN {
            target_dec = strtonum(target)
            in_dir_table = 0
            in_file_table = 0
            n = 0
        }

        /The Directory Table/ { in_dir_table = 1; next }
        /The File Name Table/ { in_dir_table = 0; in_file_table = 1; next }
        /Line Number Statements/ { in_file_table = 0; next }

        in_dir_table && /^  [0-9]+\t/ {
            idx = $1
            dir = $0
            sub(/^  [0-9]+\t/, "", dir)
            dirs[idx] = dir
        }

        in_file_table && /^  [0-9]+\t/ {
            split($0, parts, /\t/)
            entry = parts[1]
            sub(/^  /, "", entry)
            dir_idx = parts[2]
            name = parts[5]
            if (dir_idx in dirs) {
                files[entry] = dirs[dir_idx] "/" name
            } else {
                files[entry] = name
            }
        }

        END {
            for (i in files) {
                # Simplify path for display
                path = files[i]
                gsub(/\/src\/home\/cody\//, "~/", path)
                gsub(/\.cargo\/registry\/src\/index\.crates\.io-[^\/]+\//, ".cargo/.../", path)
                gsub(/\.cargo\/git\/checkouts\/[^\/]+\/[^\/]+\//, ".cargo/.../", path)
                gsub(/\.rustup\/toolchains\/[^\/]+\/lib\/rustlib\/src\/rust\/library\//, "stdlib/", path)
                print i, path
            }
        }' | sort -n > /tmp/addr2line_files_$$

    # Now parse the decoded line table with file indices
    # Format: "filename    line    0xaddress    file_idx    [flags]"
    readelf --debug-dump=decodedline "$DEBUG_ELF" 2>/dev/null | \
        awk '/0x[0-9a-f]+/ {
            # Find address position
            for (i = 1; i <= NF; i++) {
                if ($i ~ /^0x[0-9a-f]+$/) {
                    addr = $i
                    line_num = $(i-1)
                    file_name = $1
                    # File index is right after address
                    file_idx = $(i+1)
                    if (file_idx !~ /^[0-9]+$/) file_idx = ""
                    if (line_num ~ /^[0-9]+$/) {
                        printf "%s|%s|%s|%s\n", addr, file_name, line_num, file_idx
                    }
                    break
                }
            }
        }' | sort -t'|' -k1,1 | \
        awk -F'|' -v target="$DEBUG_TARGET_HEX" -v filemap="/tmp/addr2line_files_$$" '
        BEGIN {
            target_dec = strtonum(target)
            n = 0
            # Load file map
            while ((getline < filemap) > 0) {
                idx = $1
                path = $0
                sub(/^[0-9]+ /, "", path)
                files[idx] = path
            }
            close(filemap)
        }
        {
            addr_hex = $1
            file = $2
            line = $3
            file_idx = $4
            addr = strtonum(addr_hex)

            # Try to get full path from file index
            if (file_idx != "" && file_idx in files) {
                fullpath = files[file_idx]
            } else {
                fullpath = file
            }

            addrs[n] = addr
            entries[n] = sprintf("%s  %s:%s", addr_hex, fullpath, line)
            n++
        }
        END {
            best_idx = -1
            best_addr = 0
            for (i = 0; i < n; i++) {
                if (addrs[i] <= target_dec && addrs[i] > best_addr) {
                    best_addr = addrs[i]
                    best_idx = i
                }
            }

            if (best_idx >= 0) {
                start = best_idx - 3
                if (start < 0) start = 0
                for (i = start; i < best_idx; i++) {
                    print "  " entries[i]
                }
                printf "→ %s  (TARGET)\n", entries[best_idx]
                for (i = best_idx + 1; i < n && i <= best_idx + 3; i++) {
                    print "  " entries[i]
                }
            } else {
                print "No matching line entry found"
            }
        }'

    rm -f /tmp/addr2line_files_$$
fi
