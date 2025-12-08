# Integrating ZKsyncOS as a guest program

# Introduction
ZKsyncOS is a guest program that is designed to be generic over enough parameters to prove the state transition functions of both ZKsync and Ethereum. These are bundled by [`SystemTypes`](../zk_ee/src/system/mod.rs). Parameters include:

- A VM (see [`ExecutionEnvironment`](../zk_ee/src/system/execution_environment/mod.rs) trait)
- A state model (see [`IOSubsystem`](../zk_ee/src/system/io.rs), [`SystemIOTypesConfig`](../zk_ee/src/types_config/mod.rs))
- A resource metering model (see [`Resources`](../zk_ee/src/system/resources.rs) trait)

ZKsyncOS is an operating system in the sense that it manages requests for resources made by a running program. Those resources include:
- Storage and account state
- Transient execution memory
- Call frame management to handle reversions
- A resource metering service
- Queries to external oracles to assist in proving
- Ports for communicating with external blockchain layers

ZKsyncOS defines an API that should allow proving EVM execution in any ZKVM based on RISC-V. As far we know, only ZKsync Airbender has beeen used to prove ZKsyncOS, and we expect complications beyond what's written here would arise when using a different ZKVM. That said, ZKsyncOS documentation is quite good and it seems a good amount of thought was put into the design.

  1. Adapt memory layout
    - Disassemble binary, verify symbols at expected addresses
    - Run in target simulator, check memory accesses stay in bounds
    - Intentionally corrupt regions to confirm isolation
  2. Implement host-guest IO
    - Minimal test binary: write to oracle, read back, verify round-trip
    - Test protocol state machine in isolation (unit test the oracle handler)
    - Fuzz the read/write sequence, check no hangs or panics
  3. Implement delegations
    - Unit test each delegation against known test vectors (e.g., U256 mul,
  blake2s)
    - Compare outputs to reference Rust implementations
    - Edge cases: zero, max values, overflow conditions
  4. Build for minimal target
    - Disassemble binary, grep for forbidden instructions
    - Static analysis pass to reject unexpected opcodes
    - Run in strict simulator that traps on unsupported instructions
  5. Create entry point
    - End-to-end: single no-op transaction, compare output hash to reference
    - Incrementally add tx types: transfer, contract call, deploy
    - Replay known mainnet/testnet blocks, diff against canonical results

[toc]

# Integration Plan

As currently configured, ZKsyncOS links against Airbender's `riscv_common`, which provides a framework for oracle communication (`csr_read_word`, `csr_write_word`), exit functions (`zksync_os_finish_success`, `zksync_os_finish_error`), and a panic handler.

In brief, the task of integration for an RV32IM VM is one of updating linker scripts, implementing handling of certain `csrrw` instructions for interacting with oracles, implementing those oracles, and handling program exit signals. 

| Symbol                                     | Purpose                             | Airbender Reference           |
|--------------------------------------------|-------------------------------------|-------------------------------|
| `csr_read_word() -> u32`                   | Read next word from oracle response | `riscv_common/src/lib.rs:17`  |
| `csr_write_word(usize)`                    | Write word to oracle query buffer   | `riscv_common/src/lib.rs:5`   |
| `rust_abort() -> !`                        | Called on panic                     | `riscv_common/src/lib.rs:31`  |
| `zksync_os_finish_error() -> !`            | Signal proof failure                | `riscv_common/src/lib.rs:37`  |
| `zksync_os_finish_success(&[u32; 8]) -> !` | Signal success with output          | `riscv_common/src/lib.rs:54`  |
| `#[panic_handler]`                         | Rust panic handler                  | `riscv_common/src/lib.rs:111` |
| `#[global_allocator]`                      | Heap allocator                      | `riscv_common/src/lib.rs:138` |



## 1. Set guest memory layout 

The memory layout is defined in two linker scripts:
- `zksync_os/src/lds/memory.x` - Defines memory regions
- `zksync_os/src/lds/link.x` - Defines section placement and sizes

These must be changed to be compatible with the ZKVMs memory layout expectations.

Here is the default layout supported by Airbender:

