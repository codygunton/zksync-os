# Data Flow in zkSync OS

## Introduction

Understanding data flow is critical to working with zkSync OS. This document traces how data enters the system, transforms through various stages, and exits as block results or zero-knowledge proofs. The zkSync OS architecture is designed around a clear data pipeline that maintains determinism while supporting multiple execution environments.

**Why Data Flow Matters:**
- **Determinism**: The same inputs must always produce the same outputs for proof verification
- **Modularity**: Clean data boundaries enable testing and component replacement
- **Performance**: Understanding bottlenecks requires knowing where data transforms
- **Debugging**: Tracing issues requires following the data path

**Major Data Pathways:**
1. **Block Execution Path**: Input → Oracle → Bootloader → EE → Result
2. **Witness Generation Path**: Input → RISC-V → CSR Oracle → Witness Stream
3. **Storage Access Path**: EE → IOSubsystem → Oracle → Storage Source
4. **Call Execution Path**: EE → Preemption → Bootloader → Callee EE → Resume

---

## Entry Points

zkSync OS has several entry points depending on the execution mode and use case.

### API Layer Entry Points

#### Location: `api/src/lib.rs`

The primary public API provides three main functions:

#### 1. `run_block()` - Direct Block Execution

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
    T: ReadStorageTree + Clone,
    PS: PreimageSource + Clone,
    TS: TxSource,
    TR: TxResultCallback,
```

**Purpose**: Execute a block natively (x86/ARM) for sequencer operation
**Input**: Block metadata, state tree, transaction source
**Output**: Block results with state changes and transaction results
**Use Case**: Production sequencer, development testing

#### 2. `run_block_generate_witness()` - Proof Generation

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

**Purpose**: Execute block in RISC-V simulator and generate witness data for proving
**Input**: Block metadata, in-memory state, transaction list, RISC-V binary path
**Output**: Witness data as Vec<u32> stream
**Use Case**: Proof generation, verification testing

#### 3. `simulate_tx()` - Transaction Simulation

```rust
pub fn simulate_tx<S, PS>(
    // Parameters for single transaction simulation
) -> Result<TxResult, ForwardSubsystemError>
```

**Purpose**: Simulate single transaction without validation (eth_call, eth_estimateGas)
**Input**: Transaction data, current state
**Output**: Execution result with gas used
**Use Case**: RPC methods, gas estimation

### RISC-V Entry Point

#### Location: `zksync_os/src/main.rs`

```rust
#[no_mangle]
pub extern "C" fn _start_rust() {
    // Setup allocator, exception handlers
    unsafe {
        let result = workload();
        // Store result in registers x10-x17 for output
    }
}

fn workload() -> u256 {
    run_proving::<NonDeterminismSource>()
}
```

**Purpose**: Entry point for RISC-V binary execution in proof mode
**Flow**: `_start_rust()` → `workload()` → `run_proving()`
**Environment**: Bare-metal RISC-V, no OS, custom allocator

### Testing Entry Point

#### Location: `zksync_os_runner/src/lib.rs`

```rust
pub fn run(
    binary_path: &str,
    oracle: ZkEENonDeterminismSource<impl IOOracle>,
    config: RunnerConfig,
) -> Vec<u32>
```

**Purpose**: Execute RISC-V binary in simulator for testing/witness generation
**Use Case**: Development, testing, witness generation

---

## Data Input Sources

All external data enters zkSync OS through **query processors** that respond to oracle queries. This abstraction enables the same code to run with different data sources (in-memory, database, RPC).

### Query Processor Architecture

```
┌─────────────────────────────────────────────────┐
│   ZkEENonDeterminismSource (Oracle Container)   │
│                                                 │
│  Query ID Ranges:                               │
│  ├─ 0x1000-0x1FFF → BlockMetadataResponder      │
│  ├─ 0x2000-0x2FFF → TxDataResponder             │
│  ├─ 0x3000-0x3FFF → ReadTreeResponder           │
│  ├─ 0x4000-0x4FFF → ReadStorageResponder        │
│  ├─ 0x5000-0x5FFF → GenericPreimageResponder    │
│  ├─ 0x6000-0x6FFF → ZKProofDataResponder        │
│  ├─ 0x7000-0x7FFF → DACommitmentSchemeResponder │
│  ├─ 0x8000-0x8FFF → ArithmeticQuery             │
│  └─ 0x9000-0x9FFF → BlobKZGCommitmentQuery      │
│                                                 │
│  Process:                                       │
│  1. Query arrives with ID                       │
│  2. Lookup processor by ID range                │
│  3. Route to processor                          │
│  4. Processor returns iterator of u32 results   │
└─────────────────────────────────────────────────┘
```

### 1. BlockMetadataResponder

**Location**: `forward_system/src/system/query_processors/block_metadata.rs`

**Purpose**: Provides block-level context (number, timestamp, gas limits)

**Queries**:
- `BLOCK_METADATA_QUERY_ID`: Returns all block metadata

**Returns**: `BlockMetadataFromOracle` structure containing:
- `chain_id`: Network identifier
- `block_number`: Current block height
- `timestamp`: Block timestamp
- `eip1559_basefee`: Base fee per gas (EIP-1559)
- `coinbase`: Block proposer address
- `gas_limit`: Maximum gas for block
- `pubdata_limit`: Maximum pubdata bytes
- `block_hashes`: Previous 256 block hashes
- `pubdata_price`: Cost per pubdata byte
- `native_price`: Native token price
- `mix_hash`: Prevrandao value

**Code Reference**: `forward_system/src/system/query_processors/block_metadata.rs`

### 2. TxDataResponder

**Location**: `forward_system/src/system/query_processors/tx_data.rs`

**Purpose**: Provides transaction data to bootloader

**Queries** (4 types):
1. `NEXT_TX_SIZE_QUERY_ID`: Returns size of next transaction in bytes
2. `TX_DATA_WORDS_QUERY_ID`: Returns transaction bytes as u32 words
3. `TX_ENCODING_FORMAT_QUERY_ID`: Returns encoding format (RLP or ABI)
4. `TX_FROM_QUERY_ID`: Returns sender address (for RLP encoded txs)

**Data Flow**:
```
TxSource (external) → TxDataResponder → Bootloader
         ↓
  peek_next_tx_size()
  get_next_tx_encoding()
  get_next_tx_data()
         ↓
  Bootloader parses and executes
