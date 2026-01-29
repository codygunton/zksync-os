# ZKsyncOS Changes for ZisK Integration

This document summarizes the changes made to the ZKsyncOS repository since the `forkbase` tag to support ZisK (64-bit RISC-V) proving.

## Build System

Multi-target build system supporting both ZisK (RV64) and Airbender (RV32) targets.

| Files | Description |
|-------|-------------|
| `zksync_os/build.sh` | New: Unified build script with `--machine zisk\|airbender` flag |
| `zksync_os/.cargo/config.toml` | Target configuration for RV64/RV32 |
| `zksync_os/Cargo.toml` | Target-specific features and dependencies |
| `zksync_os/Cargo.lock` | Updated dependencies |
| `zksync_os/dump_bin.sh` | Simplified, delegates to build.sh |
| `zksync_os/src/lds/link-512m.x` | New: ZisK linker script (512MB address space) |
| `zksync_os/src/lds/memory-zisk.x` | New: ZisK memory layout |
| `zksync_os/.gitignore` | Build artifact ignores |
| `zksync_os/app.bin`, `zksync_os/app.elf` | Converted to symlinks (actual files are target-specific) |
| `zksync_os/zksync-airbender` | New: Submodule for Airbender linker scripts |

## UART Support (64-bit)

64-bit UART output for ZisK target (ZisK uses 64-bit I/O registers vs Airbender's 32-bit).

| Files | Description |
|-------|-------------|
| `zksync_os/src/quasi_uart.rs` | New: 64-bit UART implementation for ZisK |
| `zksync_os/src/main.rs` | Conditional UART initialization by target |
| `proof_running_system/src/run/query_processors/uart_print.rs` | RV64 UART processing support |

## Crypto RV64 Target Support

All cryptographic code extended with RV64 target cfg attributes alongside existing RV32 support.

| Files | Description |
|-------|-------------|
| `crypto/src/lib.rs` | Target-specific feature gating |
| `crypto/src/bigint_delegation/delegation.rs` | RV64 delegation support |
| `crypto/src/bigint_delegation/u256.rs` | 256-bit integer ops |
| `crypto/src/bls12_381/curves/*.rs` | BLS12-381 curve operations (g1, g2, swu_iso, util) |
| `crypto/src/bls12_381/fields/*.rs` | BLS12-381 field operations (fq, fq2, fq6, fq12, fr) |
| `crypto/src/bn254/curves/*.rs` | BN254 curve operations |
| `crypto/src/bn254/fields/*.rs` | BN254 field operations |
| `crypto/src/secp256k1/field/*.rs` | secp256k1 field implementations |
| `crypto/src/secp256k1/scalars/*.rs` | secp256k1 scalar operations |
| `crypto/src/secp256r1/*.rs` | secp256r1 support |
| `crypto/Cargo.toml` | Dependencies and features |

## ZisK Keccak Precompile

Native Keccak hashing via ZisK precompile instead of software implementation.

| Files | Description |
|-------|-------------|
| `crypto/src/sha3/delegated/zisk_precompile.rs` | New: ZisK Keccak CSR interface |
| `crypto/src/sha3/delegated/mod.rs` | Keccak delegation routing |
| `crypto/src/sha3/mod.rs` | SHA3 module organization |
| `supporting_crates/keccak/src/lib.rs` | Keccak crate adjustments |

## Test Rig (ZisK Bridge)

Integration layer for running ZKsyncOS through ZisK emulator.

| Files | Description |
|-------|-------------|
| `tests/rig/src/zisk_bridge.rs` | New: ZisK emulator bridge for witness generation |
| `tests/rig/src/chain.rs` | Chain abstraction with ZisK support |
| `tests/rig/src/lib.rs` | Module exports |
| `tests/rig/Cargo.toml` | ZisK dependencies |

## Eth Runner

Test runner for Ethereum block execution, extended with ZisK support.

| Files | Description |
|-------|-------------|
| `tests/instances/eth_runner/src/single_run.rs` | New: Single block execution mode |
| `tests/instances/eth_runner/src/main.rs` | CLI with `single-eth-run` command |
| `tests/instances/eth_runner/src/ethproofs.rs` | Ethproofs integration |
| `tests/instances/eth_runner/src/live_run/mod.rs` | Live run mode adjustments |
| `tests/instances/eth_runner/Cargo.toml` | Dependencies |
| `tests/instances/eth_runner/blocks/24198369/*` | New: Test block data (block.json, witness.json, etc.) |

## Supporting Crates

Minor adjustments to support 64-bit targets.

| Files | Description |
|-------|-------------|
| `supporting_crates/delegated_u256/src/copy.rs` | U256 copy operations |
| `supporting_crates/delegated_u256/src/delegation.rs` | Delegation interface |
| `supporting_crates/u256/src/lib.rs` | U256 type definitions |
| `cycle_marker/src/lib.rs` | Cycle counting markers |
| `forward_system/src/lib.rs` | Forward system adjustments |

## I/O Oracle

Oracle system for external data queries during execution.

| Files | Description |
|-------|-------------|
| `proof_running_system/src/io_oracle/mod.rs` | Oracle query handling |
| `proof_running_system/Cargo.toml` | Dependencies |

## Other Changes

| Files | Description |
|-------|-------------|
| `Cargo.toml` | Workspace configuration |
| `.gitignore` | Ignore patterns |
| `.gitmodules` | Submodule configuration |
| `system_hooks/src/eip_2537/mod.rs` | EIP-2537 hook adjustments |
| `system_hooks/src/lib.rs` | System hooks organization |
| `bootloader/src/bootloader/transaction_flow/zk/mod.rs` | Minor adjustment |

## Key Changes Summary

1. **Dual-Target Support**: Build system supports both ZisK (RV64) and Airbender (RV32) with a single codebase using cfg attributes.

2. **64-bit I/O**: UART and oracle interfaces adapted for ZisK's 64-bit register width.

3. **ZisK Precompiles**: Native Keccak hashing via ZisK CSR interface for improved performance.

4. **Witness Bridge**: Integration layer enabling ZKsyncOS to run through ZisK emulator for witness generation and proving.

5. **Test Infrastructure**: Block-level testing with pre-captured Ethereum block data.