```
0x40000000  ┌──────────────────────────┐  (1 GB total addressable)
            │        Unused            │
0x36200000  ├──────────────────────────┤  _eheap
            │                          │
            │     HEAP (768 MB)        │  ↑ grows upward
            │                          │
0x06200000  ├──────────────────────────┤  _sheap / _sstack (SP init)
            │                          │
            │     STACK (64 MB)        │  ↓ grows downward
            │                          │
0x00200000  ├──────────────────────────┤  _estack (stack limit) / RAM start
            │     ROM (2 MB)           │  ← zksync_os.bin loaded here
0x00000000  └──────────────────────────┘  entry point
```

| Assumption        | Default Value     | Location     |
|-------------------|-------------------|--------------|
| ROM origin        | `0x00000000`      | `memory.x:3` |
| ROM size          | 2 MB              | `memory.x:3` |
| RAM origin        | `0x00200000` (2M) | `memory.x:4` |
| Total addressable | 1 GB              | `memory.x:4` |
| Stack size        | 64 MB             | `link.x:3`   |
| Heap size         | 768 MB            | `link.x:4`   |
| Heap alignment    | 2 MB              | `link.x:113` |

## 2. Extend ISA

To follow the existing pattern, your ZKVM must support RV32IM plus `csrrw` instructions for host-guest communication. No other instruction from the privileged architecture is needed.

Aside from its dependency on Airbenders riscv_common crate, ZKsyncOS uses the `csrrw` instruction in exactly the following places, none of which is essential:
  1. cycle_marker/src/lib.rs (2 uses of 0x7ff)
  Purpose: Debug/profiling markers to annotate execution traces
  - Gated with #[cfg(feature = "cycle_marker")]
  - Also architecture-gated with #[cfg(target_arch = "riscv32")]

  2. zksync_os/src/asm/asm_reduced.S (5 uses of mscratch/mepc)
  Purpose: Machine-mode trap handling for unaligned memory access emulation
  - Can be disabled by #[cfg(not(feature = "no_exception_handling"))] in machine_trap.rs
  - If your ZKVM natively supports unaligned memory access, enable no_exception_handling feature
  - The entry point (_start) itself uses zero CSRs - only the trap handler does
  - Alternative: a ZKVM could provide its own trap mechanism or handle misalignment in hardware

  3. supporting_crates/delegated_u256/src/delegation.rs (1 use of 0x7ca)
  Purpose: Delegate U256 bigint operations to a specialized prover circuit for efficiency

Airbender's CSR-based approach uses `csrrw` for:
- IO oracle queries (storage, tx data, preimages)
- Non-determinism hints for witness construction 
- Delegations (precompiles offloaded to specialized circuits). Note that the use of delegations is gates behind `--features proving`.

## 3. 
The oracle query protocol is responsible for fetching external data that the guest program cannot compute on its own—transaction inputs, storage values, preimages, and computational advice. The guest writes a query to the host, the host processes it and prepares a response, then the guest reads the result back via CSR. References in ZKsyncOS: guest side (makes queries): [`proof_running_system/src/io_oracle/mod.rs`](proof_running_system/src/io_oracle/mod.rs) — `CsrBasedIOOracle`; host side (dispatches queries): [`oracle_provider/src/lib.rs`](oracle_provider/src/lib.rs) — `ZkEENonDeterminismSource`; query ID definitions [`zk_ee/src/oracle/query_ids.rs`](zk_ee/src/oracle/query_ids.rs). In summary:

```
┌─────────────────────────────────────────────────────────────┐
│                    QUERY PROTOCOL                           │
├─────────────────────────────────────────────────────────────┤
│  Guest writes (via CSR 0x7c0):                              │
│    1. query_type   (u32) - identifies the query             │
│    2. input_len    (u32) - number of input words            │
│    3. input[0..n]  (u32 each) - query parameters            │
│                                                             │
│  Host responds (guest reads via CSR 0x7c0):                 │
│    1. result_len   (u32) - number of result words           │
│    2. result[0..n] (u32 each) - response data               │
└─────────────────────────────────────────────────────────────┘
```