```

**Code Reference**: `forward_system/src/system/query_processors/tx_data.rs`

### 3. ReadTreeResponder

**Location**: `forward_system/src/system/query_processors/read_tree.rs`

**Purpose**: Provides storage values with Merkle proofs for verification

**Queries**:
- `STORAGE_TREE_QUERY`: Read storage value with proof
- `MERKLE_PROOF_QUERY`: Get Merkle proof for key

**Returns**: Storage value + Merkle proof nodes

**Use Case**: Proof generation mode where proofs must be verified

**Code Reference**: `forward_system/src/system/query_processors/read_tree.rs`

### 4. ReadStorageResponder

**Location**: `forward_system/src/system/query_processors/read_storage.rs`

**Purpose**: Provides storage values without proofs (faster)

**Queries**:
- `InitialStorageSlotQuery::QUERY_ID`: Read initial storage value

**Returns**: Storage value only (no Merkle proof)

**Use Case**: Forward execution mode where proofs aren't needed

**Code Reference**: `forward_system/src/system/query_processors/read_storage.rs`

### 5. GenericPreimageResponder

**Location**: `forward_system/src/system/query_processors/generic_preimage.rs`

**Purpose**: Provides preimages (original data) for hashes

**Queries**:
- `PREIMAGE_QUERY_ID`: Returns preimage for a hash

**Common Preimages**:
- **Bytecode**: Contract bytecode from bytecode hash
- **Account Data**: Account properties from account hash

**Returns**: Raw bytes of preimage

**Code Reference**: `forward_system/src/system/query_processors/generic_preimage.rs`

### 6. ZKProofDataResponder

**Location**: `forward_system/src/system/query_processors/zk_proof_data.rs`

**Purpose**: Provides proof-related data (previous block proof, batch proof)

**Queries**:
- `ZK_PROOF_DATA_QUERY_ID`: Returns proof data

**Returns**: `ProofData<StorageCommitment>` structure

**Use Case**: Multi-block batch proving, proof aggregation

**Code Reference**: `forward_system/src/system/query_processors/zk_proof_data.rs`

### 7. DACommitmentSchemeResponder

**Location**: `forward_system/src/system/query_processors/da_commitment_scheme.rs`

**Purpose**: Provides data availability commitment scheme

**Queries**:
- `DA_COMMITMENT_QUERY_ID`: Returns DA scheme identifier

**DA Schemes**:
- `BlobsZKsyncOS`: EIP-4844 blob commitments with KZG
- `Blake2s`: Blake2s hash commitments
- `Keccak256`: Keccak256 hash commitments

**Code Reference**: `forward_system/src/system/query_processors/da_commitment_scheme.rs`

### 8. Callable Oracles

**Location**: `callable_oracles/src/`

**Purpose**: Expensive operations delegated to oracle (too costly for RISC-V)

**Queries**:
- `ArithmeticQuery`: Field arithmetic, modular operations
- `BlobKZGCommitmentQuery`: EIP-4844 blob KZG commitments
- `HashToPrimeQuery`: Cryptographic hash-to-prime

**Use Case**: Cryptographic operations that are too expensive in RISC-V

**Code Reference**: `callable_oracles/src/`

---

## Complete Data Flow Pipeline

### Overview Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    INPUT LAYER                               │
│  BlockContext + Tree + PreimageSource + TxSource            │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│               STAGE 1: INPUT ASSEMBLY                        │
│  • Create ZkEENonDeterminismSource oracle                   │
│  • Register all query processors                            │
│  • Setup query ID routing                                   │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│            STAGE 2: RISC-V EXECUTION                        │
│  • Load RISC-V binary (proof mode only)                    │
│  • Execute: _start_rust() → run_proving()                  │
│  • CSR-based oracle communication                          │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│         STAGE 3: BOOTLOADER INITIALIZATION                  │
│  • Setup allocator and memory                               │
│  • Query block metadata from oracle                         │
│  • Initialize system state                                  │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│          STAGE 4: TRANSACTION EXECUTION                     │
│  For each transaction:                                      │
│  ├─ Query tx data from oracle                              │
│  ├─ Parse and validate transaction                         │
│  ├─ Execute in EVM (or other EE)                           │
│  │  ├─ Opcode loop                                         │
│  │  ├─ External calls (preemption)                         │
│  │  ├─ Storage access (oracle queries)                     │
│  │  └─ Event emission                                      │
│  └─ Collect results                                        │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│           STAGE 5: RESULT COLLECTION                        │
│  • ForwardRunningResultKeeper accumulates:                  │
│    - Transaction results                                    │
│    - Storage writes                                         │
│    - Events and logs                                        │
│    - Account diffs                                          │
│    - Preimages                                              │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│          STAGE 6: OUTPUT FORMATTING                         │
│  Convert to BlockOutput:                                    │
│  ├─ Block header                                            │
│  ├─ Transaction results (Vec<TxResult>)                    │
│  ├─ Storage writes (Vec<StorageWrite>)                     │
│  ├─ Account diffs                                           │
│  ├─ Events and logs                                         │
│  ├─ Published preimages                                     │
│  └─ Witness data (proof mode)                              │
└─────────────────────────────────────────────────────────────┘
                      │
                      ▼
              [OUTPUT: BlockOutput or Vec<u32> witness]
```

### Stage 1: Input Assembly

**Location**: `forward_system/src/run/mod.rs` (lines 65-150)

**Process**:

1. **Create Oracle Container**:
```rust
let mut oracle = ZkEENonDeterminismSource::new();
```

2. **Register Query Processors**:
```rust
// Block metadata
oracle.add_external_processor(BlockMetadataResponder::new(block_context));

// Transaction data
oracle.add_external_processor(TxDataResponder::new(tx_source));

// Storage access
oracle.add_external_processor(ReadTreeResponder::new(tree.clone()));
oracle.add_external_processor(ReadStorageResponder::new(tree));

// Preimages
oracle.add_external_processor(GenericPreimageResponder::new(preimage_source));

// Proof data
oracle.add_external_processor(ZKProofDataResponder::new(proof_data));

// DA commitment
oracle.add_external_processor(DACommitmentSchemeResponder::new(da_scheme));

// Callable oracles
oracle.add_external_processor(ArithmeticQuery);
oracle.add_external_processor(BlobKZGCommitmentQuery);
```

3. **Build Query ID Routing Table**:
   - Oracle maintains `BTreeMap<QueryID, ProcessorIndex>`
   - Each processor reserves a range of query IDs
   - Queries are routed by ID to correct processor

**Output**: Configured oracle ready for queries

### Stage 2: RISC-V Execution

**Location**: `zksync_os_runner/src/lib.rs` (proof mode only)

**Process**:

1. **Load RISC-V Binary**:
```rust
let binary = std::fs::read(binary_path)?;
```

2. **Initialize Simulator**:
```rust
let config = risc_v_simulator::RunConfig {
    entry_point: 0,
    max_cycles: 1 << 36,
    // ...
};
```

3. **Execute with Oracle Delegation**:
```rust
risc_v_simulator::runner::run_simple_with_delegated_functions(
    &binary,
    config,
    oracle_delegation_callbacks,
)
```

4. **CSR Communication Protocol**:

```
RISC-V Code                        Oracle (Host)
     |                                  |
     |  Write query_id to CSR          |
     |--------------------------------->|
     |  Write query params to CSR      |
     |--------------------------------->|
     |                                  |
     |                                  | Buffer query
     |                                  | Lookup processor
     |                                  | Execute query
     |                                  |
     |  Read result length from CSR    |
     |<---------------------------------|
     |  Read result words (u32s)       |
     |<---------------------------------|
     |  Read result words (u32s)       |
     |<---------------------------------|
     |  ...repeat until complete       |
     |<---------------------------------|
```

**Key Points**:
- CSR reads trigger oracle queries
- Results streamed as u32 chunks
- All communication recorded for witness (if using `ReadWitnessSource`)

### Stage 3: Bootloader Initialization

**Location**: `basic_bootloader/src/bootloader/mod.rs`

**Process**:

1. **Setup Memory and Allocator**:
```rust
// Initialize heap with boundaries
let allocator = setup_allocator(heap_start, heap_end);

// Setup stack pointer
setup_stack(stack_start);
```

2. **Query Block Metadata**:
```rust
let block_metadata = system.query_oracle::<BlockMetadataFromOracle>();
```

3. **Initialize System State**:
```rust
let mut system = System::new(
    io_subsystem,
    metadata,
    allocator,
);
```

4. **Load ROM Data** (RISC-V):
   - Copy data sections to RAM
   - Initialize constants

**Output**: Ready-to-execute system state

### Stage 4: Transaction Execution

**Location**:
- Bootloader: `basic_bootloader/src/bootloader/mod.rs`
- EVM: `evm_interpreter/src/lib.rs`

**Transaction Loop**:

```rust
loop {
    // 1. Get next transaction size
    let tx_size = query_oracle(NEXT_TX_SIZE_QUERY_ID);
    if tx_size == 0 { break; } // No more transactions

    // 2. Get transaction encoding format
    let encoding = query_oracle(TX_ENCODING_FORMAT_QUERY_ID);

    // 3. Get transaction data
    let tx_data = query_oracle(TX_DATA_WORDS_QUERY_ID, tx_size);

    // 4. Get sender (if RLP)
    let sender = if encoding == RLP {
        query_oracle(TX_FROM_QUERY_ID)
    } else {
        decode_from_tx_data(&tx_data)
    };

    // 5. Parse transaction
    let tx = parse_transaction(&tx_data, encoding);

    // 6. Validate (forward mode only)
    if forward_mode {
        validate_signature(&tx, sender)?;
        validate_nonce(&tx, sender)?;
        check_balance_for_fee(&tx, sender)?;
    }

    // 7. Create call request
    let call_request = ExternalCallRequest {
        caller: sender,
        callee: tx.to,
        calldata: tx.data,
        call_value: tx.value,
        gas_limit: tx.gas_limit,
        // ...
    };

    // 8. Execute in EE
    let result = execute_in_ee(call_request, &mut system);

    // 9. Handle result
    match result {
        CallResult::Successful { returndata } => {
            commit_changes();
            emit_receipt(success, gas_used, logs);
        }
        CallResult::Failed { returndata } => {
            revert_changes();
            emit_receipt(failure, gas_used, []);
        }
    }

    // 10. Callback to result keeper
    tx_result_callback.on_tx_result(tx_result);
}
```

**EVM Execution Detail**:

```
Interpreter::start_executing_frame()
     |
     v
┌─────────────────────────────────────┐
│     OPCODE EXECUTION LOOP           │
│                                     │
│  while !done {                      │
│    opcode = bytecode[ip]            │
│    ip += 1                          │
│                                     │
│    match opcode {                   │
│      PUSH => stack.push(val)        │
│      POP => stack.pop()             │
│      ADD => a = pop(), b = pop(),   │
│             push(a + b)             │
│      MUL => a = pop(), b = pop(),   │
│             push(a * b)             │
│      SLOAD => key = pop(),          │
│               val = storage_read(), │
│               push(val)             │
│      SSTORE => key = pop(),         │
│                val = pop(),         │
│                storage_write(k, v)  │
│      CALL => preempt_for_call()     │
│      RETURN => return success       │
│      REVERT => return failure       │
│      // ... 100+ opcodes            │
│    }                                │
│                                     │
│    gas.charge(opcode_cost)          │
│    if gas.is_empty() { OOG error }  │
│  }                                  │
└─────────────────────────────────────┘
```

