# ZisK Integration Changes

Summary of changes in `zisk-integration` branch vs `forkbase` tag (commit ef2f9f944, "Fix clz opcode").

**Stats:** 84 files changed, ~2100 insertions, ~500 deletions

## Semantic Groupings

### 1. RV64 Architecture Support (Core)

**Files:**
- `cycle_marker/src/lib.rs` - Add `target_arch = "riscv64"` alongside `riscv32` in all `#[cfg]` attributes
- `crypto/src/lib.rs` - Enable bigint delegation for RV64
- `crypto/src/sha3/mod.rs` - Route RV64 to delegated Keccak
- `crypto/src/bigint_delegation/delegation.rs` - Enable for RV64
- `crypto/src/secp256k1/**` - RV64 cfg additions for field/scalar operations
- `crypto/src/secp256r1/**` - RV64 cfg additions
- `crypto/src/bls12_381/**` - RV64 cfg additions for BLS12-381 curves/fields
- `crypto/src/bn254/**` - RV64 cfg additions for BN254
- `crypto/src/ark_ff_delegation/biginteger/mod.rs` - Fix slice indexing for RV64 (safer pointer arithmetic)
- `supporting_crates/delegated_u256/src/*.rs` - RV64 cfg additions
- `supporting_crates/keccak/src/lib.rs` - RV64 cfg additions
- `supporting_crates/u256/src/lib.rs` - RV64 cfg additions
- `forward_system/src/lib.rs` - RV64 cfg additions
- `system_hooks/src/**` - RV64 cfg additions

**Pattern:** Most changes are mechanical `riscv32` → `any(riscv32, riscv64)` cfg changes.

### 2. ZisK Keccak Precompile

**Files:**
- `crypto/Cargo.toml` - Add `zisk_keccak` feature
- `crypto/src/sha3/delegated/mod.rs` - Add routing for `zisk_keccak` feature
- `crypto/src/sha3/delegated/zisk_precompile.rs` - **New file**: CSR 0x800 syscall wrapper

**Purpose:** Routes Keccak-f1600 permutations to ZisK's native precompile instead of software computation. When the guest writes a state address to CSR 0x800, ZisK performs Keccak-f1600 in the host and generates a native proof table entry.

### 3. Build System for Multi-Target

**Files:**
- `zksync_os/build.sh` - **New file**: Unified build script supporting airbender/qemu/zisk targets (209 lines)
- `zksync_os/.cargo/config.toml` - Default to RV64 target (`riscv64im-unknown-none-elf.json`), add `build-std`
- `zksync_os/src/lds/memory-zisk.x` - **New file**: ZisK memory layout (ROM at 0x80000000, RAM at 0xa0000000)
- `zksync_os/src/lds/memory-airbender.x` - **New file**: Airbender memory layout (ROM at 0, RAM at 2M)
- `zksync_os/src/lds/link-512m.x` - **New file**: 512MB linker script for 64-bit (16M stack, 440M heap)

**Memory layouts:**
- ZisK: ROM 0x80000000 (128M), RAM 0xa0000000 (512M)
- Airbender: ROM 0x0 (2M), RAM 0x200000 (1022M)

### 4. 64-bit UART Support

**Files:**
- `zksync_os/src/quasi_uart.rs` - Dual implementation based on `target_pointer_width`:
  - 32-bit (Airbender): CSR-based QuasiUART protocol
  - 64-bit (ZisK): Memory-mapped UART at 0xa0000200
- `zksync_os/src/main.rs` - Minor adjustments for 64-bit compatibility

### 5. ZisK Witness Generation Bridge

**Files:**
- `tests/rig/src/zisk_bridge.rs` - **New file** (383 lines): Bridge between `ZkEENonDeterminismSource` and ZisK's oracle callback
- `tests/rig/src/lib.rs` - Export zisk_bridge module
- `tests/rig/src/chain.rs` - Add `run_block_generate_witness_zisk()` and significant refactoring (~400 lines changed)
- `tests/rig/Cargo.toml` - Add `zisk-witness` feature with `ziskemu` and `zisk-core` dependencies

**Purpose:** Allows running ZisK emulator with zksync-os's oracle for 64-bit witness generation. Handles the protocol translation between 32-bit oracle responses and 64-bit guest reads.

### 6. eth_runner Enhancements

**Files:**
- `tests/instances/eth_runner/src/main.rs` - Add `SingleEthRun` command with `--skip-witness` flag
- `tests/instances/eth_runner/src/single_run.rs` - ZisK witness generation support
- `tests/instances/eth_runner/src/ethproofs.rs` - Refactored for better abstraction
- `tests/instances/eth_runner/src/live_run/mod.rs` - Minor adjustments
- `tests/instances/eth_runner/Cargo.toml` - Feature flags:
  - `zisk-witness` - Enables ZisK witness generation
  - `cycle_marker` - Enables cycle marker output for Airbender simulator
  - Changed default features to `["evm_replay"]`