Integration points in Airbender to model
  1. CSR interception (bridging to oracle):
  - risc_v_simulator/src/cycle/state.rs:1171-1301 — intercepts CSR 0x7c0 reads/writes
  - Calls non_determinism_source.read() and write_with_memory_access()

  2. Oracle data wiring:
  - risc_v_simulator/src/abstractions/non_determinism.rs:27-48 — QuasiUARTSource holds oracle data as VecDeque<u32>
  - tools/cli/src/prover_utils.rs:275-279 — populates the oracle with batch data

  3. External data sources:
  - tools/cli/src/main.rs:192-218 — fetches from RPC (anvil_zks_getBoojumWitness)
  - execution_utils/src/verifiers.rs:44-148 — assembles oracle data from proof metadata
  - verifier_common/src/proof_flattener.rs — flattens proofs/queries into u32 sequences

### Query Dependencies

While not strictly enforced, the expected query order is:
1. `ZK_PROOF_DATA_INIT` - Called once at startup
2. `INITIAL_STATE_COMMITMENT` - Called once after init
3. For each transaction:
   - `NEXT_TX_SIZE`
   - `TX_DATA_WORDS` (possibly multiple calls)
   - `TX_ENCODING_FORMAT`
   - Various storage/preimage queries during execution
4. `DISCONNECT_ORACLE` - Called when transitioning to autonomous mode

### DISCONNECT_ORACLE Behavior

After `DISCONNECT_ORACLE` (0x40000000) is processed:
- The oracle enters "disconnected" state
- Subsequent CSR writes are silently ignored
- Subsequent CSR reads return `0`
- The oracle cannot reconnect; this is a one-way transition
- zksync-os continues execution in "autonomous mode" (no more oracle queries)

### Query Types

Your oracle must handle these query types (minimum viable set). The "Handler Reference" column points to existing zksync-os implementations that ZKVMs can reuse directly—these are not Airbender-specific and handle the query logic independently of the proving system.

#### Critical Queries (Required)

| Query ID     | Name                         | Input         | Output                         | Handler Reference |
|--------------|------------------------------|---------------|--------------------------------|-------------------|
| `0x40070001` | `ZK_PROOF_DATA_INIT`         | None          | Batch metadata, initial state  | [`forward_system/src/run/query_processors/zk_proof_data.rs`](forward_system/src/run/query_processors/zk_proof_data.rs) |
| `0x40040000` | `INITIAL_STATE_COMMITMENT`   | None          | Initial state root (8 x u32)   | [`forward_system/src/run/query_processors/read_tree.rs`](forward_system/src/run/query_processors/read_tree.rs) |
| `0x40060000` | `NEXT_TX_SIZE`               | None          | Transaction size in bytes      | [`forward_system/src/run/query_processors/tx_data.rs`](forward_system/src/run/query_processors/tx_data.rs) |
| `0x40060001` | `TX_DATA_WORDS`              | offset, count | Transaction data               | [`forward_system/src/run/query_processors/tx_data.rs`](forward_system/src/run/query_processors/tx_data.rs) |
| `0x40060002` | `TX_ENCODING_FORMAT`         | None          | Encoding format ID             | [`forward_system/src/run/query_processors/tx_data.rs`](forward_system/src/run/query_processors/tx_data.rs) |
| `0x40060003` | `TX_FROM`                    | None          | Sender address (5 x u32)       | [`forward_system/src/run/query_processors/tx_data.rs`](forward_system/src/run/query_processors/tx_data.rs) |
| `0x40030000` | `INITIAL_STORAGE_SLOT_VALUE` | address, slot | Storage value (8 x u32)        | [`forward_system/src/run/query_processors/read_storage.rs`](forward_system/src/run/query_processors/read_storage.rs) |
| `0x40020000` | `GENERIC_PREIMAGE`           | hash          | Preimage data                  | [`forward_system/src/run/query_processors/generic_preimage.rs`](forward_system/src/run/query_processors/generic_preimage.rs) |
| `0x40000000` | `DISCONNECT_ORACLE`          | None          | Empty (switches to autonomous) | [`oracle_provider/src/lib.rs:94`](oracle_provider/src/lib.rs) |

#### Advice Queries (For Precompiles)