**Preemption for External Calls**:

When CALL/DELEGATECALL/STATICCALL/CREATE is encountered:

```rust
// 1. EE prepares call request
let external_call = ExternalCallRequest {
    callee: address,
    calldata: input_data,
    call_value: value,
    available_resources: calculate_gas_to_pass(),
    modifier: CallModifier::Normal, // or Delegate/Static
    // ...
};

// 2. EE returns preemption point
return ExecutionEnvironmentPreemptionPoint::CallRequest(external_call);

// 3. Bootloader handles call
let call_result = orchestrate_call(external_call);

// 4. Bootloader resumes EE with result
let next_preemption = ee.continue_after_preemption(call_result);
```

**Storage Access**:

```
SLOAD opcode
     |
     v
IOSubsystem::storage_read()
     |
     v
Query Oracle:
  - InitialStorageSlotQuery (forward mode)
  - STORAGE_TREE_QUERY (proof mode with Merkle proof)
     |
     v
ReadStorageResponder / ReadTreeResponder
     |
     v
Look up in storage tree
     |
     v
Return value to EE
     |
     v
Push value onto EVM stack
```

### Stage 5: Result Collection

**Location**: `forward_system/src/system/result_keeper.rs`

**ForwardRunningResultKeeper** accumulates:

```rust
pub struct ForwardRunningResultKeeper<TR> {
    // Transaction results
    pub tx_results: Vec<TxResult>,

    // Storage changes
    pub storage_writes: HashMap<WarmStorageKey, WarmStorageValue>,

    // Account changes
    pub account_diffs: HashMap<Address, AccountDiff>,

    // Events
    pub events: Vec<Event>,

    // Logs
    pub logs: Vec<L2ToL1Log>,

    // Preimages
    pub preimages: HashMap<Hash, Vec<u8>>,

    // Transaction result callback
    tx_callback: TR,
}
```

**Accumulation Process**:

1. **Transaction Result**:
```rust
impl ResultKeeperExt for ForwardRunningResultKeeper {
    fn handle_tx_result(&mut self, result: TxResult) {
        self.tx_results.push(result.clone());
        self.tx_callback.on_tx_result(result);
    }
}
```

2. **Storage Write**:
```rust
impl IOResultKeeper for ForwardRunningResultKeeper {
    fn on_storage_write(&mut self, key: WarmStorageKey, value: WarmStorageValue) {
        self.storage_writes.insert(key, value);
    }
}
```

3. **Event Emission**:
```rust
impl IOResultKeeper for ForwardRunningResultKeeper {
    fn on_event(&mut self, event: Event) {
        self.events.push(event);
    }
}
```

### Stage 6: Output Formatting

**Location**: `forward_system/src/run/output.rs`

**Convert ResultKeeper to BlockOutput**:

```rust
pub struct BlockOutput {
    pub header: BlockHeader,
    pub tx_results: Vec<TxResult>,
    pub storage_writes: Vec<StorageWrite>,
    pub account_diffs: Vec<AccountDiff>,
    pub published_preimages: Vec<(Hash, Vec<u8>)>,
    pub pubdata: Vec<u8>,
    pub computational_native_used: u64,
}
```

**Conversion Process**:

1. **Block Header**:
```rust
let header = BlockHeader {
    number: block_metadata.block_number,
    timestamp: block_metadata.timestamp,
    hash: compute_block_hash(),
    parent_hash: block_metadata.block_hashes.get(0),
    state_root: compute_state_root(&storage_writes),
    // ...
};
```

2. **Transaction Results**:
```rust
pub struct TxResult {
    pub execution_result: ExecutionResult,
    pub gas_used: u64,
    pub gas_refunded: u64,
    pub computational_native_used: u64,
    pub pubdata_used: u64,
    pub logs: Vec<Log>,
    pub l2_to_l1_logs: Vec<L2ToL1LogWithPreimage>,
    pub contract_address: Option<Address>, // For CREATE
}

pub enum ExecutionResult {
    Success(ExecutionOutput),
    Revert { output: Vec<u8> },
}

pub enum ExecutionOutput {
    Create {
        bytecode: Vec<u8>,
        address: Address,
    },
    Call {
        output: Vec<u8>,
    },
}
```

3. **Storage Writes**:
```rust
pub struct StorageWrite {
    pub account: Address,
    pub key: U256,
    pub value: U256,
    pub flat_key: [u8; 32], // Derived flat storage key
}
```

4. **Account Diffs**:
```rust
pub struct AccountDiff {
    pub address: Address,
    pub nonce: Option<u64>,
    pub balance: Option<U256>,
    pub bytecode_hash: Option<Hash>,
}
```

5. **Witness Data** (proof mode):

In proof mode, `ReadWitnessSource` wraps the oracle and records all CSR communications:

```rust
pub struct ReadWitnessSource<I> {
    inner: I,
    witness_items: Vec<u32>,
}

impl IOOracle for ReadWitnessSource {
    fn process_query(&mut self, query_id: usize, params: &[usize]) -> impl Iterator<Item = u32> {
        // Record query
        self.witness_items.push(query_id as u32);
        for &param in params {
            self.witness_items.extend(u32_chunks_from_usize(param));
        }

        // Execute query
        let results = self.inner.process_query(query_id, params);

        // Record results
        for result in results {
            self.witness_items.push(result);
        }

        // Return results
        results
    }
}
```

**Final output**: `Vec<u32>` containing all oracle queries and responses, fed to zkSNARK prover

---

## Query Processing Pipeline

### CSR Communication Flow

