# ZisK Integration Changes

Summary of changes in `zisk-squash` branch vs `upstream/popzxc/more-ethproofs-fusaka`.

## Semantic Groupings

### 1. RV64 Architecture Support (Core)

**Files:**
- `cycle_marker/src/lib.rs` - Add `target_arch = "riscv64"` alongside `riscv32` in all `#[cfg]` attributes
- `crypto/src/lib.rs` - Enable bigint delegation for RV64
- `crypto/src/sha3/mod.rs` - Route RV64 to delegated Keccak
- `zksync_os/src/main.rs` - Remove 32-bit usize assertions, add comments about 64-bit truncation
- `zksync_os/src/quasi_uart.rs` - Dual implementation: CSR-based (32-bit) vs memory-mapped UART (64-bit)

**Pattern:** Most changes are mechanical `riscv32` → `any(riscv32, riscv64)` cfg changes.

### 2. ZisK Keccak Precompile

**Files:**
- `crypto/src/sha3/delegated/mod.rs` - Add routing for `zisk_keccak` feature
- `crypto/src/sha3/delegated/zisk_precompile.rs` - **New file**: CSR 0x800 syscall wrapper

**Purpose:** Routes Keccak-f1600 permutations to ZisK's native precompile instead of software computation.

### 3. Build System for Multi-Target

**Files:**
- `zksync_os/build.sh` - **New file**: Unified build script supporting airbender/qemu/zisk targets
- `zksync_os/build_witness.sh` - **New file**: Witness generation build
- `zksync_os/.cargo/config.toml` - Default to RV64 target, add build-std
- `zksync_os/src/lds/memory-zisk.x` - **New file**: ZisK memory layout (RAM at 0xa0000000)
- `zksync_os/src/lds/memory-zisk-debug.x` - **New file**: Debug variant
- `zksync_os/src/lds/memory-qemu.x` - **New file**: QEMU memory layout
- `zksync_os/src/lds/memory-airbender.x` - **New file**: Airbender memory layout
- `zksync_os/src/lds/link-512m.x` - **New file**: 512MB linker script for 64-bit

### 4. ZisK Witness Generation Bridge

**Files:**
- `tests/rig/src/zisk_bridge.rs` - **New file**: Bridge between ZkEENonDeterminismSource and ZisK's oracle callback
- `tests/rig/src/lib.rs` - Export zisk_bridge module
- `tests/rig/src/chain.rs` - Add `run_block_generate_witness_zisk()` method
- `tests/rig/Cargo.toml` - Add `zisk-witness` feature with ziskemu dependency

**Purpose:** Allows running ZisK emulator with zksync-os's oracle for 64-bit witness generation.

### 5. eth_runner Enhancements

**Files:**
- `tests/instances/eth_runner/src/main.rs` - Add `SingleEthRun` command with `--skip-witness` flag
- `tests/instances/eth_runner/src/single_run.rs` - ZisK witness generation support
- `tests/instances/eth_runner/Cargo.toml` - Feature flags for rig integration

### 6. Test Data

**Files:**
- `tests/instances/eth_runner/blocks/19299001/*` - New block data
- `tests/instances/eth_runner/blocks/24198369/witness.json` - Updated witness
- `tests/instances/eth_runner/*_witness` - Witness files for various blocks
- `tests/instances/eth_runner/witness_*.bin` - Binary witness files

### 7. Submodule Configuration

**Files:**
- `.gitmodules` - Add inner `zksync_os/zksync-airbender` submodule pointing to fork
- `zksync_os/zksync-airbender` - Submodule reference

### 8. Documentation

**Files:**
- `ZISK_GLOBAL_ALLOC_ISSUE.md` - Documents global allocator panic issue

### 9. Minor/Formatting Changes

**Files:**
- `basic_bootloader/.../blob_eval_precompile.rs` - Formatting + use `<Fr as PrimeField>::BigInt` instead of `crypto::BigInt`
- Various crypto files - Minor cfg adjustments
- `tests/fuzzer/Cargo.lock`, `tests/fuzzer/fuzz/Cargo.lock` - Lock file updates

---

## Changes That Could Be Simplified

### 1. quasi_uart.rs Duplication
The 32-bit and 64-bit implementations share structure but are fully duplicated. Could use a trait or macro to reduce code.

### 2. cfg Attribute Proliferation
Many files have `#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]`. A cfg alias like `target_family = "riscv"` or a feature flag would reduce repetition.

### 3. zisk_bridge.rs Complexity
The bridge has extensive debug logging and state tracking. Some of this could be simplified once the integration is stable.

### 4. Build Script
`build.sh` is 209 lines. The machine-specific configurations could be extracted to separate config files.

---

## Inessential Changes (Could Be Removed)

### 1. Debug Print in main.rs
```rust
let _ = LoggerTy::default().write_fmt(format_args!("PROOF IT WORKS\n"));
```
This debug message is not needed for production.

### 2. ZISK_GLOBAL_ALLOC_ISSUE.md
This documents a resolved issue. Can be removed once the fix is merged.

### 3. Test Data Files
Large binary witness files and some block data could be generated on-demand rather than committed:
- `tests/instances/eth_runner/witness_*.bin`
- `tests/instances/eth_runner/*_witness`

### 4. Verbose Logging in zisk_bridge.rs
The `VERBOSE_BRIDGE` and `VERBOSE_ORACLE` env var checks add overhead and could be compile-time features.

### 5. Duplicate Memory Layouts
`memory-qemu.x` may not be needed if QEMU isn't actively used for testing.

### 6. Inner Submodule (zksync_os/zksync-airbender)
This is a fork reference. Once upstream airbender has RV64 support, this can point to upstream and the .gitmodules entry can be simplified.

### 7. Cargo.lock Changes in Fuzzer
The large diffs in `tests/fuzzer/Cargo.lock` and `tests/fuzzer/fuzz/Cargo.lock` are dependency updates unrelated to ZisK.