| Query ID     | Name                        | Purpose                       | Handler Reference                                                                                    |
|--------------|-----------------------------|-------------------------------|------------------------------------------------------------------------------------------------------|
| `0x40050010` | `MODEXP_ADVICE`             | Modular exponentiation result | [`callable_oracles/src/arithmetic/mod.rs`](callable_oracles/src/arithmetic/mod.rs)                   |
| `0x40050020` | `BLOB_COMMITMENT_AND_PROOF` | KZG commitment verification   | [`callable_oracles/src/blob_kzg_commitment/mod.rs`](callable_oracles/src/blob_kzg_commitment/mod.rs) |

#### Debug (Optional)

| Query ID     | Name   | Purpose                   | Handler Reference |
|--------------|--------|---------------------------|-------------------|
| `0xFFFFFFFF` | `UART` | Debug output (write-only) | [`forward_system/src/run/query_processors/uart_print.rs`](forward_system/src/run/query_processors/uart_print.rs) |

#### ZK_PROOF_DATA_INIT Response Format

The exact word layout for `ZK_PROOF_DATA_INIT` (0x40070001) response:

```
Word 0-1:   batch_number (u64, little-endian)
Word 2-3:   timestamp (u64, little-endian)
Word 4-5:   l1_gas_price (u64, little-endian)
Word 6-7:   fair_l2_gas_price (u64, little-endian)
Word 8-15:  base_system_contract_hash_0 (32 bytes as 8 × u32)
Word 16-23: base_system_contract_hash_1 (32 bytes as 8 × u32)
Word 24:    num_transactions (u32)
```

#### TX_ENCODING_FORMAT Values

Known encoding format IDs:
- `0` - Legacy transaction format
- `1` - EIP-2930 (access list)
- `2` - EIP-1559 (fee market)
- `113` (0x71) - EIP-712 (zkSync native)

The format ID determines how `TX_DATA_WORDS` content should be interpreted.

---

## 4. Exit/Completion Detection

**Success**: Guest enters infinite loop with output in registers x10-x17

```
Detection algorithm:
1. Track instruction pointer (PC)
2. If PC hasn't changed for N cycles AND last instruction was a branch to self
3. Read x10-x17 as the 256-bit public output
```

**Output register mapping**:
```
x10 = output[0]  (least significant)
x11 = output[1]
x12 = output[2]
x13 = output[3]
x14 = output[4]
x15 = output[5]
x16 = output[6]
x17 = output[7]  (most significant)
```

**Error**: Write to CSR `cycle` (0xC00)
```
If CSR write to address 0xC00 detected → abort with error
```

---

## 5. Data Serialization Format

All data through the oracle uses **usize serialization** (32-bit words, little-endian):

```rust
// 20-byte address serialized as 5 x u32
address[0..4]   → word 0
address[4..8]   → word 1
address[8..12]  → word 2
address[12..16] → word 3
address[16..20] → word 4

// 32-byte hash serialized as 8 x u32
hash[0..4]   → word 0
hash[4..8]   → word 1
...
hash[28..32] → word 7

// u64 value serialized as 2 x u32
value & 0xFFFFFFFF       → word 0 (low)
(value >> 32) & 0xFFFFFF → word 1 (high)
```

---

## 8. Testing recommendations

**Phase 1: Basic Execution**
1. Load zksync_os.bin, run until first host-guest communication
2. Verify your interface is being accessed (CSR 0x7c0 if using Airbender's approach)
3. Return dummy data to see if execution progresses

**Phase 2: Query Protocol**
1. Log all oracle reads/writes
2. Verify query_type, input_len, input pattern
3. Implement `DISCONNECT_ORACLE` (return empty) to test autonomous mode

**Phase 3: Simple Transaction**
1. Prepare minimal batch (single transfer tx)
2. Implement `ZK_PROOF_DATA_INIT`, `NEXT_TX_SIZE`, `TX_DATA_WORDS`
3. Run full execution, capture x10-x17 output

**Phase 4: Full Integration**
1. Implement remaining query types
2. Run against mainnet/testnet blocks
3. Compare output hashes with reference implementation