```
┌──────────────────────────────────────────────────────────────┐
│                   RISC-V EXECUTION                            │
│                                                               │
│  fn query_oracle(query_id: usize, params: &[usize])         │
│  -> Vec<u32> {                                               │
│                                                               │
│    // 1. Write query_id to CSR                              │
│    write_csr(CSR_QUERY_ID, query_id);                       │
│                                                               │
│    // 2. Write parameters                                   │
│    for param in params {                                     │
│      write_csr(CSR_QUERY_PARAM, param);                     │
│    }                                                          │
│                                                               │
│    // 3. Signal query ready                                 │
│    write_csr(CSR_QUERY_READY, 1);                           │
│    │                                                          │
│    │  [Simulator detects CSR write]                         │
│    │         ↓                                               │
│    └────────────────────────────────────────────────────────┐
│                                                               │
│  ┌──────────────────────────────────────────────────────────┴┐
│  │              HOST ORACLE PROCESSING                        │
│  │                                                             │
│  │  1. Read buffered query_id and params                     │
│  │  2. Lookup processor:                                      │
│  │     processor_idx = ranges.get(query_id)                  │
│  │  3. Route to processor:                                    │
│  │     results = processors[idx].process(query_id, params)   │
│  │  4. Buffer results                                         │
│  │  5. Prepare for CSR reads                                 │
│  └─────────────────┬────────────────────────────────────────┘
│                     ↓                                         │
│    // 4. Read result length                                  │
│    let len = read_csr(CSR_RESULT_LEN);                       │
│                                                               │
│    // 5. Read result words                                   │
│    let mut results = Vec::with_capacity(len);               │
│    for _ in 0..len {                                         │
│      let word = read_csr(CSR_RESULT_WORD);                  │
│      results.push(word);                                     │
│    }                                                          │
│                                                               │
│    results                                                    │
│  }                                                            │
└──────────────────────────────────────────────────────────────┘
```

### Query ID Routing

**Mechanism**:

```rust
pub struct ZkEENonDeterminismSource<I> {
    processors: Vec<Box<dyn QueryProcessor>>,
    ranges: BTreeMap<usize, usize>, // query_id -> processor_index
}

impl ZkEENonDeterminismSource {
    pub fn add_external_processor<P: QueryProcessor>(&mut self, processor: P) {
        let index = self.processors.len();
        let range = processor.reserved_query_id_range();

        for query_id in range {
            self.ranges.insert(query_id, index);
        }

        self.processors.push(Box::new(processor));
    }

    pub fn process_query(&mut self, query_id: usize, params: &[usize])
    -> impl Iterator<Item = u32> {
        let processor_idx = self.ranges.get(&query_id)
            .expect("Unknown query ID");

        self.processors[processor_idx].process_buffered_query(query_id, params)
    }
}
```

### Parameter Serialization

Parameters are passed as `&[usize]` slices. Different query types expect different parameter counts:

**Examples**:

```rust
// Block metadata query (no parameters)
query_oracle(BLOCK_METADATA_QUERY_ID, &[])

// Transaction size query (no parameters)
query_oracle(NEXT_TX_SIZE_QUERY_ID, &[])

// Storage read query (flat key as 4x usize for 256 bits)
let flat_key: [u8; 32] = derive_flat_storage_key(address, key);
let params = bytes_to_usize_chunks(&flat_key);
query_oracle(STORAGE_READ_QUERY_ID, &params)

// Preimage query (hash as multiple usize)
let hash: [u8; 32] = bytecode_hash;
let params = bytes_to_usize_chunks(&hash);
query_oracle(PREIMAGE_QUERY_ID, &params)
```

### Result Streaming

Results are returned as `Iterator<Item = u32>` to support large responses without allocating upfront:

```rust
// Example: Storage tree query returns value + Merkle proof
// Can be hundreds of u32 words for deep trees

let mut results = query_oracle(STORAGE_TREE_QUERY, &params);

// Read value (8 u32s for 256-bit value)
let value = results.take(8).collect::<Vec<u32>>();

// Read proof node count
let node_count = results.next().unwrap();

// Read proof nodes
for _ in 0..node_count {
    let node = results.take(NODE_SIZE).collect::<Vec<u32>>();
    verify_node(&node);
}
```

---

## Storage Access Pipeline

### SLOAD Flow

