# zkSync OS API Reference

## Table of Contents

- [Introduction](#introduction)
- [Primary Rust API](#primary-rust-api)
- [RPC Integration](#rpc-integration)
- [Internal APIs](#internal-apis)
- [Type Reference](#type-reference)
- [Usage Patterns](#usage-patterns)
- [Error Handling](#error-handling)
- [Code Examples](#code-examples)

---

## Introduction

zkSync OS provides multiple API layers to support different use cases, from high-level block execution to low-level RISC-V proving. This document describes all public and internal APIs available for integrating with zkSync OS.

### API Layers Overview

zkSync OS exposes three main API layers:

1. **Primary Rust API** (`zksync_os_api` crate): High-level functions for block execution and witness generation
2. **RPC Integration**: Ethereum-compatible JSON-RPC endpoints via Alloy types
3. **Internal APIs**: Low-level interfaces for advanced users and system development

### Who Uses Each API

| API Layer | Primary Users | Use Cases |
|-----------|--------------|-----------|
| **Primary Rust API** | Sequencer nodes, RPC providers, integration tests | Block execution, witness generation, transaction simulation |
| **RPC Integration** | External clients, wallets, dApps | Standard Ethereum JSON-RPC calls, custom zkOS methods |
| **Internal APIs** | Core developers, proof systems, testing infrastructure | System development, custom execution modes, advanced testing |

---

## Primary Rust API

The primary API is exposed through the `zksync_os_api` crate (`/api/src/lib.rs`), providing high-level functions for block execution and witness generation.

### run_block()

**Location**: `/api/src/lib.rs:14` (re-exported from `/forward_system/src/run/mod.rs:65`)

**Purpose**: Execute a block of transactions in native (x86/ARM) forward execution mode. This is the primary API for sequencer nodes to produce blocks quickly without proof generation overhead.

**Signature**:
```rust
pub fn run_block<T, PS, TS, TR>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError>
where
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    TR: TxResultCallback,
```

**Generic Parameters**:

- **`T: ReadStorageTree`**: Storage tree implementation providing Merkle proofs
  - Must implement `read_storage()` for querying storage values
  - Must implement `get_leaf_proof()` for generating Merkle proofs
  - Common implementation: `InMemoryTree` from `forward_system::run::test_impl`

- **`PS: PreimageSource`**: Provider for bytecode and account data
  - Must implement `get_preimage()` for hash → data resolution
  - Must implement `contains_bytecode()` for bytecode existence checks
  - Common implementation: `InMemoryPreimageSource`

- **`TS: TxSource`**: Transaction data provider
  - Must implement `next_tx()` to stream transactions
  - Must implement `tx_from()` to get transaction sender
  - Common implementations: `TxListSource` (in-memory), custom DB sources

- **`TR: TxResultCallback`**: Callback invoked after each transaction execution
  - Must implement `on_tx_result()` to handle per-transaction results
  - Use `NoopTxCallback` if results aren't needed immediately
  - Useful for streaming results or updating databases

**Parameters**:

1. **`block_context: BlockContext`** (`BlockMetadataFromOracle`)
   - Block-level metadata: chain_id, block_number, timestamp, gas_limit, etc.
   - See [Type Reference](#blockcontext) for all fields
   - Remains constant for all transactions in the block

2. **`tree: T`**
   - Storage tree providing Merkle proofs for storage reads
   - Queries initial storage values not in cache
   - Validates state root transitions

3. **`preimage_source: PS`**
   - Provides contract bytecode and account data by hash
   - Queries on first access (e.g., EXTCODECOPY, account creation)
   - Caches results for subsequent accesses

4. **`tx_source: TS`**
   - Streams transactions to execute in order
   - Returns `NextTxResponse::Tx(transaction)` or `NextTxResponse::NoMoreTxs`
   - Transactions must be pre-validated and properly encoded

5. **`tx_result_callback: TR`**
   - Called after each transaction completes
   - Receives `TxResult` with gas usage, logs, storage changes, etc.
   - Use for real-time result processing or database updates

6. **`tracer: &mut impl Tracer<ForwardRunningSystem>`**
   - Optional execution tracer for debugging and profiling
   - Receives callbacks at key execution points (opcode execution, calls, etc.)
   - Use `NopTracer` for no tracing overhead

**Return Type**: `Result<BlockOutput, ForwardSubsystemError>`

- **Success**: Returns `BlockOutput` containing:
  - Block header with gas usage and metadata
  - Per-transaction results (status, gas, logs, storage writes)
  - Aggregate storage writes for the entire block
  - Account diffs (balance, nonce, bytecode hash changes)
  - Published preimages (new bytecode and account data)
  - Total pubdata (for L1 submission cost calculation)

- **Error**: Returns `ForwardSubsystemError` on fatal execution errors
  - Bootloader errors (invalid transaction format, etc.)
  - System errors (I/O failures, internal bugs)
  - Note: Individual transaction failures are returned in `BlockOutput`, not as errors

**Key Features**:
- **Direct External Access**: Can query databases, networks, and file systems
- **No Witness Generation**: Maximum throughput without proof overhead
- **Flexible I/O**: Integrates with any storage backend via trait implementations
- **Result Streaming**: Per-transaction callback for real-time processing

**When to Use**:
- Sequencer nodes producing blocks for the network
- RPC nodes executing historical blocks
- Development and debugging with full tooling
- Integration testing with external databases

**Performance**: Optimized for maximum throughput (~10,000 TPS target)

**Cross-references**: See [DataFlow.md](./DataFlow.md#forward-execution-flow) for detailed execution pipeline, [Architecture.md](./Architecture.md#forward-running-sequencer-mode) for execution mode details.

---

### run_block_generate_witness()

**Location**: `/api/src/lib.rs:20`

**Purpose**: Execute a block in RISC-V proof mode and return a witness (execution trace) that can be passed to the zkSNARK prover. This is the primary API for prover nodes to generate zero-knowledge proofs.

**Signature**:
```rust
pub fn run_block_generate_witness(
    block_context: BlockContext,
    tree: InMemoryTree,
    preimage_source: InMemoryPreimageSource,
    tx_source: TxListSource,
    proof_data: ProofData<StorageCommitment>,
    da_commitment_scheme: DACommitmentScheme,
    zksync_os_bin_path: &str,
) -> Vec<u32>
```

**Parameters**:

1. **`block_context: BlockContext`**
   - Same as `run_block()`: block metadata (chain_id, timestamp, etc.)

2. **`tree: InMemoryTree`**
   - In-memory storage tree implementation
   - Must contain all storage values needed for execution
   - Used for Merkle proof generation in RISC-V environment

3. **`preimage_source: InMemoryPreimageSource`**
   - In-memory preimage provider
   - Must contain all bytecode and account data needed
   - Cannot query external databases in RISC-V mode

4. **`tx_source: TxListSource`**
   - In-memory transaction list
   - All transactions provided upfront (no streaming)
   - Implementation: `TxListSource { transactions: Vec<EncodedTx> }`

5. **`proof_data: ProofData<StorageCommitment>`**
   - Previous block's proof data for continuity
   - Contains: previous state root, previous block hash, batch metadata
   - `None` for genesis block or single-block mode

6. **`da_commitment_scheme: DACommitmentScheme`**
   - Data availability commitment scheme
   - Options:
     - `DACommitmentScheme::BlobsZKsyncOS` - EIP-4844 blob commitments
     - `DACommitmentScheme::Calldata` - L1 calldata submission
     - `DACommitmentScheme::NoDA` - No data availability (testing only)

7. **`zksync_os_bin_path: &str`**
   - Path to zkSync OS RISC-V binary
   - Example: `"zksync_os/for_tests.bin"` or `"zksync_os/singleblock_batch.bin"`
   - Binary is loaded and executed in RISC-V simulator
   - See [Build System](#build-profiles) for binary generation

**Return Type**: `Vec<u32>`

Returns a flat vector of 32-bit words representing the execution witness. This witness is the complete non-deterministic input trace needed by the zkSNARK prover to reconstruct execution.

**Witness Format**:
- Stream of u32 words recording all CSR (Control & Status Register) read operations
- Structure: `[query_id, param_count, param1_low, param1_high, ..., response_len, response1_low, response1_high, ...]`
- Contains all oracle queries and responses made during execution
- Used by prover to deterministically replay execution

**Execution Flow**:
```
1. Create oracle processors (metadata, storage, transactions, etc.)
2. Wrap oracle in ReadWitnessSource to record all CSR reads
3. Load RISC-V binary via zksync_os_runner::run()
4. Execute in RISC-V simulator with oracle delegation
5. Collect witness items from ReadWitnessSource
6. Verify output is non-zero (indicates success)
7. Return witness Vec<u32>
```

**Key Features**:
- **Deterministic**: Identical inputs produce byte-for-byte identical witnesses
- **Complete**: Witness contains all data needed for proof generation
- **Isolated**: No external system access during RISC-V execution
- **Verifiable**: Witness can be verified by zkSNARK prover

**When to Use**:
- Generating ZK proofs for L1 verification
- Prover nodes processing blocks for proof submission
- Testing proof generation pipeline
- Debugging deterministic execution

**When NOT to Use**:
- Sequencer block production (use `run_block()` instead)
- RPC simulation (use `simulate_tx()` instead)
- Performance testing (RISC-V simulation is slow)

**Performance**: Slower than native execution due to RISC-V simulation overhead (~100-1000x slower depending on configuration)

**Important Notes**:
- Output `[0u32; 8]` indicates failure - assertion will panic
- All data must be provided upfront (no lazy loading)
- Binary path must be valid and readable
- Witness size can be large (millions of u32 words for complex blocks)

**Cross-references**: See [ProofSystem.md](./ProofSystem.md#witness-generation) for witness format details, [DataFlow.md](./DataFlow.md#proof-execution-flow) for RISC-V execution pipeline.

---

### simulate_tx()

**Location**: `/forward_system/src/run/mod.rs:427`

**Purpose**: Simulate a single transaction on top of given state without validation. This is the implementation for Ethereum's `eth_call` and `eth_estimateGas` RPC methods.

**Signature**:
```rust
pub fn simulate_tx<S, PS>(
    transaction: EncodedTx,
    block_context: BlockContext,
    storage: S,
    preimage_source: PS,
    tracer: &mut impl Tracer<CallSimulationSystem>,
) -> Result<TxResult, ForwardSubsystemError>
where
    S: ReadStorage,
    PS: PreimageSource,
```

**Generic Parameters**:

- **`S: ReadStorage`**: Flat storage provider (simpler than `ReadStorageTree`)
  - Must implement `read_storage(address, key)` to return storage values
  - No Merkle proof generation required
  - Common implementation: `InMemoryStorage` or RPC-backed storage

- **`PS: PreimageSource`**: Same as `run_block()` - provides bytecode and account data

**Parameters**:

1. **`transaction: EncodedTx`**
   - Encoded transaction to simulate
   - Can have incomplete validation fields (nonce, signature, etc.)
   - Only execution-relevant fields matter (to, data, value, gas_limit)
   - Validation step is **skipped** entirely

2. **`block_context: BlockContext`**
   - Block context for simulation (block number, timestamp, etc.)
   - Usually current block or pending block
   - Affects EVM opcodes: TIMESTAMP, NUMBER, BLOCKHASH, etc.

3. **`storage: S`**
   - Flat storage providing current state
   - No Merkle tree required (simpler than block execution)
   - Must include all slots accessed during execution

4. **`preimage_source: PS`**
   - Provides bytecode and account data
   - Same as in `run_block()`

5. **`tracer: &mut impl Tracer<CallSimulationSystem>`**
   - Execution tracer (use `NopTracer` if not needed)
   - Receives opcode-level execution events

**Return Type**: `Result<TxResult, ForwardSubsystemError>`

- **Success**: Returns `TxResult = Result<TxOutput, InvalidTransaction>`
  - `Ok(TxOutput)`: Successful simulation
    - `gas_used`: Total gas consumed
    - `gas_refunded`: Gas refunds (SSTORE, SELFDESTRUCT)
    - `output`: Return data (from RETURN opcode)
    - `logs`: Events emitted (LOG0-LOG4)
    - `status`: true (success) or false (revert)
    - `contract_address`: Some(address) for CREATE/CREATE2, None otherwise
  - `Err(InvalidTransaction)`: Transaction preparation failed
    - Should not happen since validation is skipped

- **Error**: Returns `ForwardSubsystemError` on system errors

**Key Features**:
- **No Validation**: Signature, nonce, balance checks are skipped
- **State Simulation**: Does not persist state changes
- **Gas Estimation**: Can be used to estimate gas by iterating gas limits
- **Debugging**: Full execution trace available via tracer

**Use Cases**:

1. **`eth_call` Implementation**:
   ```rust
   let result = simulate_tx(
       transaction,
       current_block_context,
       rpc_storage_provider,
       rpc_preimage_provider,
       &mut NopTracer,
   )?;
   // Return result.output or result.revert_reason
   ```

2. **`eth_estimateGas` Implementation**:
   ```rust
   // Binary search on gas_limit
   let mut low = 21000;
   let mut high = block_gas_limit;
   while low < high {
       let mid = (low + high) / 2;
       let mut tx = transaction.clone();
       tx.gas_limit = mid;
       let result = simulate_tx(tx, context, storage, preimage, &mut NopTracer)?;
       if result.is_ok() {
           high = mid;
       } else {
           low = mid + 1;
       }
   }
   // Return low as estimated gas
   ```

3. **Debugging Smart Contracts**:
   ```rust
   let mut tracer = DetailedTracer::new();
   let result = simulate_tx(tx, context, storage, preimage, &mut tracer)?;
   // Analyze tracer.opcode_trace for debugging
   ```

**Important Notes**:
- **No State Changes**: Simulation does not affect actual state
- **Insufficient Balance**: If sender lacks balance for value transfer, an **internal error** is returned (not a revert)
- **No Validation**: Signature and nonce are not checked
- **Single Transaction**: Only one transaction per call (no block context)

**Cross-references**: See [DataFlow.md](./DataFlow.md#simulation-flow) for simulation pipeline, [ExecutionEnvironments.md](./ExecutionEnvironments.md) for EVM execution details.

---

## RPC Integration

zkSync OS integrates with Ethereum JSON-RPC standards using the Alloy library for type compatibility.

### Alloy Library Usage

**Purpose**: Ethereum-compatible type definitions for RPC integration

**Key Types**:
- `alloy::primitives::Address` - 20-byte Ethereum addresses
- `alloy::primitives::B256` - 32-byte fixed arrays (hashes, storage keys)
- `alloy::primitives::U256` - 256-bit unsigned integers
- `alloy::consensus::Transaction` - Standard Ethereum transaction types
- `alloy::rpc::types::*` - RPC request/response types

**Usage in zkSync OS**:
```rust
use alloy::primitives::{Address, U256};
use alloy::consensus::Transaction;

// Convert between internal types and Alloy types
let address: B160 = internal_address;
let alloy_address = Address::from_slice(&address.to_be_bytes());

// Parse transactions from RPC
let tx: Transaction = alloy::rpc::types::eth::Transaction::try_into(rpc_tx)?;
```

### Integration with anvil-zksync

**Purpose**: Local development node with zkSync OS integration

**Repository**: [github.com/matter-labs/anvil-zksync](https://github.com/matter-labs/anvil-zksync)

**Setup**:
```bash
# Terminal 1: Build zkSync OS binary
cd zksync_os
./dump_bin.sh --type for-tests

# Terminal 2: Run anvil-zksync with zkOS
cd anvil-zksync
cargo run -- --use-zkos --zkos-bin-path=../zksync-os/zksync_os/for_tests.bin
```

**Default Endpoints**:
- **HTTP**: `http://localhost:8011`
- **WebSocket**: `ws://localhost:8011`
- **Alternative port**: `8012` (if 8011 is busy)

**Standard Ethereum RPC Methods**:
All standard methods are supported via Alloy types:
- `eth_blockNumber`, `eth_getBlockByNumber`
- `eth_sendRawTransaction`, `eth_sendTransaction`
- `eth_call`, `eth_estimateGas`
- `eth_getTransactionReceipt`, `eth_getLogs`
- `eth_getBalance`, `eth_getCode`, `eth_getStorageAt`
- `eth_chainId`, `eth_gasPrice`

### Custom RPC Methods

#### zkos_getWitness

**Purpose**: Retrieve witness data for a specific block (for proof generation)

**Method**: `zkos_getWitness`

**Parameters**:
```json
["0x1"]  // Block number as hex string
```

**Request Example**:
```bash
http POST http://127.0.0.1:8011 \
    Content-Type:application/json \
    id:=1 \
    jsonrpc="2.0" \
    method="zkos_getWitness" \
    params:='["0x1"]'
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "witness": "0x0000001a0000000500000001...", // Hex-encoded witness data
    "witness_size": 1234567, // Number of u32 words
    "block_number": "0x1",
    "block_hash": "0xabc..."
  }
}
```

**Witness Format**:
- Hex-encoded `Vec<u32>` from `run_block_generate_witness()`
- Can be passed directly to zkSNARK prover
- Size varies based on block complexity (typically MB-scale)

**Use Cases**:
- External prover nodes fetching witness data
- Testing proof generation pipeline
- Debugging RISC-V execution

**Error Responses**:
- `Block not found`: Block number doesn't exist
- `Witness not available`: Block executed in forward mode (no witness)
- `Witness generation failed`: RISC-V execution error

**Implementation Location**: Implemented in anvil-zksync, not in zkSync OS core

---

## Internal APIs

These APIs are lower-level interfaces intended for advanced users, core developers, and system testing.

### ForwardSystem API

**Location**: `/forward_system/src/run/mod.rs`

**Purpose**: Native execution system with direct database/network access

#### run_block()

Already documented in [Primary Rust API](#run_block) section.

#### generate_proof_input()

**Location**: `/forward_system/src/run/mod.rs:98`

**Signature**:
```rust
pub fn generate_proof_input<T, PS, TS>(
    zk_os_program_path: PathBuf,
    block_context: BlockContext,
    proof_data: ProofData<StorageCommitment>,
    da_commitment_scheme: DACommitmentScheme,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
) -> Result<Vec<u32>, ForwardSubsystemError>
where
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
```

**Purpose**: Generate proof input (witness) for a block with additional proof metadata

**Differences from `run_block_generate_witness()`**:
- Includes callable oracles (arithmetic, KZG commitments) in oracle setup
- Includes UART print responder for debugging output
- Supports proof data for batch proving
- More flexible generic parameters (not restricted to in-memory types)

**Return Type**: `Result<Vec<u32>, ForwardSubsystemError>`
- Success: Witness data as Vec<u32>
- Error: Subsystem error if RISC-V execution fails

#### generate_batch_proof_input()

**Location**: `/forward_system/src/run/mod.rs:153`

**Signature**:
```rust
pub fn generate_batch_proof_input(
    mut blocks_proof_inputs: Vec<&[u32]>,
    da_commitment_scheme: DACommitmentScheme,
    blocks_pubdata: Vec<&[u8]>,
) -> Vec<u32>
```

**Purpose**: Combine multiple block witnesses into a single batch proof input

**Parameters**:
- `blocks_proof_inputs`: Witness data for each block in the batch
- `da_commitment_scheme`: Must match scheme used for individual blocks
- `blocks_pubdata`: Public data (calldata) for each block

**Return Type**: Combined witness for batch proving

**Batch Format**:
```
[num_blocks, block1_witness..., block2_witness..., ..., blobs_advice...]
```

**Use Case**: Multi-block batch proving (reduces L1 verification costs)

#### make_oracle_for_proofs()

**Location**: `/forward_system/src/run/mod.rs:210`

**Purpose**: Create an oracle with all processors configured for proof generation

**Includes**:
- BlockMetadataResponder
- TxDataResponder
- ReadTreeResponder (Merkle proofs)
- GenericPreimageResponder (bytecode, accounts)
- ZKProofDataResponder (previous proof data)
- DACommitmentSchemeResponder (DA scheme)
- ArithmeticQuery (field arithmetic oracle)
- BlobCommitmentAndProofQuery (KZG commitments)
- UARTPrintResponder (optional debug output)

**Use Case**: Custom RISC-V execution with full oracle support

---

### ProofRunningSystem API

**Location**: `/proof_running_system/src/system/bootloader.rs`

**Purpose**: RISC-V bootloader initialization and oracle interface

#### run_proving()

**Location**: `/proof_running_system/src/system/bootloader.rs:149`

**Signature**:
```rust
pub fn run_proving<I, L>(
    heap_start: *mut usize,
    heap_end: *mut usize,
) -> [u32; 8]
where
    I: NonDeterminismCSRSourceImplementation,
    L: Logger + Default,
```

**Generic Parameters**:
- **`I: NonDeterminismCSRSourceImplementation`**: CSR-based oracle implementation
  - Provides `read()` method for CSR reads
  - Implements query protocol for oracle communication
- **`L: Logger + Default`**: Logger for RISC-V execution
  - `UARTLogger` for UART output
  - `NopLogger` for no logging

**Parameters**:
- `heap_start`: Pointer to start of heap memory
- `heap_end`: Pointer to end of heap memory

**Return Type**: `[u32; 8]` - 256-bit output placed in registers x10-x17

**Execution Flow**:
1. Initialize Talc allocator with heap boundaries
2. Create CSR-based oracle proxy
3. Call `run_proving_inner()` to start bootloader
4. Execute all transactions via EVM interpreter
5. Return final output in registers

**Use Case**: Direct RISC-V execution entry point (called from `zksync_os::main::_start_rust()`)

**Important**: This is a bare-metal function - no OS, no stdlib, `no_std` environment

---

### Runner API

**Location**: `/zksync_os_runner/src/lib.rs`

**Purpose**: RISC-V simulator integration for witness generation

#### run()

**Location**: `/zksync_os_runner/src/lib.rs:48`

**Signature**:
```rust
pub fn run(
    img_path: PathBuf,
    diagnostics: Option<DiagnosticsConfig>,
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>,
) -> [u32; 8]
```

**Parameters**:

1. **`img_path: PathBuf`**
   - Path to zkSync OS RISC-V binary
   - Example: `PathBuf::from("zksync_os/for_tests.bin")`
   - Binary must be in raw format (not ELF)

2. **`diagnostics: Option<DiagnosticsConfig>`**
   - Optional profiling and diagnostics configuration
   - `Some(config)` enables flamegraph generation and cycle counting
   - `None` for fastest execution (no profiling overhead)
   - DiagnosticsConfig includes:
     - Symbol table path for profiling
     - Profiler configuration (frequency, reverse graph)

3. **`cycles: usize`**
   - Maximum number of RISC-V cycles allowed
   - Execution aborts if this limit is exceeded
   - Typical values:
     - `1 << 25` (33M cycles) for small blocks
     - `1 << 36` (68B cycles) for large blocks
   - Set conservatively to prevent infinite loops

4. **`non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>`**
   - Oracle implementation providing non-deterministic inputs
   - Common types:
     - `ReadWitnessSource` - Records witness during execution
     - `ZkEENonDeterminismSource` - Standard oracle with processors
     - `QuasiUARTSource` - Simple oracle for testing

**Return Type**: `[u32; 8]` - Program output from registers x10-x17

**Execution Flow**:
1. Load binary from `img_path`
2. Initialize RISC-V simulator with configuration
3. Execute program with oracle delegation
4. Monitor CSR reads/writes for oracle communication
5. Extract output from registers x10-x17
6. Print opcode statistics (if enabled)

**Key Features**:
- **Cycle Counting**: Tracks exact RISC-V cycle count
- **Profiling**: Flamegraph generation for performance analysis
- **Diagnostics**: Opcode statistics and execution trace
- **Witness Recording**: Captures all oracle interactions

**Use Cases**:
- Witness generation for proof systems
- Performance profiling of zkSync OS
- Debugging RISC-V execution
- Testing without actual prover

**Performance Notes**:
- Simulator overhead: ~100-1000x slower than native
- Profiling overhead: additional 10-50% slowdown
- Witness recording: minimal overhead (~5%)

#### run_and_get_effective_cycles()

**Location**: `/zksync_os_runner/src/lib.rs:57`

**Signature**:
```rust
pub fn run_and_get_effective_cycles(
    img_path: PathBuf,
    diagnostics: Option<DiagnosticsConfig>,
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>,
) -> ([u32; 8], Option<u64>)
```

**Purpose**: Same as `run()` but also returns effective cycle count (for benchmarking)

**Return Type**: `([u32; 8], Option<u64>)`
- First element: Program output
- Second element: Effective cycle count (if cycle_marker feature enabled)

**Use Case**: Performance benchmarking and optimization

---

## Type Reference

This section provides quick reference for key API types. See [KeyTypes.md](./KeyTypes.md) for complete documentation.

### BlockContext

**Type Alias**: `pub use BlockMetadataFromOracle as BlockContext`

**Location**: `/zk_ee/src/system/metadata/zk_metadata.rs:108-127`

**Purpose**: Block-level metadata for transaction execution

**Key Fields**:
```rust
pub struct BlockContext {
    pub chain_id: u64,              // Chain ID (e.g., 324 for zkSync Era)
    pub block_number: u64,          // Current block number
    pub block_hashes: BlockHashes,  // Previous 256 block hashes
    pub timestamp: u64,             // Block timestamp (Unix time)
    pub eip1559_basefee: U256,      // EIP-1559 base fee per gas
    pub pubdata_price: U256,        // Cost per byte of pubdata
    pub native_price: U256,         // Native token price
    pub coinbase: B160,             // Block beneficiary address
    pub gas_limit: u64,             // Block gas limit
    pub pubdata_limit: u64,         // Block pubdata limit
    pub mix_hash: U256,             // Prevrandao / difficulty
}
```

**Used By**: All API functions (`run_block()`, `simulate_tx()`, etc.)

**EVM Opcodes**: CHAINID, NUMBER, TIMESTAMP, BASEFEE, COINBASE, GASLIMIT, DIFFICULTY, BLOCKHASH

**Cross-reference**: [KeyTypes.md#blockmetadatafromoracle](./KeyTypes.md#blockmetadatafromoracle)

---

### BlockOutput

**Location**: `/zksync_os_interface/src/types.rs` (interface), `/forward_system/src/run/output.rs:24`

**Purpose**: Complete output from block execution

**Structure**:
```rust
pub struct BlockOutput {
    // Block header with metadata
    pub header: BlockHeader,

    // Per-transaction results
    pub tx_results: Vec<TxResult>,

    // Aggregate storage writes
    pub storage_writes: Vec<StorageWrite>,

    // Account changes (balance, nonce, bytecode)
    pub account_diffs: Vec<AccountDiff>,

    // Published preimages (bytecode, account data)
    pub published_preimages: Vec<(B256, Vec<u8>)>,

    // Raw pubdata for L1 submission
    pub pubdata: Vec<u8>,

    // Total computational native cycles used
    pub computaional_native_used: u64,
}
```

**Components**:

1. **BlockHeader**:
   ```rust
   pub struct BlockHeader {
       pub block_number: u64,
       pub timestamp: u64,
       pub gas_limit: u64,
       pub gas_used: u64,
       pub pubdata_used: u64,
       // ... other fields
   }
   ```

2. **TxResult**: See [TxResult](#txresult) below

3. **StorageWrite**:
   ```rust
   pub struct StorageWrite {
       pub key: B256,              // Flat storage key (Blake2s hash)
       pub value: B256,            // New value
       pub account: B160,          // Contract address
       pub account_key: B256,      // Original storage key
   }
   ```

4. **AccountDiff**:
   ```rust
   pub struct AccountDiff {
       pub address: Address,       // Account address
       pub nonce: u64,             // New nonce
       pub balance: U256,          // New balance
       pub bytecode_hash: B256,    // Bytecode hash (or zero)
   }
   ```

**Use Cases**:
- Sequencer: Build blocks for L1 submission
- RPC: Generate transaction receipts
- State sync: Apply state changes to database
- Proofs: Validate execution correctness

**Cross-reference**: [KeyTypes.md](./KeyTypes.md) for detailed type documentation

---

### TxResult

**Type Alias**: `pub type TxResult = Result<TxOutput, InvalidTransaction>`

**Location**: `/forward_system/src/run/output.rs:27`

**Purpose**: Result of a single transaction execution

**Success Case** (`Ok(TxOutput)`):
```rust
pub struct TxOutput {
    // Gas accounting
    pub gas_used: u64,
    pub gas_refunded: u64,
    pub native_used: u64,
    pub computational_native_used: u64,
    pub pubdata_used: u64,

    // Contract deployment address (CREATE/CREATE2)
    pub contract_address: Option<Address>,

    // Events emitted (LOG0-LOG4)
    pub logs: Vec<Log>,

    // L2→L1 messages
    pub l2_to_l1_logs: Vec<L2ToL1Log>,

    // Per-transaction storage writes (empty in block context)
    pub storage_writes: Vec<StorageWrite>,

    // Execution result
    pub execution_result: ExecutionResult,
}
```

**ExecutionResult**:
```rust
pub enum ExecutionResult {
    Success(ExecutionOutput),  // Transaction succeeded
    Revert(Vec<u8>),          // Transaction reverted with reason
}

pub enum ExecutionOutput {
    Call(Vec<u8>),                    // CALL/STATICCALL return data
    Create(Vec<u8>, Address),         // CREATE/CREATE2 return data + address
}
```

**Error Case** (`Err(InvalidTransaction)`):
```rust
pub enum InvalidTransaction {
    TxDecodeFailed,              // Invalid RLP encoding
    EmptyTxData,                 // No transaction data
    BootloaderError(String),     // Bootloader validation error
    // ... other validation errors
}
```

**Status Determination**:
- `Ok(TxOutput { execution_result: Success(_), ... })` → Transaction succeeded
- `Ok(TxOutput { execution_result: Revert(_), ... })` → Transaction reverted
- `Err(InvalidTransaction)` → Transaction validation failed

**Gas Refunds**:
- SSTORE refunds (clearing storage)
- SELFDESTRUCT refunds
- Max refund: gas_used / 5 (EIP-3529)

**Cross-reference**: [KeyTypes.md](./KeyTypes.md) for all type definitions

---

### ForwardSubsystemError

**Location**: `/forward_system/src/run/errors.rs:3`

**Purpose**: Error type for forward execution system

**Definition**:
```rust
zk_ee::define_subsystem!(Forward,
    cascade WrappedError {
        Bootloader(BootloaderSubsystemError),
    }
);
```

**Error Hierarchy**:
- `ForwardSubsystemError` (top level)
  - `Bootloader(BootloaderSubsystemError)` - Bootloader errors
    - Transaction validation failures
    - Invalid transaction format
    - System contract errors

**Common Error Scenarios**:

1. **Invalid Transaction Format**:
   - Malformed RLP encoding
   - Invalid signature
   - Missing required fields

2. **Bootloader Errors**:
   - Transaction validation failed
   - Insufficient base gas
   - Invalid nonce increment

3. **System Errors**:
   - I/O subsystem failures
   - Internal consistency errors
   - Oracle communication failures

**Error Handling**:
```rust
match run_block(...) {
    Ok(output) => {
        // Process successful block execution
        for tx_result in output.tx_results {
            match tx_result {
                Ok(tx_output) => {
                    // Individual transaction succeeded or reverted
                    match tx_output.execution_result {
                        Success(_) => println!("TX succeeded"),
                        Revert(_) => println!("TX reverted"),
                    }
                }
                Err(invalid_tx) => {
                    // Transaction validation failed
                    eprintln!("Invalid transaction: {:?}", invalid_tx);
                }
            }
        }
    }
    Err(subsystem_error) => {
        // Fatal system error - entire block failed
        eprintln!("Block execution failed: {:?}", subsystem_error);
    }
}
```

**Important**: Individual transaction failures are NOT returned as `ForwardSubsystemError`. They appear as `Err(InvalidTransaction)` within `BlockOutput.tx_results`.

---

## Usage Patterns

This section demonstrates common usage patterns for zkSync OS APIs.

### Sequencer Pattern

**Use Case**: Produce blocks quickly for network consensus

**Code**:
```rust
use zksync_os_api::{run_block, BlockContext};
use forward_system::run::test_impl::{InMemoryTree, InMemoryPreimageSource};

// Setup: Load state from database
let block_context = BlockContext {
    chain_id: 324,
    block_number: current_block + 1,
    timestamp: SystemTime::now().timestamp(),
    gas_limit: 30_000_000,
    // ... other fields
};

let tree = load_merkle_tree_from_db();
let preimage_source = load_preimages_from_db();
let tx_source = DatabaseTxSource::new(pending_txs);

// Callback to stream results to database
struct DbResultCallback { db: Database }
impl TxResultCallback for DbResultCallback {
    fn on_tx_result(&mut self, tx_index: usize, result: &TxResult) {
        // Write result to database immediately
        self.db.store_tx_result(tx_index, result);
    }
}

let callback = DbResultCallback { db: sequencer_db };
let mut tracer = NopTracer; // No tracing for production

// Execute block
let block_output = run_block(
    block_context,
    tree,
    preimage_source,
    tx_source,
    callback,
    &mut tracer,
)?;

// Commit block to L1
submit_block_to_l1(block_output.header, block_output.pubdata);

// Update local state
update_state_from_block_output(block_output);
```

**Key Points**:
- Use `TxResultCallback` to stream results (avoid memory buildup)
- Use `NopTracer` for maximum performance
- Load state from database incrementally
- Commit results to database as soon as possible

---

### Prover Pattern

**Use Case**: Generate ZK proofs for L1 verification

**Code**:
```rust
use zksync_os_api::run_block_generate_witness;
use forward_system::run::test_impl::{InMemoryTree, InMemoryPreimageSource};
use zksync_os_interface::traits::TxListSource;

// Setup: Load complete state (no streaming - all data upfront)
let block_context = fetch_block_context(block_number);
let tree = load_complete_merkle_tree(block_number - 1);
let preimage_source = load_all_preimages(block_number);
let transactions = fetch_all_block_transactions(block_number);
let tx_source = TxListSource { transactions };

// Previous block proof for continuity
let proof_data = fetch_previous_block_proof(block_number - 1);
let da_commitment_scheme = DACommitmentScheme::BlobsZKsyncOS;

// Generate witness
let witness = run_block_generate_witness(
    block_context,
    tree,
    preimage_source,
    tx_source,
    proof_data,
    da_commitment_scheme,
    "zksync_os/singleblock_batch.bin",
)?;

println!("Witness size: {} u32 words ({} MB)",
    witness.len(),
    witness.len() * 4 / 1_000_000
);

// Submit witness to prover cluster
submit_witness_to_prover(block_number, witness);

// Wait for proof generation
let proof = wait_for_proof(block_number, timeout)?;

// Submit proof to L1
submit_proof_to_l1(block_number, proof);
```

**Key Points**:
- All data must be in memory upfront (no lazy loading)
- Witness generation is slow (~minutes for complex blocks)
- Witness size can be large (100s of MB)
- Use appropriate binary (`singleblock_batch.bin` vs `for_tests.bin`)
- Proof generation happens externally (not in zkSync OS)

---

### Simulation Pattern

**Use Case**: Implement `eth_call` and `eth_estimateGas` RPC methods

**Code Example 1: eth_call**:
```rust
use forward_system::run::simulate_tx;
use zksync_os_interface::traits::EncodedTx;

// RPC request: eth_call
async fn handle_eth_call(
    to: Address,
    data: Vec<u8>,
    from: Option<Address>,
    value: Option<U256>,
    gas: Option<u64>,
) -> Result<Vec<u8>, RpcError> {
    // Build transaction (validation fields can be dummy)
    let transaction = EncodedTx {
        to: Some(to),
        data,
        from: from.unwrap_or_default(),
        value: value.unwrap_or_default(),
        gas_limit: gas.unwrap_or(30_000_000),
        // Signature not checked (validation skipped)
        nonce: 0,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        signature: vec![],
        // ... other fields
    };

    // Get current state
    let block_context = get_latest_block_context();
    let storage = RpcStorageProvider::new(rpc_client);
    let preimage_source = RpcPreimageProvider::new(rpc_client);

    // Simulate
    let result = simulate_tx(
        transaction,
        block_context,
        storage,
        preimage_source,
        &mut NopTracer,
    )?;

    // Extract return data
    match result {
        Ok(tx_output) => {
            match tx_output.execution_result {
                ExecutionResult::Success(ExecutionOutput::Call(data)) => Ok(data),
                ExecutionResult::Success(ExecutionOutput::Create(data, addr)) => Ok(data),
                ExecutionResult::Revert(reason) => Err(RpcError::Revert(reason)),
            }
        }
        Err(invalid_tx) => Err(RpcError::InvalidTransaction(invalid_tx)),
    }
}
```

**Code Example 2: eth_estimateGas**:
```rust
async fn handle_eth_estimate_gas(
    to: Address,
    data: Vec<u8>,
    from: Option<Address>,
    value: Option<U256>,
) -> Result<u64, RpcError> {
    let block_context = get_latest_block_context();
    let storage = RpcStorageProvider::new(rpc_client);
    let preimage_source = RpcPreimageProvider::new(rpc_client);

    // Binary search on gas limit
    let mut low = 21000u64; // Base transaction cost
    let mut high = block_context.gas_limit;

    while low < high {
        let mid = (low + high) / 2;

        let transaction = EncodedTx {
            to: Some(to),
            data: data.clone(),
            from: from.unwrap_or_default(),
            value: value.unwrap_or_default(),
            gas_limit: mid,
            // ... other fields
        };

        let result = simulate_tx(
            transaction,
            block_context.clone(),
            storage.clone(),
            preimage_source.clone(),
            &mut NopTracer,
        )?;

        match result {
            Ok(tx_output) => {
                match tx_output.execution_result {
                    ExecutionResult::Success(_) => {
                        // Transaction succeeded with this gas
                        high = mid;
                    }
                    ExecutionResult::Revert(_) => {
                        // Transaction reverted (OOG or logical revert)
                        // Check if it's OOG
                        if tx_output.gas_used >= mid - 1000 {
                            low = mid + 1; // Likely OOG
                        } else {
                            return Err(RpcError::ExecutionReverted); // Logical revert
                        }
                    }
                }
            }
            Err(invalid_tx) => return Err(RpcError::InvalidTransaction(invalid_tx)),
        }
    }

    // Add buffer (10%) for safety
    Ok((low as f64 * 1.1) as u64)
}
```

**Key Points**:
- Validation is skipped (dummy signature/nonce OK)
- Use RPC-backed storage providers for remote state
- Binary search for gas estimation (10-20 iterations typical)
- Add safety buffer to gas estimates
- Handle both success and revert cases

---

## Error Handling

### Error Categories

zkSync OS errors fall into three categories:

1. **Subsystem Errors** (`ForwardSubsystemError`)
   - Fatal system failures
   - Entire block execution fails
   - Rare in production

2. **Transaction Validation Errors** (`InvalidTransaction`)
   - Transaction rejected before execution
   - Per-transaction failures
   - Included in `BlockOutput.tx_results` as `Err(InvalidTransaction)`

3. **Execution Errors** (EVM reverts)
   - Transaction executed but reverted
   - Included in `BlockOutput.tx_results` as `Ok(TxOutput { execution_result: Revert(...) })`

### Common Error Scenarios

#### 1. Invalid Transaction Format

**Cause**: Malformed transaction encoding

**Detection**: During transaction parsing in bootloader

**Result**: `Err(InvalidTransaction::TxDecodeFailed)`

**Handling**:
```rust
match block_output.tx_results[i] {
    Err(InvalidTransaction::TxDecodeFailed) => {
        log::warn!("Transaction {} has invalid encoding", i);
        // Skip transaction, continue with next
    }
    // ... other cases
}
```

**Recovery**: Skip transaction, continue block execution

---

#### 2. Out of Gas

**Cause**: Transaction runs out of gas during execution

**Detection**: Resources exhausted during opcode execution

**Result**: `Ok(TxOutput { execution_result: Revert(vec![]), gas_used: gas_limit, ... })`

**Handling**:
```rust
if tx_output.gas_used >= tx_output.gas_limit - 1000 {
    log::info!("Transaction ran out of gas");
    // All gas consumed, no refund
}
```

**Recovery**: Transaction reverted, state rolled back, all gas consumed

---

#### 3. State Change in Static Call

**Cause**: SSTORE, LOG, CREATE, etc. in STATICCALL context

**Detection**: During opcode validation in interpreter

**Result**: `Ok(TxOutput { execution_result: Revert(vec![]), ... })`

**Handling**: Same as any revert - transaction failed, state rolled back

---

#### 4. Call Stack Too Deep

**Cause**: Call depth exceeds 1024

**Detection**: Before starting callee frame

**Result**: CALL returns 0 (failure), caller continues

**Handling**: Automatic - EVM semantics handle this gracefully

---

#### 5. Insufficient Balance

**Cause**: Value transfer exceeds sender balance

**For Validated Transactions**: Caught in `before_executing_frame()`, CALL returns 0

**For Simulations** (`simulate_tx()`): Returns `ForwardSubsystemError` (internal error)

**Handling**:
```rust
// Validated transaction (in run_block)
match call_result {
    CallResult::PreparationStepFailed => {
        // Insufficient balance - call failed before execution
        // CALL opcode returns 0, caller continues
    }
    // ... other cases
}

// Simulation (eth_call)
match simulate_tx(...) {
    Err(ForwardSubsystemError::...) => {
        // Could be insufficient balance or other internal error
        return Err(RpcError::InternalError);
    }
    Ok(result) => // Success or revert
}
```

---

#### 6. RISC-V Execution Failure

**Cause**: RISC-V binary crashes or times out

**Detection**: `zksync_os_runner::run()` returns `[0u32; 8]`

**Result**: Panic in `run_block_generate_witness()`

**Handling**:
```rust
let witness = run_block_generate_witness(...)?;
// Assertion: assert_ne!(output, [0u32; 8]);
// Will panic if RISC-V execution failed
```

**Recovery**: No automatic recovery - requires investigation

**Prevention**:
- Ensure all data is available (tree, preimages, transactions)
- Use sufficient cycle limit
- Test with same data in forward mode first

---

### Error Handling Best Practices

1. **Distinguish Error Types**:
   ```rust
   match run_block(...) {
       Ok(output) => {
           for (i, tx_result) in output.tx_results.iter().enumerate() {
               match tx_result {
                   Ok(tx) => match &tx.execution_result {
                       Success(_) => log::info!("TX {i} succeeded"),
                       Revert(reason) => log::info!("TX {i} reverted: {:?}", reason),
                   },
                   Err(invalid) => log::warn!("TX {i} invalid: {:?}", invalid),
               }
           }
       }
       Err(subsystem_err) => log::error!("Block failed: {:?}", subsystem_err),
   }
   ```

2. **Log All Errors**: Track error rates for monitoring

3. **Retry Subsystem Errors**: Temporary failures may succeed on retry

4. **Don't Retry Transaction Errors**: Invalid transactions won't become valid

5. **Handle Reverts Gracefully**: Reverts are normal EVM behavior, not errors

---

## Code Examples

### Example 1: Simple Block Execution

**Scenario**: Execute a block with 3 transactions in forward mode

```rust
use zksync_os_api::{run_block, BlockContext};
use forward_system::run::test_impl::{
    InMemoryTree, InMemoryPreimageSource, NoopTxCallback,
};
use zksync_os_interface::traits::TxListSource;
use zk_ee::system::tracer::NopTracer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup block context
    let block_context = BlockContext {
        chain_id: 324,
        block_number: 100,
        timestamp: 1234567890,
        eip1559_basefee: 1000000000u64.into(), // 1 gwei
        gas_limit: 30_000_000,
        pubdata_limit: 100_000,
        coinbase: [0x42; 20].into(),
        // ... other fields with defaults
        ..Default::default()
    };

    // Setup storage (empty tree for example)
    let tree = InMemoryTree::new();

    // Setup preimages (empty for example)
    let preimage_source = InMemoryPreimageSource::new();

    // Create transactions
    let transactions = vec![
        create_erc20_transfer(alice, bob, 100),
        create_erc20_transfer(bob, charlie, 50),
        create_contract_call(charlie, contract_address, "doSomething()"),
    ];
    let tx_source = TxListSource { transactions };

    // Execute block
    let mut tracer = NopTracer;
    let block_output = run_block(
        block_context,
        tree,
        preimage_source,
        tx_source,
        NoopTxCallback,
        &mut tracer,
    )?;

    // Print results
    println!("Block {} executed successfully", block_output.header.block_number);
    println!("Gas used: {} / {}", block_output.header.gas_used, block_output.header.gas_limit);
    println!("Transactions: {}", block_output.tx_results.len());

    for (i, tx_result) in block_output.tx_results.iter().enumerate() {
        match tx_result {
            Ok(tx_output) => {
                let status = match &tx_output.execution_result {
                    ExecutionResult::Success(_) => "SUCCESS",
                    ExecutionResult::Revert(_) => "REVERT",
                };
                println!("  TX {}: {} - gas: {}", i, status, tx_output.gas_used);
            }
            Err(invalid_tx) => {
                println!("  TX {}: INVALID - {:?}", i, invalid_tx);
            }
        }
    }

    Ok(())
}
```

---

### Example 2: Witness Generation with Profiling

**Scenario**: Generate witness with flamegraph profiling

```rust
use zksync_os_api::run_block_generate_witness;
use forward_system::run::test_impl::{InMemoryTree, InMemoryPreimageSource};
use zksync_os_interface::traits::TxListSource;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load block data
    let block_number = 100;
    let block_context = load_block_context(block_number)?;
    let tree = load_merkle_tree(block_number - 1)?;
    let preimage_source = load_preimages(block_number)?;
    let transactions = load_transactions(block_number)?;
    let tx_source = TxListSource { transactions };

    // Previous block proof
    let proof_data = load_previous_proof(block_number - 1)?;
    let da_commitment_scheme = DACommitmentScheme::BlobsZKsyncOS;

    println!("Starting witness generation for block {}", block_number);
    let start = std::time::Instant::now();

    // Generate witness
    let witness = run_block_generate_witness(
        block_context,
        tree,
        preimage_source,
        tx_source,
        proof_data,
        da_commitment_scheme,
        "zksync_os/for_tests.bin",
    )?;

    let duration = start.elapsed();
    let witness_size_mb = (witness.len() * 4) as f64 / 1_000_000.0;

    println!("Witness generation completed:");
    println!("  Duration: {:.2}s", duration.as_secs_f64());
    println!("  Witness size: {:.2} MB", witness_size_mb);
    println!("  Witness words: {}", witness.len());

    // Save witness to file
    let witness_path = format!("witness_block_{}.bin", block_number);
    save_witness(&witness, &witness_path)?;

    println!("Witness saved to {}", witness_path);

    Ok(())
}

fn save_witness(witness: &[u32], path: &str) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(path)?;
    let bytes: Vec<u8> = witness.iter()
        .flat_map(|&w| w.to_le_bytes())
        .collect();
    file.write_all(&bytes)?;
    Ok(())
}
```

---

### Example 3: Custom Transaction Result Processor

**Scenario**: Stream transaction results to database and update indices

```rust
use forward_system::run::TxResultCallback;
use zksync_os_api::run_block;

// Custom callback that processes results immediately
struct DatabaseResultProcessor {
    db: Database,
    block_number: u64,
    cumulative_gas: u64,
}

impl TxResultCallback for DatabaseResultProcessor {
    fn on_tx_result(&mut self, tx_index: usize, result: &TxResult) {
        match result {
            Ok(tx_output) => {
                // Update cumulative gas
                self.cumulative_gas += tx_output.gas_used;

                // Store transaction result
                self.db.store_transaction_result(
                    self.block_number,
                    tx_index as u64,
                    tx_output,
                );

                // Index events (logs) for efficient querying
                for (log_index, log) in tx_output.logs.iter().enumerate() {
                    self.db.index_log(
                        self.block_number,
                        tx_index as u64,
                        log_index as u64,
                        log,
                    );
                }

                // Update account balances
                for account_diff in &tx_output.account_diffs {
                    self.db.update_account(
                        account_diff.address,
                        account_diff.nonce,
                        account_diff.balance,
                    );
                }

                println!("Processed TX {}: {} gas (total: {})",
                    tx_index, tx_output.gas_used, self.cumulative_gas);
            }
            Err(invalid_tx) => {
                // Store invalid transaction
                self.db.store_invalid_transaction(
                    self.block_number,
                    tx_index as u64,
                    invalid_tx,
                );

                println!("Invalid TX {}: {:?}", tx_index, invalid_tx);
            }
        }
    }
}

fn process_block(block_number: u64) -> Result<(), Box<dyn std::error::Error>> {
    let block_context = load_block_context(block_number)?;
    let tree = load_tree(block_number - 1)?;
    let preimage_source = load_preimages(block_number)?;
    let tx_source = load_transactions(block_number)?;

    let mut processor = DatabaseResultProcessor {
        db: Database::connect()?,
        block_number,
        cumulative_gas: 0,
    };

    let mut tracer = NopTracer;

    let block_output = run_block(
        block_context,
        tree,
        preimage_source,
        tx_source,
        &mut processor, // Results streamed to database during execution
        &mut tracer,
    )?;

    // Finalize block in database
    processor.db.finalize_block(block_number, &block_output.header);

    Ok(())
}
```

---

## Cross-References

**For deeper understanding of API usage in context, see:**

- **[DataFlow.md](./DataFlow.md)**: Complete data flow from API entry to output
  - Forward execution pipeline
  - Proof execution pipeline
  - Oracle query protocol
  - Witness generation flow

- **[Architecture.md](./Architecture.md)**: System architecture and execution modes
  - Forward running vs. proof running
  - Compilation targets
  - Build profiles and feature flags

- **[KeyTypes.md](./KeyTypes.md)**: Detailed documentation of all types
  - BlockContext field descriptions
  - BlockOutput structure
  - TxResult and TxOutput
  - Error types

- **[ExecutionEnvironments.md](./ExecutionEnvironments.md)**: EVM execution details
  - Opcode execution
  - Gas accounting
  - Call handling

- **[SystemLayer.md](./SystemLayer.md)**: System layer operations
  - IO subsystem
  - Resource accounting
  - Metadata access

---

## Summary

zkSync OS provides a comprehensive API surface for block execution and proof generation:

**Primary Rust API** (`zksync_os_api` crate):
- `run_block()` - Fast native execution for sequencers
- `run_block_generate_witness()` - RISC-V execution with witness generation
- `simulate_tx()` - Transaction simulation for RPC

**RPC Integration**:
- Standard Ethereum JSON-RPC via Alloy types
- Custom method: `zkos_getWitness` for proof data
- Integration with anvil-zksync for local development

**Internal APIs**:
- ForwardSystem: `generate_proof_input()`, `make_oracle_for_proofs()`
- ProofRunningSystem: `run_proving()` for RISC-V bootloader
- Runner: `run()` for RISC-V simulation

**Key Design Principles**:
- **Trait-based abstractions**: Storage, preimages, transactions all pluggable
- **Dual execution modes**: Native for speed, RISC-V for proofs
- **Comprehensive error handling**: Distinguish system errors from transaction failures
- **Result streaming**: Callbacks for real-time processing

This API design enables zkSync OS to serve both high-throughput sequencers and deterministic proof generation while maintaining Ethereum compatibility.