### 7. Test Data

**Files:**
- `tests/instances/eth_runner/blocks/24198369/*` - New block data (account_diffs.json, blobs.json, block.json, block_hashes.json, calltrace.json, difftrace.json, prestatetrace.json, receipts.json, witness.json)

### 8. Workspace and Dependency Configuration

**Files:**
- `Cargo.toml` - Patch section points airbender crates to local `zksync_os/zksync-airbender/` paths
- `zksync_os/Cargo.toml` - Add `zisk_keccak` feature, patch airbender crates
- `proof_running_system/Cargo.toml` - Remove `lock_api` dependency, simplify `talc` features

### 9. Submodule Configuration

**Files:**
- `.gitmodules` - Add inner `zksync_os/zksync-airbender` submodule
- `zksync_os/zksync-airbender` - Submodule reference (fork with RV64 support)

### 10. Binary Symlinks

**Files:**
- `zksync_os/app.bin` - Converted from large binary to symlink (18 bytes)
- `zksync_os/app.elf` - Converted from large binary to symlink (18 bytes)

---

## Changes That Could Be Simplified

### 1. quasi_uart.rs Duplication
The 32-bit and 64-bit implementations share structure but are fully duplicated with `#[cfg(target_pointer_width = "...")]`. Could use a trait or macro to reduce code.

### 2. cfg Attribute Proliferation
Many files have `#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]`. A cfg alias like `target_family = "riscv"` or a feature flag would reduce repetition.

### 3. zisk_bridge.rs Complexity
The bridge has extensive debug logging and state tracking (383 lines). Some of this could be simplified once the integration is stable.

### 4. Build Script Size
`build.sh` is 209 lines. The machine-specific configurations could be extracted to separate config files.

---

## Notes

- **Blake2s delegation** (CSR 0x7C7) support was removed from ZisK as it is not needed for Ethereum execution. The current implementation uses Keccak for the MPT.
- **U256 delegation** (CSR 0x7CA) is still supported for BigInt arithmetic operations.
- **Keccak** uses CSR 0x800 for ZisK's native precompile (different from Airbender's CSR 0x7CB delegation protocol).

---

## Dependency Rationale

### Why zksync-os depends on VM-specific crates

Both Airbender and ZisK define the oracle callback interface that zksync-os must implement. This creates unavoidable coupling where the VM defines the interface and zksync-os implements adapters for it.

#### Airbender Dependencies (structural, always required)

| Crate | Used By | Purpose |
|-------|---------|---------|
| `risc_v_simulator` | oracle_provider, zksync_os_runner, rig, cycle_marker | Defines `NonDeterminismCSRSource` trait, `MemorySource` trait, memory implementations |
| `riscv_transpiler` | zksync_os_runner, rig | VM types, `RamPeek` trait |

The oracle's core interface IS the Airbender trait—`oracle_provider/src/lib.rs` implements `NonDeterminismCSRSource<M> for ZkEENonDeterminismSource`. This is deeply embedded because Airbender was the original/only target.

#### ZisK Dependencies (optional, for test/witness generation)

| Crate | Used By | Purpose |
|-------|---------|---------|
| `ziskemu` | rig (feature-gated) | `OracleCallback` type, `OracleOp` enum, `ZiskMemoryReader` trait, `ZiskEmulator` |
| `zisk-core` | rig (feature-gated) | `Riscv2zisk` for ELF→ROM conversion |

These are behind the `zisk-witness` feature flag in `tests/rig`. The dependency exists because `rig` contains a convenience function (`run_block_generate_witness_zisk`) that:
1. Creates the bridge adapting zksync-os oracle → ZisK callback
2. Converts ELF to ZisK ROM format
3. Runs the ZisK emulator
4. Returns witness data

**Could this be cleaner?** Yes—the ROM conversion (#2) and emulator execution (#3) are ZisK toolchain concerns, not zksync-os concerns. Ideally:
- zksync-os would only export the bridge types (`ZiskOracleBridge`, `ZiskMemorySource`)
- ZisK would handle ROM loading and emulator execution

However, since this is test/development code (not production), the current structure is acceptable. The `zisk-witness` feature is disabled by default, so builds that don't need ZisK witness generation don't pull these dependencies.

#### Symmetry Comparison

| Aspect | Airbender | ZisK |
|--------|-----------|------|
| Crates affected | 7 (oracle_provider, zksync_os_runner, etc.) | 1 (rig only) |
| Optional? | No, always required | Yes, behind `zisk-witness` feature |
| Depth | Core—defines oracle interface traits | Surface—only in test harness |