```
┌────────────────────────────────────────────────────────────┐
│ 1. EVM SLOAD Opcode                                        │
│    Location: evm_interpreter/src/opcodes/storage.rs       │
│                                                             │
│    let address = interpreter.address;                      │
│    let key = interpreter.stack.pop()?;                     │
│    │                                                        │
│    └──────────────────────────────────────────────────────┐
│                                                             │
│ ┌──────────────────────────────────────────────────────────┴┐
│ │ 2. IOSubsystem::storage_read()                            │
│ │    Location: zk_ee/src/system/io.rs                      │
│ │                                                            │
│ │    // Check if warm (already accessed)                   │
│ │    if let Some(warm_value) = self.check_warm_cache(key) {│
│ │        charge_warm_access_gas();                          │
│ │        return warm_value.current_value;                   │
│ │    }                                                       │
│ │                                                            │
│ │    // Cold access - need oracle query                    │
│ │    charge_cold_access_gas();                              │
│ │    │                                                       │
│ │    └───────────────────────────────────────────────────┐ │
│ │                                                          │ │
│ │ ┌────────────────────────────────────────────────────────┴─┤
│ │ │ 3. Flat Storage Key Derivation                         │ │
│ │ │    Location: storage_models/src/flat_storage.rs        │ │
│ │ │                                                          │ │
│ │ │    fn derive_flat_storage_key(                         │ │
│ │ │        address: Address,                                │ │
│ │ │        key: U256                                        │ │
│ │ │    ) -> [u8; 32] {                                      │ │
│ │ │        keccak256(                                       │ │
│ │ │            address.as_bytes() ||                        │ │
│ │ │            key.to_be_bytes()                            │ │
│ │ │        )                                                 │ │
│ │ │    }                                                     │ │
│ │ │    │                                                     │ │
│ │ │    └──────────────────────────────────────────────────┐ │ │
│ │ │                                                         │ │ │
│ │ │ ┌───────────────────────────────────────────────────────┴─┤ │
│ │ │ │ 4. Oracle Query                                        │ │ │
│ │ │ │                                                         │ │ │
│ │ │ │    // Forward mode: ReadStorageResponder             │ │ │
│ │ │ │    query_oracle(                                      │ │ │
│ │ │ │        InitialStorageSlotQuery::QUERY_ID,            │ │ │
│ │ │ │        flat_key_as_usize_chunks                       │ │ │
│ │ │ │    )                                                   │ │ │
│ │ │ │                                                         │ │ │
│ │ │ │    // Proof mode: ReadTreeResponder                  │ │ │
│ │ │ │    query_oracle(                                      │ │ │
│ │ │ │        STORAGE_TREE_QUERY_ID,                        │ │ │
│ │ │ │        flat_key_as_usize_chunks                       │ │ │
│ │ │ │    )                                                   │ │ │
│ │ │ │    │                                                   │ │ │
│ │ │ │    └────────────────────────────────────────────────┐ │ │ │
│ │ │ │                                                       │ │ │ │
│ │ │ │ ┌─────────────────────────────────────────────────────┴─┤ │ │
│ │ │ │ │ 5. Responder Processes Query                        │ │ │ │
│ │ │ │ │    Location: forward_system/src/system/             │ │ │ │
│ │ │ │ │              query_processors/read_storage.rs       │ │ │ │
│ │ │ │ │                                                       │ │ │ │
│ │ │ │ │    let value = storage_tree.get(flat_key)?;        │ │ │ │
│ │ │ │ │                                                       │ │ │ │
│ │ │ │ │    // In proof mode, also return Merkle proof      │ │ │ │
│ │ │ │ │    if proof_mode {                                  │ │ │ │
│ │ │ │ │        let proof = storage_tree.get_proof(flat_key);│ │ │ │
│ │ │ │ │        return (value, proof);                       │ │ │ │
│ │ │ │ │    }                                                 │ │ │ │
│ │ │ │ │                                                       │ │ │ │
│ │ │ │ │    return value;                                    │ │ │ │
│ │ │ │ └─────────────────────────────────────────────────────┘ │ │ │
│ │ │ │                                                          │ │ │
│ │ │ │ 6. Update Warm Cache                                   │ │ │
│ │ │ │                                                          │ │ │
│ │ │ │    self.warm_cache.insert(key, WarmStorageValue {     │ │ │
│ │ │ │        initial_value: value,                           │ │ │
│ │ │ │        current_value: value,                           │ │ │
│ │ │ │        is_warm: true,                                  │ │ │
│ │ │ │        // ...                                           │ │ │
│ │ │ │    });                                                  │ │ │
│ │ │ └────────────────────────────────────────────────────────┘ │ │
│ │ │                                                             │ │
│ │ │ 7. Return Value to EE                                     │ │
│ │ └───────────────────────────────────────────────────────────┘ │
│ │                                                                │
│ │ 8. Push Value onto EVM Stack                                 │
│ │    interpreter.stack.push(value);                             │
│ └──────────────────────────────────────────────────────────────┘
│                                                                  │
│ 9. Charge Gas and Native Resources                             │
└────────────────────────────────────────────────────────────────┘
```

### SSTORE Flow

```
SSTORE opcode
     |
     v
IOSubsystem::storage_write()
     |
     v
Check warm cache
     |
     v
Create/Update WarmStorageValue:
  - initial_value (unchanged)
  - current_value (new value)
  - pubdata_diff_bytes (calculate diff)
  - is_new_storage_slot (if first write)
     |
     v
Charge gas:
  - Base cost
  - Cold access cost (if not warm)
  - Pubdata cost (per diff byte)
     |
     v
Update warm cache
     |
     v
Mark for inclusion in BlockOutput.storage_writes
```

### State Change Tracking

```rust
pub struct WarmStorageValue {
    // Value at start of transaction
    pub initial_value: U256,

    // Current value during execution
    pub current_value: U256,

    // Value at the start of tx (for gas calculations)
    pub value_at_the_start_of_tx: U256,

    // Track nested call depth
    pub changes_stack_depth: usize,

    // When last accessed (for warmth)
    pub last_accessed_at_tx_number: Option<u32>,

    // Pubdata cost (bytes that changed)
    pub pubdata_diff_bytes: u8,

    // Is this a new slot?
    pub is_new_storage_slot: bool,
}
```

**Usage**:
- On SLOAD: Check if warm, charge appropriate gas
- On SSTORE: Update current_value, calculate pubdata diff
- On transaction boundary: Move current → initial
- On REVERT: Restore from snapshot

---

## Output Formats

### BlockOutput Structure

**Location**: `forward_system/src/run/output.rs`

```rust
pub struct BlockOutput {
    /// Block header with hash, state root, etc.
    pub header: BlockHeader,

    /// Results for each transaction in order
    pub tx_results: Vec<TxResult>,

    /// All storage writes from all transactions
    pub storage_writes: Vec<StorageWrite>,

    /// Account state changes
    pub account_diffs: Vec<AccountDiff>,

    /// New preimages (bytecode, account data)
    pub published_preimages: Vec<(Hash, Vec<u8>)>,

    /// Compressed public data for L1
    pub pubdata: Vec<u8>,

    /// Total native resources used (RISC-V cycles)
    pub computational_native_used: u64,
}
```

**BlockHeader Fields**:
```rust
pub struct BlockHeader {
    pub number: u64,
    pub timestamp: u64,
    pub hash: H256,
    pub parent_hash: H256,
    pub state_root: H256,
    pub transactions_root: H256,
    pub receipts_root: H256,
    pub logs_bloom: Bloom,
    pub gas_used: u64,
    pub gas_limit: u64,
    // ... other Ethereum-like fields
}
```

### TxResult Structure

```rust
pub struct TxResult {
    /// Success or failure with output data
    pub execution_result: ExecutionResult,

    /// Gas consumed by transaction
    pub gas_used: u64,

    /// Gas refunded to sender
    pub gas_refunded: u64,

    /// Native RISC-V resources used
    pub computational_native_used: u64,

    /// Pubdata bytes used
    pub pubdata_used: u64,

    /// Contract address for CREATE transactions
    pub contract_address: Option<Address>,

    /// EVM logs (LOG0-LOG4)
    pub logs: Vec<Log>,

    /// L2→L1 messages
    pub l2_to_l1_logs: Vec<L2ToL1LogWithPreimage>,
}

pub enum ExecutionResult {
    /// Successful execution
    Success(ExecutionOutput),

    /// Reverted with optional message
    Revert { output: Vec<u8> },
}

pub enum ExecutionOutput {
    /// Contract creation
    Create {
        bytecode: Vec<u8>,
        address: Address,
    },

    /// Contract call
    Call {
        output: Vec<u8>,
    },
}
```

### Witness Data Format

**Type**: `Vec<u32>`

**Structure**: Flat stream of u32 words containing all oracle queries and responses

**Example Witness Stream**:
```
[
    // Query 1: Block metadata
    BLOCK_METADATA_QUERY_ID,     // Query ID
    0,                            // Param count
    20,                           // Result length
    chain_id_word0,               // Result data
    chain_id_word1,
    block_number_word0,
    block_number_word1,
    timestamp_word0,
    // ... more metadata words

    // Query 2: First transaction size
    NEXT_TX_SIZE_QUERY_ID,        // Query ID
    0,                            // Param count
    1,                            // Result length
    128,                          // TX size in bytes

    // Query 3: First transaction data
    TX_DATA_WORDS_QUERY_ID,       // Query ID
    1,                            // Param count
    128,                          // Size param
    32,                           // Result length (128 bytes / 4)
    tx_word0,                     // Transaction data
    tx_word1,
    // ... 30 more tx words

    // Query 4: Storage read
    STORAGE_READ_QUERY_ID,        // Query ID
    8,                            // Param count (32-byte key)
    key_word0,                    // Key params
    key_word1,
    // ... 6 more key words
    8,                            // Result length (32-byte value)
    value_word0,                  // Value data
    value_word1,
    // ... 6 more value words

    // ... thousands more queries
]
```

**Usage**: Fed to zkSNARK prover to generate proof that execution was correct

---

## Data Transformation Examples

### Example 1: Transaction Execution Flow

**Input**: RLP-encoded transaction

```
Transaction bytes (RLP):
[0xf8, 0x6c, 0x80, 0x85, 0x04, 0xa8, 0x17, 0xc8, 0x00, ...]
```

**Transformation Steps**:

1. **TxDataResponder receives query**:
```rust
// Bootloader queries transaction size
let size = query_oracle(NEXT_TX_SIZE_QUERY_ID, &[]);
// Returns: 108 (0x6c bytes)
```

2. **TxDataResponder provides data**:
```rust
// Bootloader queries transaction data
let tx_data = query_oracle(TX_DATA_WORDS_QUERY_ID, &[108]);
// Returns: [0xf86c8085, 0x04a817c8, 0x00..., ...] (as u32 chunks)
```

3. **Bootloader parses RLP**:
```rust
let tx = parse_rlp(&tx_data);
// Extracts:
// - nonce: 0
// - gas_price: 20000000000 (20 gwei)
// - gas_limit: 21000
// - to: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
// - value: 1000000000000000000 (1 ETH)
// - data: []
// - v, r, s: signature components
```

4. **Signature recovery**:
```rust
let sender = recover_sender(&tx)?;
// Returns: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
```

5. **Create call request**:
```rust
let call_request = ExternalCallRequest {
    caller: sender,
    callee: tx.to,
    calldata: tx.data,
    call_value: U256::from(1_000_000_000_000_000_000u64),
    available_resources: Resources::from_ergs(21000),
    // ...
};
```

6. **EVM executes** (in this case, simple ETH transfer):
```rust
// No bytecode, just value transfer
system.io.transfer_value(sender, tx.to, tx.value)?;
```

7. **Result collection**:
```rust
TxResult {
    execution_result: ExecutionResult::Success(ExecutionOutput::Call {
        output: vec![],
    }),
    gas_used: 21000,
    gas_refunded: 0,
    computational_native_used: 142857, // RISC-V cycles
    pubdata_used: 0,
    contract_address: None,
    logs: vec![],
    l2_to_l1_logs: vec![],
}
```

**Output**: TxResult included in BlockOutput.tx_results

### Example 2: Storage Access Transformation

**Input**: SLOAD opcode with address and key

```
EVM execution:
  Address: 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
  Key: 0x0000000000000000000000000000000000000000000000000000000000000001
```

**Transformation Steps**:

1. **SLOAD opcode handler**:
```rust
// Pop key from stack
let key = interpreter.stack.pop()?;
// key = U256::from(1)
```

2. **Derive flat storage key**:
```rust
let address = interpreter.address;
let flat_key = derive_flat_storage_key(address, key);

// Computation:
// flat_key = keccak256(
//     0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb ||
//     0x0000000000000000000000000000000000000000000000000000000000000001
// )
// Result: 0x3a...4f (32-byte hash)
```

3. **Oracle query**:
```rust
// Convert flat_key to usize chunks
let params = [
    flat_key[0..8],   // 0x3a..
    flat_key[8..16],  // ..
    flat_key[16..24], // ..
    flat_key[24..32], // ..4f
];

let result = query_oracle(InitialStorageSlotQuery::QUERY_ID, &params);
```

4. **ReadStorageResponder processes**:
```rust
// Look up in storage tree
let value = storage_tree.get(&flat_key)?;
// Returns: 0x0000000000000000000000000000000000000000000000000000000000000042
```

5. **Result streaming**:
```rust
// Split value into u32 chunks
[
    0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000042,
]
```

6. **Reconstruct in EVM**:
```rust
// Combine u32 chunks back to U256
let value = U256::from_big_endian(&u32_chunks_to_bytes(&result));
// value = U256::from(0x42) = 66
```

7. **Push to stack**:
```rust
interpreter.stack.push(value)?;
```

8. **Update warm cache**:
```rust
warm_cache.insert(WarmStorageKey { address, key }, WarmStorageValue {
    initial_value: value,
    current_value: value,
    value_at_the_start_of_tx: value,
    is_warm: true,
    pubdata_diff_bytes: 0,
    is_new_storage_slot: false,
    // ...
});
```

**Output**: Value available on EVM stack, cache updated

### Example 3: Witness Collection

**Input**: Block execution with witness recording enabled

```rust
let inner_oracle = ZkEENonDeterminismSource::new();
// ... register processors ...

let witness_oracle = ReadWitnessSource::new(inner_oracle);
```

**Transformation Steps**:

1. **First query**: Block metadata
```
Query:
  ID: BLOCK_METADATA_QUERY_ID (0x1000)
  Params: []

Witness records:
  [0x1000, 0, 20, ...]  // ID, param count, result length, results
```

2. **Second query**: Transaction size
```
Query:
  ID: NEXT_TX_SIZE_QUERY_ID (0x2000)
  Params: []

Witness records:
  [0x2000, 0, 1, 108]  // ID, param count, result length, size
```

3. **Third query**: Storage read
```
Query:
  ID: STORAGE_READ_QUERY_ID (0x4000)
  Params: [flat_key as 8x u32]

Witness records:
  [0x4000, 8, key0, key1, ..., key7, 8, val0, val1, ..., val7]
```

4. **Fourth query**: Preimage (bytecode)
```
Query:
  ID: PREIMAGE_QUERY_ID (0x5000)
  Params: [bytecode_hash as 8x u32]

Witness records:
  [0x5000, 8, hash0, hash1, ..., hash7, 1024, byte0, byte1, ..., byte1023]
```

5. **Continue for all queries throughout execution**...

6. **Extract witness**:
```rust
let witness = witness_oracle.finalize();
// witness: Vec<u32> with ~100,000 - 1,000,000 words
```

7. **Feed to prover**:
```rust
let proof = zksnark_prover::prove(
    circuit,
    witness,
    proving_key,
);
```

**Output**: zkSNARK proof that execution was correct

---

## Query ID Reference Table

| Query ID Range | Constant Name | Processor | Purpose |
|---|---|---|---|
| 0x1000-0x1FFF | `BLOCK_METADATA_QUERY_ID` | BlockMetadataResponder | Block-level metadata |
| 0x2000 | `NEXT_TX_SIZE_QUERY_ID` | TxDataResponder | Next transaction size |
| 0x2001 | `TX_DATA_WORDS_QUERY_ID` | TxDataResponder | Transaction data bytes |
| 0x2002 | `TX_ENCODING_FORMAT_QUERY_ID` | TxDataResponder | TX encoding (RLP/ABI) |
| 0x2003 | `TX_FROM_QUERY_ID` | TxDataResponder | TX sender address |
| 0x3000 | `STORAGE_TREE_QUERY_ID` | ReadTreeResponder | Storage value + Merkle proof |
| 0x3001 | `MERKLE_PROOF_QUERY_ID` | ReadTreeResponder | Merkle proof only |
| 0x4000 | `InitialStorageSlotQuery::QUERY_ID` | ReadStorageResponder | Storage value (no proof) |
| 0x5000 | `PREIMAGE_QUERY_ID` | GenericPreimageResponder | Preimage for hash |
| 0x6000 | `ZK_PROOF_DATA_QUERY_ID` | ZKProofDataResponder | Proof data |
| 0x7000 | `DA_COMMITMENT_QUERY_ID` | DACommitmentSchemeResponder | DA commitment scheme |
| 0x8000-0x8FFF | `ARITHMETIC_QUERY_ID` | ArithmeticQuery | Field arithmetic |
| 0x9000 | `BLOB_KZG_COMMITMENT_QUERY_ID` | BlobKZGCommitmentQuery | EIP-4844 blob commitments |
| 0xFFFF | `UART_QUERY_ID` | UARTPrintResponder | Debug output (dev only) |
| 0xFFFE | `DISCONNECT_ORACLE_QUERY_ID` | (none) | Signal end of queries |

**Code References**:
- Query ID constants: `zk_ee/src/oracle/query_ids.rs`
- Processor registration: `forward_system/src/run/mod.rs:65-150`

---

## Summary

Data flow in zkSync OS follows a clear pipeline:

1. **Input** enters through API functions or RISC-V entry
2. **Oracle** provides all external data via query processors
3. **Bootloader** initializes system and manages transaction loop
4. **Execution Environments** process transactions with opcode execution
5. **System Layer** handles I/O, storage, events through IOSubsystem
6. **Result Keeper** accumulates all execution effects
7. **Output** formatted as BlockOutput or witness stream

**Key principles**:
- **Determinism**: Same inputs always produce same outputs
- **Oracle abstraction**: All external data goes through oracle queries
- **Preemption model**: EEs don't call each other directly
- **Witness recording**: All oracle interactions recorded for proving

**For more details**:
- Entry points → [API.md](API.md)
- Type details → [KeyTypes.md](KeyTypes.md)
- Execution environments → [ExecutionEnvironments.md](ExecutionEnvironments.md)
- Storage internals → [Storage.md](Storage.md)
- Proof generation → [ProofSystem.md](ProofSystem.md)
