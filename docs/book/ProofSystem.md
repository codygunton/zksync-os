# Proof System in zkSync OS

## Introduction

The zkSync OS proof system enables trustless verification of L2 execution through zero-knowledge proofs. This document details how zkSync OS executes in a RISC-V environment to generate verifiable computation proofs.

**Zero-Knowledge Proof System Overview:**
- **Purpose**: Generate cryptographic proofs that block execution was performed correctly
- **Verifier**: Anyone can verify proofs on L1 Ethereum without re-executing transactions
- **Succinctness**: Proofs are small (~200KB) regardless of computation size
- **Privacy**: Proofs reveal only public inputs/outputs, not intermediate computation

**Why RISC-V?**
- **Standardization**: RISC-V is an open ISA with well-defined semantics
- **Determinism**: ISA-level determinism easier than language-level (vs. native x86/ARM)
- **Proof System Integration**: zkSNARK circuits for RISC-V are well-studied
- **Flexibility**: Same binary can be simulated or proven without modification
- **Ecosystem**: Leverage existing RISC-V toolchains (LLVM, GCC, objcopy)

**Proof Generation Workflow:**

```
┌──────────────────────────────────────────────────────────────┐
│                    INPUT PREPARATION                          │
│  • Block metadata, transactions, storage state               │
│  • Compile to RISC-V binary (dump_bin.sh)                   │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│                 WITNESS GENERATION PHASE                      │
│  1. Load RISC-V binary in simulator                          │
│  2. Execute with oracle providing external data              │
│  3. Record all CSR reads/writes (oracle queries)             │
│  4. Output: Witness stream (Vec<u32>)                        │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│                    PROVING PHASE                              │
│  1. Feed witness to zkSNARK prover (airbender)               │
│  2. Generate proof of correct RISC-V execution               │
│  3. Output: zkSNARK proof (~200KB)                           │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│                 VERIFICATION & SUBMISSION                     │
│  1. Verify proof locally (optional)                          │
│  2. Submit proof to L1 verifier contract                     │
│  3. L1 updates state root after verification                 │
└──────────────────────────────────────────────────────────────┘
```

---

## RISC-V Execution Environment

### Entry Point

**Location**: `zksync_os/src/main.rs`

**Bare-Metal Startup Sequence:**

```rust
#[link_section = ".init.rust"]
#[export_name = "_start_rust"]
unsafe extern "C" fn start_rust() -> ! {
    main()
}

#[inline(never)]
fn main() -> ! {
    unsafe { workload() }
}

unsafe fn workload() -> ! {
    // 1. Setup allocator with heap boundaries
    let heap_start = addr_of_mut!(_sheap);
    let heap_end = addr_of_mut!(_eheap);

    // 2. Load ROM sections to RAM
    load_to_ram(/* .rodata */);
    load_to_ram(/* .data */);

    // 3. Initialize Talc allocator
    init_allocator(heap_start, heap_end);

    // 4. Run proving bootloader
    let output = run_proving::<
        CSRBasedNonDeterminismSource,
        LoggerTy,
    >(heap_start, heap_end);

    // 5. Exit with output in registers x10-x17
    zksync_os_finish_success(&output);
}
```

**Key Points:**
- `#[no_std]`, `#[no_main]`: No standard library or OS runtime
- `_start_rust()`: Assembly bootstrap transfers control here
- Memory sections managed explicitly (heap, stack, .data, .rodata)
- Output returned via RISC-V registers x10-x17 (convention for 256-bit result)

### Bare-Metal Environment

**No Operating System:**
- No syscalls, no file system, no threads, no dynamic linking
- All I/O through CSR (Control & Status Registers)
- Exception/trap handling implemented manually

**Memory Layout** (defined in linker script):

```
ROM (read-only):
  0x00000000 - 0x001FFFFF  (2MB)
  ├─ .text      : Executable code
  ├─ .rodata    : Read-only data (copied to RAM at startup)
  └─ .data init : Initial values for .data (copied to RAM)

RAM (read-write):
  0x00200000 - 0xFFFFFFFF
  ├─ .data      : Mutable static data (copied from ROM)
  ├─ .bss       : Zero-initialized data
  ├─ Heap       : Dynamic allocations (_sheap to _eheap)
  └─ Stack      : Call frames (_sstack to _estack, grows downward)
```

**Linker Script Symbols:**

```c
extern "C" {
    static mut _sheap: usize;   // Heap start
    static mut _eheap: usize;   // Heap end
    static mut _sstack: usize;  // Stack start
    static mut _estack: usize;  // Stack end
    static mut _sidata: usize;  // .data in ROM
    static mut _sdata: usize;   // .data in RAM start
    static mut _edata: usize;   // .data in RAM end
    static mut _sirodata: usize; // .rodata in ROM
    static mut _srodata: usize;  // .rodata in RAM start
    static mut _erodata: usize;  // .rodata in RAM end
}
```

### Memory Management with Talc Allocator

**Location**: `proof_running_system/src/talc.rs`

**Talc**: A simple, deterministic bump allocator suitable for RISC-V:
- **Bump allocation**: Allocate by incrementing a pointer
- **Simple free list**: Basic coalescing for deallocation
- **Deterministic**: No randomization, no complex heuristics
- **Bounded**: Pre-defined heap region (heap_start to heap_end)

**Allocator Initialization:**

```rust
pub unsafe fn init_allocator(heap_start: *mut usize, heap_end: *mut usize) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "scalloc")] {
            // Size-class allocator (alternative)
            SizeClassesAllocator::init(
                USED_ALLOCATOR.as_mut_ptr(),
                heap_start,
                heap_end,
            );
        } else {
            // Talc (default)
            create_talc_allocator_wrapper(
                USED_ALLOCATOR.as_mut_ptr(),
                heap_start,
                heap_end,
            );
        }
    }
}
```

**ProxyAllocator Pattern:**

```rust
static mut USED_ALLOCATOR: MaybeUninit<TalcWrapper> = MaybeUninit::uninit();

#[derive(Clone, Copy, Debug, Default)]
pub struct ProxyAllocator;

unsafe impl Allocator for ProxyAllocator {
    fn allocate(&self, layout: Layout)
    -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            USED_ALLOCATOR.assume_init_ref().allocate(layout)
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        USED_ALLOCATOR.assume_init_ref().deallocate(ptr, layout)
    }
}
```

**Why This Design:**
- Global allocator initialized once at startup
- All allocations go through ProxyAllocator → TalcWrapper
- Deterministic allocation patterns for reproducible proofs
- No global state changes after initialization

### CSR-Based Oracle Communication Protocol

**Location**: `zksync_os/src/main.rs:100-122`

**CSR (Control & Status Register) Overview:**

RISC-V defines special registers for system-level operations. zkSync OS repurposes one CSR for oracle communication:

- **CSR writes**: Send query requests and parameters to oracle
- **CSR reads**: Receive query results from oracle
- **No memory access**: All data flows through CSR register

**CSRBasedNonDeterminismSource Implementation:**

```rust
#[derive(Clone, Copy, Debug)]
pub struct CSRBasedNonDeterminismSource;

impl NonDeterminismCSRSourceImplementation for CSRBasedNonDeterminismSource {
    #[inline(always)]
    fn csr_read_impl() -> usize {
        csr_read_word() as usize
    }

    #[inline(always)]
    fn csr_write_impl(value: usize) {
        core::hint::black_box(csr_write_word(value))
    }
}
```

**Complete CSR Communication Protocol:**

```
┌────────────────────────────────────────────────────────────────┐
│                 RISC-V CODE (Guest)                             │
│                                                                 │
│  fn query_oracle(query_id: u32, params: &[u32]) -> Vec<u32> { │
│                                                                 │
│    // 1. Write query ID                                        │
│    csr_write(query_id);                                        │
│    │                                                            │
│    │ [Simulator intercepts CSR write]                         │
│    ▼                                                            │
├────────────────────────────────────────────────────────────────┤
│                  HOST ORACLE (Simulator)                        │
│                                                                 │
│    // Buffer query_id                                          │
│    query_buffer.query_type = query_id;                         │
│    query_buffer.remaining_len = None;                          │
│                                                                 │
├────────────────────────────────────────────────────────────────┤
│                 RISC-V CODE (Guest)                             │
│                                                                 │
│    // 2. Write parameter count                                 │
│    csr_write(params.len());                                    │
│    │                                                            │
│    ▼                                                            │
├────────────────────────────────────────────────────────────────┤
│                  HOST ORACLE (Simulator)                        │
│                                                                 │
│    // Allocate buffer for params                               │
│    query_buffer.remaining_len = Some(params.len());            │
│                                                                 │
├────────────────────────────────────────────────────────────────┤
│                 RISC-V CODE (Guest)                             │
│                                                                 │
│    // 3. Write parameters                                      │
│    for param in params {                                       │
│      csr_write(param);                                         │
│    }                                                            │
│    │                                                            │
│    ▼                                                            │
├────────────────────────────────────────────────────────────────┤
│                  HOST ORACLE (Simulator)                        │
│                                                                 │
│    // Buffer parameters                                        │
│    query_buffer.buffer.push(param);                            │
│    query_buffer.remaining_len -= 1;                            │
│                                                                 │
│    // When complete, process query                             │
│    if query_buffer.remaining_len == 0 {                        │
│      let processor_id = ranges.get(query_id);                  │
│      let results = processors[processor_id]                    │
│        .process_buffered_query(query_id, buffer);              │
│                                                                 │
│      current_iterator = Some(results);                         │
│      iterator_len_to_indicate = Some(results.len() * 2);       │
│    }                                                            │
│                                                                 │
├────────────────────────────────────────────────────────────────┤
│                 RISC-V CODE (Guest)                             │
│                                                                 │
│    // 4. Read result length                                    │
│    let len = csr_read();                                       │
│    │                                                            │
│    ▼                                                            │
├────────────────────────────────────────────────────────────────┤
│                  HOST ORACLE (Simulator)                        │
│                                                                 │
│    // Return buffered length                                   │
│    return iterator_len_to_indicate.take();                     │
│                                                                 │
├────────────────────────────────────────────────────────────────┤
│                 RISC-V CODE (Guest)                             │
│                                                                 │
│    // 5. Read result words                                     │
│    let mut results = Vec::with_capacity(len);                  │
│    for _ in 0..len {                                           │
│      let word = csr_read();                                    │
│      results.push(word);                                       │
│    }                                                            │
│    │                                                            │
│    ▼                                                            │
├────────────────────────────────────────────────────────────────┤
│                  HOST ORACLE (Simulator)                        │
│                                                                 │
│    // Stream results from iterator                             │
│    let next = current_iterator.next();                         │
│                                                                 │
│    // Handle 32/64-bit mismatch                                │
│    if on_low_half {                                            │
│      high_half = Some((next >> 32) as u32);                    │
│      return (next & 0xFFFFFFFF) as u32;                        │
│    } else {                                                     │
│      return high_half.take();                                  │
│    }                                                            │
│                                                                 │
├────────────────────────────────────────────────────────────────┤
│                 RISC-V CODE (Guest)                             │
│                                                                 │
│    return results;                                             │
│  }                                                              │
└────────────────────────────────────────────────────────────────┘
```

**32-bit / 64-bit Handling:**
- RISC-V is 32-bit (RV32I), host is 64-bit
- Oracle stores results as `usize` (64-bit on host)
- Each 64-bit word split into two 32-bit CSR reads
- Low half returned first, high half cached for next read

**Query Buffering:**
- Oracle accumulates writes until parameter count reached
- Then processes query and prepares result iterator
- Result streamed word-by-word on subsequent reads

**Disconnect Protocol:**
- Special query ID `DISCONNECT_ORACLE_QUERY_ID` (0xFFFE)
- Signals end of oracle interactions
- Allows post-processing without oracle (e.g., final hash computation)

---

## Witness Generation

### What is a Witness?

A **witness** is a complete execution trace that allows a prover to generate a zero-knowledge proof. For zkSync OS:

- **All non-deterministic inputs**: Storage values, block metadata, transactions
- **All oracle query results**: Every response from external data sources
- **Execution is reproducible**: Given witness, prover can re-execute and verify correctness

**Witness vs. Execution:**
- **Forward execution**: Native code, fast, uses databases/RPCs
- **Proof execution**: RISC-V code, uses witness stream, deterministic
- **Witness generation**: RISC-V simulation that records all oracle queries

**Why Witness?**
- zkSNARK prover needs all computation inputs to generate proof
- Determinism: Same inputs must produce same outputs
- Efficiency: Prover can parallelize with pre-recorded witness
- Separation: Witness generation (slow) vs. proving (parallelizable)

### ReadWitnessSource Wrapper

**Location**: `oracle_provider/src/lib.rs:294-324`

**Purpose**: Wraps any oracle and records all CSR read operations

```rust
pub struct ReadWitnessSource<M: MemorySource> {
    original_source: ZkEENonDeterminismSource<M>,
    read_items: Rc<RefCell<Vec<u32>>>,
}

impl<M: MemorySource> ReadWitnessSource<M> {
    pub fn new(original_source: ZkEENonDeterminismSource<M>) -> Self {
        Self {
            original_source,
            read_items: Rc::new(RefCell::new(vec![])),
        }
    }

    pub fn get_read_items(&self) -> Rc<RefCell<Vec<u32>>> {
        self.read_items.clone()
    }
}
```

**Recording Implementation:**

```rust
impl<M: MemorySource> NonDeterminismCSRSource<M> for ReadWitnessSource<M> {
    fn read(&mut self) -> u32 {
        // Delegate to original oracle
        let item = self.original_source.read();

        // Record the read value
        self.read_items.borrow_mut().push(item);

        item
    }

    fn write_with_memory_access(&mut self, memory: &M, value: u32) {
        // Pass through writes (don't record)
        self.original_source.write_with_memory_access(memory, value);
    }
}
```

**Key Insight**: Only reads are recorded, not writes
- Writes are deterministic (query IDs, parameters) and don't need recording
- Reads contain all non-deterministic data from external sources
- Prover can replay execution by feeding recorded reads back through CSR

### Witness Data Format: Vec<u32>

**Format**: Flat stream of 32-bit words

**Structure**: Sequence of query-response pairs

```
[
    // Query 1: Block metadata
    len_1,              // Length indicator (for this query's results)
    result_1_word_0,
    result_1_word_1,
    ...,
    result_1_word_N,

    // Query 2: Transaction size
    len_2,
    result_2_word_0,

    // Query 3: Storage read
    len_3,
    result_3_word_0,
    result_3_word_1,
    ...,

    // ... thousands more queries
]
```

**Example Witness Stream:**

```rust
// Block metadata query
vec![
    20,                     // 20 words of metadata
    0x00000001,             // chain_id (word 0)
    0x00000000,             // chain_id (word 1)
    0x00000064,             // block_number (word 0)
    0x00000000,             // block_number (word 1)
    0x5F5E100,              // timestamp (word 0)
    0x00000000,             // timestamp (word 1)
    // ... 14 more metadata words

    // Transaction size query
    1,                      // 1 word result
    108,                    // TX size: 108 bytes

    // Transaction data query
    27,                     // 27 words (108 bytes / 4)
    0xf86c8085,             // TX data word 0 (RLP header)
    0x04a817c8,             // TX data word 1
    // ... 25 more TX words

    // Storage read query
    8,                      // 8 words (256-bit value)
    0x00000000,             // Value word 0
    0x00000000,             // Value word 1
    0x00000000,             // Value word 2
    0x00000000,             // Value word 3
    0x00000000,             // Value word 4
    0x00000000,             // Value word 5
    0x00000000,             // Value word 6
    0x00000042,             // Value word 7 (actual value: 0x42)

    // ... continue for entire block execution
]
```

**Witness Size Estimates:**
- Small block (10 TXs): ~100KB - 500KB witness
- Medium block (100 TXs): ~1MB - 5MB witness
- Large block (1000 TXs): ~10MB - 50MB witness

**Compression**: Witness can be compressed before storage/transmission

### API: run_block_generate_witness

**Location**: `api/src/lib.rs`

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

**Parameters:**
- `block_context`: Block metadata (number, timestamp, gas limits, etc.)
- `tree`: In-memory storage tree with initial state
- `preimage_source`: Bytecode and account data by hash
- `tx_source`: List of transactions to execute
- `proof_data`: Previous block state root and timestamp for validation
- `da_commitment_scheme`: Data availability scheme (Blobs, Keccak256, etc.)
- `zksync_os_bin_path`: Path to RISC-V binary (e.g., "zksync_os/for_tests.bin")

**Return Value:**
- `Vec<u32>`: Witness stream containing all oracle queries and responses

**Example Usage:**

```rust
use zksync_os_api::run_block_generate_witness;

let block_context = BlockContext {
    chain_id: 1,
    block_number: 100,
    timestamp: 1234567890,
    gas_limit: 30_000_000,
    // ... other fields
};

let tree = InMemoryTree::new();
tree.insert(storage_key, storage_value);

let preimage_source = InMemoryPreimageSource::new();
preimage_source.add_bytecode(bytecode_hash, bytecode);

let tx_source = TxListSource::new(vec![tx1, tx2, tx3]);

let proof_data = ProofData {
    state_root_view: previous_state_root,
    last_block_timestamp: 1234567800,
};

let witness = run_block_generate_witness(
    block_context,
    tree,
    preimage_source,
    tx_source,
    proof_data,
    DACommitmentScheme::BlobsZKsyncOS,
    "zksync_os/for_tests.bin",
)?;

// witness now contains ~1-50MB of u32 words
// Feed to zkSNARK prover
let proof = prover.generate_proof(witness);
```

### Witness Generation Flow Diagram

```
┌──────────────────────────────────────────────────────────────┐
│               API: run_block_generate_witness()               │
│  • BlockContext, Tree, Preimages, Transactions               │
│  • RISC-V binary path                                        │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│            1. CREATE ORACLE WITH PROCESSORS                   │
│  let mut oracle = ZkEENonDeterminismSource::new();           │
│  oracle.add_external_processor(BlockMetadataResponder);      │
│  oracle.add_external_processor(TxDataResponder);             │
│  oracle.add_external_processor(ReadTreeResponder);           │
│  oracle.add_external_processor(ReadStorageResponder);        │
│  oracle.add_external_processor(GenericPreimageResponder);    │
│  oracle.add_external_processor(ZKProofDataResponder);        │
│  oracle.add_external_processor(DACommitmentSchemeResponder); │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│              2. WRAP WITH WITNESS RECORDER                    │
│  let witness_oracle = ReadWitnessSource::new(oracle);        │
│  let read_items = witness_oracle.get_read_items();           │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│              3. LOAD RISC-V BINARY                            │
│  let binary = std::fs::read(zksync_os_bin_path)?;            │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│              4. CONFIGURE SIMULATOR                           │
│  let config = SimulatorConfig {                              │
│    bin: BinarySource::Path(bin_path),                        │
│    cycles: 1 << 36,  // ~68 billion cycle limit             │
│    entry_point: 0,                                           │
│    diagnostics: None,                                        │
│  };                                                           │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│              5. RUN SIMULATOR                                 │
│  risc_v_simulator::runner::                                  │
│    run_simple_with_entry_point_and_non_determinism_source(   │
│      config,                                                  │
│      witness_oracle,                                         │
│    );                                                         │
│                                                               │
│  Simulator executes RISC-V binary:                           │
│  • _start_rust() → workload() → run_proving()               │
│  • Bootloader loads TXs from oracle                         │
│  • EVM executes transactions                                 │
│  • Storage reads via oracle                                  │
│  • All CSR reads recorded by ReadWitnessSource              │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│              6. EXTRACT WITNESS                               │
│  let witness = read_items.borrow().clone();                  │
│  // witness: Vec<u32> with all recorded CSR reads            │
└────────────────────┬─────────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────────┐
│              7. RETURN WITNESS                                │
│  return witness;                                              │
│  // Ready for zkSNARK prover                                 │
└──────────────────────────────────────────────────────────────┘
```

**Flow Summary:**
1. Setup oracle with all query processors (metadata, TXs, storage)
2. Wrap oracle with `ReadWitnessSource` to record all reads
3. Load RISC-V binary compiled from zkSync OS
4. Configure simulator with cycle limits and binary path
5. Execute RISC-V binary in simulator with witness-recording oracle
6. Extract recorded witness stream from `ReadWitnessSource`
7. Return witness for proving

---

## RISC-V Simulator

### zksync_os_runner Purpose

**Location**: `zksync_os_runner/src/lib.rs`

**Purpose**: Execute RISC-V binaries in a simulated environment for:
- **Witness generation**: Record oracle queries for proving
- **Testing**: Validate execution without full proof generation
- **Development**: Debug zkSync OS without compiling to native
- **Benchmarking**: Profile RISC-V performance

**Key Differences from Real Prover:**
- Simulator interprets RISC-V instructions (slow)
- Prover generates zkSNARK circuits (much slower, but produces proof)
- Simulator used for witness generation, prover consumes witness

### run() Function

**Signature:**

```rust
pub fn run(
    img_path: PathBuf,
    diagnostics: Option<DiagnosticsConfig>,
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>,
) -> [u32; 8]
```

**Parameters:**
- `img_path`: Path to RISC-V binary (e.g., "zksync_os/for_tests.bin")
- `diagnostics`: Optional profiling config (flamegraph, cycle counts)
- `cycles`: Maximum number of RISC-V cycles (prevents infinite loops)
- `non_determinism_source`: Oracle for CSR reads/writes

**Return Value:**
- `[u32; 8]`: 256-bit output (public input for proof)
- Extracted from RISC-V registers x10-x17 after execution

**Implementation:**

```rust
pub fn run(
    img_path: PathBuf,
    diagnostics: Option<DiagnosticsConfig>,
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>,
) -> [u32; 8] {
    println!("ZK RISC-V simulator is starting");

    // 1. Read binary file
    let mut file = std::fs::File::open(img_path.clone())
        .expect("Binary file missing");
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).expect("Cannot read file");

    // 2. Configure simulator
    let config = SimulatorConfig {
        bin: BinarySource::Path(img_path),
        cycles,
        entry_point: 0,  // Start at address 0
        diagnostics,
    };

    // 3. Run simulator
    let run_result = risc_v_simulator::runner::
        run_simple_with_entry_point_and_non_determimism_source(
            config,
            non_determinism_source,
        );

    // 4. Print opcode statistics (if enabled)
    risc_v_simulator::cycle::state::output_opcode_stats();

    // 5. Extract output from registers x10-x17 (convention)
    run_result.state.registers[10..18]
        .try_into()
        .unwrap()
}
```

**Output Convention:**
- zkSync OS returns 256-bit hash (public input) in registers x10-x17
- This becomes the public input to the zkSNARK proof
- L1 verifier checks this hash matches state commitment

### Integration with risc_v_simulator from airbender

**Repository**: `zksync-airbender` (separate crate)

**Simulator Architecture:**

```
risc_v_simulator crate (from airbender):
├─ abstractions/
│  ├─ memory.rs          - Memory interface (VectorMemoryImpl)
│  └─ non_determinism.rs - CSR interface (NonDeterminismCSRSource)
├─ cycle/
│  ├─ state.rs           - CPU state (registers, PC, memory)
│  ├─ opcodes/           - Opcode implementations (ADD, LOAD, etc.)
│  └─ IMStandardIsaConfig - RV32I instruction set config
├─ runner/
│  └─ run_simple_with_entry_point_and_non_determinism_source()
└─ sim/
   ├─ DiagnosticsConfig   - Profiling configuration
   ├─ ProfilerConfig      - Flamegraph generation
   └─ SimulatorConfig     - Main config (binary, cycles, entry point)
```

**VectorMemoryImpl**: In-memory storage for RISC-V address space
- Backed by `Vec<u32>` for fast access
- Supports 4GB address space (32-bit)
- ROM region (0-2MB) loaded from binary
- RAM region (2MB+) zero-initialized

**NonDeterminismCSRSource**: Interface for CSR oracle
```rust
trait NonDeterminismCSRSource<M: MemorySource> {
    fn read(&mut self) -> u32;
    fn write_with_memory_access(&mut self, memory: &M, value: u32);
}
```

**IMStandardIsaConfig**: RV32I instruction set configuration
- Base integer instructions (ADD, SUB, AND, OR, XOR, etc.)
- Load/Store (LW, SW, LB, SB, etc.)
- Branches (BEQ, BNE, BLT, BGE, etc.)
- Jumps (JAL, JALR)
- System (ECALL, EBREAK, CSR instructions)

### Cycle Limits

**Default Limit**: `1 << 36` cycles (~68 billion)

**Why Limits?**
- Prevent infinite loops in buggy code
- Bound proving time (linear in cycles)
- Detect performance regressions

**Typical Cycle Counts:**
- Empty block: ~10 million cycles
- Small block (10 TXs): ~100 million cycles
- Medium block (100 TXs): ~1 billion cycles
- Large block (1000 TXs): ~10 billion cycles

**Per-Operation Costs:**
- Basic arithmetic (ADD, SUB): 1 cycle
- Memory access (LOAD, STORE): 1 cycle
- Branch: 1 cycle
- CSR read: 1 cycle (but oracle processing on host is expensive)
- Function call: ~10 cycles (save/restore registers)

**Optimization Strategies:**
- Minimize CSR queries (cache results)
- Use RISC-V-optimized algorithms
- Batch operations where possible
- Profile with diagnostics to find hotspots

### Diagnostics and Profiling

**DiagnosticsConfig:**

```rust
pub struct DiagnosticsConfig {
    /// Path to symbol file (.elf with debug symbols)
    pub symbols_path: PathBuf,

    /// Optional profiler configuration
    pub profiler_config: Option<ProfilerConfig>,
}

pub struct ProfilerConfig {
    /// Path to output flamegraph (SVG)
    pub output_path: PathBuf,

    /// Sample every N cycles (1 = every cycle, 1000 = every 1000 cycles)
    pub frequency_recip: usize,

    /// Reverse graph (callees at top)
    pub reverse_graph: bool,
}
```

**Example Usage:**

```rust
let diagnostics = DiagnosticsConfig {
    symbols_path: PathBuf::from("zksync_os/for_tests.elf"),
    profiler_config: Some(ProfilerConfig {
        output_path: PathBuf::from("profile.svg"),
        frequency_recip: 1000,  // Sample every 1000 cycles
        reverse_graph: false,
    }),
};

let output = zksync_os_runner::run(
    PathBuf::from("zksync_os/for_tests.bin"),
    Some(diagnostics),
    1 << 36,
    oracle,
);
```

**Flamegraph Output:**
- SVG file showing function call hierarchy
- Width = % of total cycles spent
- Click to zoom into hot functions
- Useful for optimization

**Opcode Statistics:**
```
ADD:    12,345,678 (15.2%)
LW:     10,234,567 (12.6%)
SW:      8,765,432 (10.8%)
BEQ:     7,654,321 ( 9.4%)
...
```

---

## Forward vs Proof Execution

zkSync OS supports two execution modes with different tradeoffs:

### Forward Execution (Native)

**Purpose**: Fast execution for sequencer operation

**Characteristics:**
- **Target**: Native x86/ARM (sequencer's architecture)
- **Speed**: ~1000x faster than RISC-V simulation
- **Optimization**: Compiler optimizations (-O3, LTO, etc.)
- **Database Access**: Direct database queries for storage
- **RPC Access**: Can make external RPC calls
- **Validation**: Full transaction validation (signature, nonce, balance)
- **Output**: `BlockOutput` with full results

**Entry Point**: `run_block()` in `api/src/lib.rs`

```rust
pub fn run_block<T, PS, TS, TR>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError>
```

**Use Cases:**
- **Sequencer**: Process transactions in real-time
- **RPC node**: Serve eth_call, eth_estimateGas
- **Development**: Fast iteration during development
- **Testing**: Unit tests with native code

**Advantages:**
- Fast (milliseconds per block)
- Can access external data sources
- Easy to debug with standard tools (gdb, perf)
- No cycle limits

**Disadvantages:**
- Not provable (native execution)
- Non-deterministic (external calls, timing, etc.)
- Architecture-dependent

### Proof Execution (RISC-V)

**Purpose**: Generate verifiable proofs of execution

**Characteristics:**
- **Target**: RISC-V RV32I (bare-metal)
- **Speed**: ~1000x slower than native
- **Determinism**: Fully deterministic (no external calls)
- **Oracle-Only I/O**: All data through CSR oracle
- **No Validation**: Assumes valid transactions (validated off-chain)
- **Output**: Witness stream (`Vec<u32>`) + public input

**Entry Point**: `run_block_generate_witness()` in `api/src/lib.rs`

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

**Use Cases:**
- **Prover**: Generate zkSNARK proofs for L1 submission
- **Verification**: Verify proof correctness locally
- **Auditing**: Reproduce execution deterministically

**Advantages:**
- Provable (zkSNARK verification on L1)
- Deterministic (same inputs → same outputs)
- Architecture-independent (RISC-V standard)
- Verifiable by anyone

**Disadvantages:**
- Slow (seconds to minutes per block)
- No external data access (oracle only)
- Limited by cycle budget
- Requires witness generation before proving

### Comparison Table

| Feature | Forward (Native) | Proof (RISC-V) |
|---------|------------------|----------------|
| **Target** | x86/ARM | RISC-V RV32I |
| **Speed** | ~10ms per block | ~10s per block (witness gen) |
| **Proving Time** | N/A | ~1-10 minutes (zkSNARK) |
| **Determinism** | No | Yes |
| **Database Access** | Yes | No (oracle only) |
| **RPC Access** | Yes | No |
| **TX Validation** | Full (sig, nonce, balance) | None (assumes valid) |
| **Output** | BlockOutput (detailed) | Witness + public input |
| **Debuggable** | gdb, perf, lldb | RISC-V simulator |
| **Cycle Limit** | None | ~68 billion |
| **Optimization** | -O3, LTO, PGO | -O3 (limited) |
| **Memory** | Host RAM (GBs) | 4GB address space |
| **Use Case** | Sequencer, RPC | Prover, verification |

### Execution Flow Comparison

**Forward Execution:**

```
API: run_block()
     ↓
ForwardSystem::run_block()
     ↓
Create System with FullIO
     ↓
Bootloader:
  ├─ Query TXs from TxSource
  ├─ Validate signatures, nonces
  ├─ Check balances
  ├─ Execute in EVM
  │  ├─ Storage reads via ReadStorageResponder (DB)
  │  └─ Storage writes via IOSubsystem
  └─ Collect results in ForwardRunningResultKeeper
     ↓
Convert to BlockOutput
     ↓
Return BlockOutput (detailed results)
```

**Proof Execution:**

```
API: run_block_generate_witness()
     ↓
Setup Oracle with processors
     ↓
Wrap with ReadWitnessSource
     ↓
Load RISC-V binary
     ↓
RISC-V Simulator:
  ├─ _start_rust() → run_proving()
  ├─ Bootloader:
  │  ├─ Query TXs from oracle (no validation)
  │  ├─ Execute in EVM
  │  │  ├─ Storage reads via oracle (recorded)
  │  │  └─ Storage writes via IOSubsystem
  │  └─ Compute public input
  └─ Exit with public input in x10-x17
     ↓
Extract witness from ReadWitnessSource
     ↓
Return Vec<u32> witness
     ↓
Feed to zkSNARK prover
     ↓
Generate proof
     ↓
Submit proof to L1
```

### When to Use Each Mode

**Use Forward Execution:**
- Real-time transaction processing (sequencer)
- RPC methods (eth_call, eth_estimateGas)
- Development and testing
- Debugging with native tools
- When proof is not required

**Use Proof Execution:**
- Generate proofs for L1 submission
- Verify block execution independently
- Audit historical blocks
- Test proof generation pipeline
- When determinism is critical

**Hybrid Approach** (common pattern):
1. Sequencer runs forward execution (fast)
2. Block finalized, results stored
3. Prover runs proof execution (slow) in background
4. Proof submitted to L1 hours later
5. L1 verifier checks proof, updates state root

---

## Compilation Process

### Building for RISC-V Target

**Toolchain**: Rust with RISC-V target support

**RISC-V Target**: `riscv32im-unknown-none-elf`
- `riscv32`: 32-bit RISC-V
- `im`: Integer + Multiply/Divide extensions
- `unknown`: No vendor
- `none`: No OS (bare-metal)
- `elf`: ELF binary format

**Install Target:**

```bash
rustup target add riscv32im-unknown-none-elf
```

**Cargo Configuration** (`zksync_os/Cargo.toml`):

```toml
[lib]
crate-type = ["staticlib"]

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"

[features]
default = []
proving = []  # Base proving features
production = ["proving"]  # Production config
for_tests = ["proving"]   # Test config with KZG
multiblock-batch = ["proving"]  # Batch proving
print_debug_info = []  # Enable logging
```

**Build Command:**

```bash
cd zksync_os

cargo build \
  --target riscv32im-unknown-none-elf \
  --release \
  --features production \
  --lib
```

**Output**: `target/riscv32im-unknown-none-elf/release/libzksync_os.a`

### dump_bin.sh Script

**Location**: `zksync_os/dump_bin.sh`

**Purpose**: Build RISC-V binary and extract executable sections

**Usage:**

```bash
./dump_bin.sh --type <TYPE>

Types:
  singleblock-batch              - Production (single block)
  singleblock-batch-logging-enabled - Production with logs
  multiblock-batch               - Multi-block batch proving
  multiblock-batch-logging-enabled  - Multi-block with logs
  for-tests                      - Test config (with KZG, P256)
  evm-replay                     - Ethereum replay mode
  evm-replay-benchmarking        - Replay with benchmarking
  evm-tester                     - EVM test suite
```

**Script Steps:**

```bash
#!/bin/sh
set -e

# Parse --type argument
TYPE="$1"

# Set features based on type
case "$TYPE" in
  singleblock-batch)
    FEATURES="proving,production"
    BIN_NAME="singleblock_batch.bin"
    ELF_NAME="singleblock_batch.elf"
    TEXT_NAME="singleblock_batch.text"
    ;;
  for-tests)
    FEATURES="proving,for_tests"
    BIN_NAME="for_tests.bin"
    ELF_NAME="for_tests.elf"
    TEXT_NAME="for_tests.text"
    ;;
  # ... other cases
esac

# Clean previous artifacts
rm -f "$BIN_NAME" "$ELF_NAME" "$TEXT_NAME"

# Build
cargo build --features "$FEATURES" --release

# Extract binary sections
cargo objcopy --features "$FEATURES" --release -- \
  -O binary "$BIN_NAME"

# Extract ELF with debug info (no .text)
cargo objcopy --features "$FEATURES" --release -- \
  -R .text "$ELF_NAME"

# Extract .text section only
cargo objcopy --features "$FEATURES" --release -- \
  -O binary --only-section=.text "$TEXT_NAME"

echo "Built [$TYPE] with features: $FEATURES"
echo "→ $BIN_NAME"
echo "→ $ELF_NAME"
echo "→ $TEXT_NAME"
```

**What is `cargo objcopy`?**
- Wrapper around LLVM's `llvm-objcopy`
- Extracts specific sections from ELF files
- `-O binary`: Convert to raw binary (no ELF headers)
- `-R .text`: Remove .text section (for symbol files)
- `--only-section=.text`: Extract only .text section

**Output Files:**

1. **`<name>.bin`**: Raw binary for simulator
   - All sections (text, data, rodata)
   - No ELF headers
   - Ready for RISC-V simulator

2. **`<name>.elf`**: Debug symbols without code
   - ELF format with symbol table
   - `.text` section removed
   - Used for profiling/diagnostics

3. **`<name>.text`**: Code section only
   - Just executable instructions
   - For analysis/inspection

### Build Profiles

**Feature Flags:**

| Feature | Purpose | Includes |
|---------|---------|----------|
| `proving` | Base proving mode | RISC-V bare-metal setup |
| `production` | Production config | P256 precompile |
| `for_tests` | Test config | P256, KZG, point_eval |
| `multiblock-batch` | Multi-block batches | Batch public input |
| `eth_runner` | Ethereum replay | Ethereum-specific features |
| `print_debug_info` | Debug logging | UART output |
| `benchmarking` | Performance testing | Cycle markers |

**Common Combinations:**

1. **Production Single Block**:
   ```bash
   ./dump_bin.sh --type singleblock-batch
   # Features: proving,production
   # Output: singleblock_batch.bin
   ```

2. **Production with Logging**:
   ```bash
   ./dump_bin.sh --type singleblock-batch-logging-enabled
   # Features: proving,production,print_debug_info
   # Output: singleblock_batch_logging_enabled.bin
   ```

3. **Multi-Block Batch**:
   ```bash
   ./dump_bin.sh --type multiblock-batch
   # Features: proving,production,multiblock-batch
   # Output: multiblock_batch.bin
   ```

4. **Test Configuration**:
   ```bash
   ./dump_bin.sh --type for-tests
   # Features: proving,for_tests
   # Output: for_tests.bin
   # Includes: KZG, P256, point_eval precompiles
   ```

**Feature Impact:**

- **Code Size**: Production ~2MB, for-tests ~3MB (due to extra precompiles)
- **Cycle Count**: Minimal (precompiles delegated to oracle)
- **Compatibility**: for-tests needed for EIP-4844 testing

### Output: RISC-V ELF Binary

**ELF Structure:**

```
ELF Header:
  Class:      ELF32
  Data:       Little-endian
  Machine:    RISC-V
  Entry:      0x00000000

Program Headers:
  Type      Offset    VirtAddr   PhysAddr   FileSiz   MemSiz    Flg Align
  LOAD      0x001000  0x00000000 0x00000000 0x180000  0x180000  R E 0x1000  (.text)
  LOAD      0x181000  0x00180000 0x00180000 0x020000  0x020000  R   0x1000  (.rodata)
  LOAD      0x1a1000  0x00200000 0x001a0000 0x010000  0x010000  RW  0x1000  (.data)
  LOAD      0x1b1000  0x00210000 0x00210000 0x000000  0x100000  RW  0x1000  (.bss, heap, stack)

Section Headers:
  [  0] .text      PROGBITS  0x00000000  0x001000  0x180000  AX
  [  1] .rodata    PROGBITS  0x00180000  0x181000  0x020000  A
  [  2] .data      PROGBITS  0x00200000  0x1a1000  0x010000  WA
  [  3] .bss       NOBITS    0x00210000  0x1b1000  0x100000  WA
  [  4] .symtab    SYMTAB    ...
  [  5] .strtab    STRTAB    ...
```

**Binary Inspection:**

```bash
# View ELF header
readelf -h singleblock_batch.bin

# View program headers
readelf -l singleblock_batch.bin

# View section headers
readelf -S singleblock_batch.bin

# Disassemble .text section
riscv32-unknown-elf-objdump -d singleblock_batch.bin | less

# View symbols
riscv32-unknown-elf-nm singleblock_batch.elf
```

**Size Analysis:**

```bash
$ ls -lh singleblock_batch.*
-rw-r--r-- 1 user user 2.1M singleblock_batch.bin   # Full binary
-rw-r--r-- 1 user user 1.2M singleblock_batch.elf   # Symbols only
-rw-r--r-- 1 user user 1.5M singleblock_batch.text  # Code only
```

**Typical Breakdown:**
- `.text` (code): ~1.5MB (70%)
- `.rodata` (constants): ~400KB (20%)
- `.data` (initialized data): ~200KB (10%)
- `.bss` (zero-initialized): 0 bytes (allocated at runtime)

---

## Proof Data

### ProofData Structure

**Location**: `zk_ee/src/common_structs/proof_data.rs`

**Purpose**: Validate execution inputs against previous chain state

```rust
#[derive(Clone, Copy, Debug)]
pub struct ProofData<SR: StateRootView<EthereumIOTypesConfig>> {
    /// State root before this block
    pub state_root_view: SR,

    /// Timestamp of previous block
    pub last_block_timestamp: u64,
}
```

**StateRootView Trait:**

```rust
trait StateRootView {
    /// Validate storage read against state root
    fn validate_read(
        &self,
        key: StorageKey,
        value: StorageValue,
        proof: MerkleProof,
    ) -> Result<(), ProofError>;

    /// Apply storage write and update state root
    fn apply_write(
        &mut self,
        key: StorageKey,
        value: StorageValue,
    );

    /// Get current state root hash
    fn state_root(&self) -> H256;
}
```

**Validation During Execution:**

```rust
// Bootloader validates proof data at start
let proof_data = query_oracle::<ProofData<_>>(ZK_PROOF_DATA_QUERY_ID);

// Validate timestamp monotonicity
if block_metadata.timestamp <= proof_data.last_block_timestamp {
    return Err("Block timestamp must be > previous timestamp");
}

// On storage reads, validate against state root
let value = storage_read(key);
proof_data.state_root_view.validate_read(key, value, merkle_proof)?;

// On storage writes, update state root view
storage_write(key, value);
proof_data.state_root_view.apply_write(key, value);

// At end, compute new state root
let new_state_root = proof_data.state_root_view.state_root();
```

### Previous Block Proof

**Purpose**: Chain proofs together for sequential verification

**Multi-Block Proving** (with `multiblock-batch` feature):

```rust
let mut batch_pi_builder = BatchPublicInputBuilder::new();

for block in blocks {
    let (io, block_metadata, current_block_hash, upgrade_tx_hash) =
        run_proving_for_block(oracle, block);

    // Accumulate block into batch public input
    oracle = io.apply_to_batch(
        block_metadata,
        current_block_hash,
        upgrade_tx_hash,
        &mut batch_pi_builder,
    );
}

// Final batch public input
let batch_public_input = batch_pi_builder.into_public_input(logger, &mut oracle);
let batch_hash = batch_public_input.hash();
```

**Batch Public Input:**
- Previous batch proof (or genesis)
- Block hashes for all blocks in batch
- State root transitions
- Upgrade transaction hashes (if any)

**Verification on L1:**

```solidity
// L1 verifier contract
function verifyBatch(
    bytes32 previousBatchHash,
    bytes32[] blockHashes,
    bytes32 newStateRoot,
    bytes proof
) external {
    // 1. Verify previous batch hash matches stored value
    require(previousBatchHash == lastVerifiedBatch);

    // 2. Verify zkSNARK proof
    require(zkVerifier.verify(proof, publicInput));

    // 3. Update state
    lastVerifiedBatch = currentBatchHash;
    stateRoot = newStateRoot;

    // 4. Emit event
    emit BatchVerified(currentBatchHash, newStateRoot);
}
```

### Batch Proof Generation

**Single Block Batch** (default):

```rust
// Generate witness for single block
let witness = run_block_generate_witness(
    block_context,
    tree,
    preimage_source,
    tx_source,
    proof_data,
    da_commitment_scheme,
    "zksync_os/singleblock_batch.bin",
)?;

// Generate proof
let proof = zksnark_prover::prove(witness)?;

// Submit to L1
l1_contract.verify_batch(
    previous_batch_hash,
    vec![block_hash],
    new_state_root,
    proof,
)?;
```

**Multi-Block Batch** (with `multiblock-batch` feature):

```rust
// Setup oracle with all blocks
let mut oracle = setup_oracle_with_blocks(blocks);

// Run RISC-V binary with multiblock-batch feature
let witness = run_multiblock_witness(
    oracle,
    "zksync_os/multiblock_batch.bin",
)?;

// Generate proof for entire batch
let proof = zksnark_prover::prove(witness)?;

// Submit batch to L1
l1_contract.verify_batch(
    previous_batch_hash,
    block_hashes,  // All block hashes
    new_state_root,
    proof,
)?;
```

**Benefits of Batch Proving:**
- **Amortized L1 Cost**: One proof verification for multiple blocks
- **Faster Finality**: Prove multiple blocks together
- **Parallelization**: Generate proofs for different batches in parallel

### DA Commitment Schemes

**Location**: `zk_ee/src/common_structs/da_commitment_scheme.rs`

**Purpose**: Define how data availability commitments are generated

```rust
pub enum DACommitmentScheme {
    /// No DA commitment
    None = 0,

    /// Empty (no DA required)
    EmptyNoDA = 1,

    /// Keccak256 hash commitment
    PubdataKeccak256 = 2,

    /// EIP-4844 blobs + Keccak256 fallback
    BlobsAndPubdataKeccak256 = 3,

    /// EIP-4844 blobs with KZG commitments (zkSync OS native)
    BlobsZKsyncOS = 4,
}
```

**DA Commitment Generation:**

```rust
fn compute_da_commitment(
    pubdata: &[u8],
    scheme: DACommitmentScheme,
) -> Result<Commitment, DAError> {
    match scheme {
        DACommitmentScheme::None => {
            Ok(Commitment::None)
        }

        DACommitmentScheme::EmptyNoDA => {
            assert!(pubdata.is_empty(), "Expected empty pubdata");
            Ok(Commitment::Empty)
        }

        DACommitmentScheme::PubdataKeccak256 => {
            let hash = keccak256(pubdata);
            Ok(Commitment::Hash(hash))
        }

        DACommitmentScheme::BlobsAndPubdataKeccak256 => {
            if pubdata.len() <= MAX_BLOB_SIZE {
                // Use blob
                let commitment = kzg_commit_to_blob(pubdata)?;
                Ok(Commitment::BlobKZG(commitment))
            } else {
                // Fallback to keccak256
                let hash = keccak256(pubdata);
                Ok(Commitment::Hash(hash))
            }
        }

        DACommitmentScheme::BlobsZKsyncOS => {
            // Always use EIP-4844 blobs with KZG
            assert!(pubdata.len() <= MAX_BLOB_SIZE);
            let commitment = kzg_commit_to_blob(pubdata)?;
            Ok(Commitment::BlobKZG(commitment))
        }
    }
}
```

**EIP-4844 Blob Commitments:**
- **Blob Size**: Up to 128KB per blob
- **KZG Commitment**: Cryptographic commitment to blob data
- **Point Evaluation**: Verify commitment without full blob
- **L1 Storage**: Blobs stored temporarily on L1 (~1 month)
- **Cost**: ~10x cheaper than CALLDATA

**Pubdata Packing:**

```rust
struct Pubdata {
    // Storage writes (key-value pairs)
    storage_diffs: Vec<(StorageKey, StorageValue)>,

    // Published bytecode
    bytecode: Vec<(BytecodeHash, Vec<u8>)>,

    // L2→L1 messages
    l2_to_l1_messages: Vec<L2ToL1Message>,

    // Compressed for DA efficiency
}

fn pack_pubdata(block_output: &BlockOutput) -> Vec<u8> {
    let mut pubdata = Vec::new();

    // Pack storage diffs
    for (key, value) in &block_output.storage_writes {
        pubdata.extend(compress_storage_diff(key, value));
    }

    // Pack bytecode
    for (hash, code) in &block_output.published_preimages {
        pubdata.extend(compress_bytecode(hash, code));
    }

    // Pack messages
    for msg in &block_output.l2_to_l1_logs {
        pubdata.extend(pack_message(msg));
    }

    pubdata
}
```

**DA Commitment in Public Input:**

```rust
struct PublicInput {
    previous_state_root: H256,
    new_state_root: H256,
    block_hash: H256,
    da_commitment: Commitment,  // Included in public input
}

// L1 verifier checks:
// 1. Proof is valid for public input
// 2. DA commitment matches published data (blob or calldata)
// 3. State transition is correct
```

---

## Integration with Prover

### Witness → zkSNARK Prover (airbender)

**Prover Pipeline:**

```
Witness (Vec<u32>)
     ↓
┌────────────────────────────────────┐
│   1. CIRCUIT GENERATION             │
│   • Convert witness to constraints │
│   • Build R1CS (Rank-1 Constraint  │
│     System) representation         │
└────────────────┬───────────────────┘
                 ↓
┌────────────────────────────────────┐
│   2. WITNESS ASSIGNMENT             │
│   • Assign witness values to       │
│     circuit variables              │
│   • Validate all constraints       │
└────────────────┬───────────────────┘
                 ↓
┌────────────────────────────────────┐
│   3. PROVING KEY LOADING            │
│   • Load pre-computed proving key  │
│   • Large file (~GB)               │
└────────────────┬───────────────────┘
                 ↓
┌────────────────────────────────────┐
│   4. PROOF GENERATION               │
│   • Compute zkSNARK proof          │
│   • Groth16 or PLONK protocol     │
│   • GPU accelerated (optional)     │
│   • Time: 1-10 minutes             │
└────────────────┬───────────────────┘
                 ↓
zkSNARK Proof (~200KB)
```

**Airbender Integration:**

```rust
use zksync_airbender::prover::{Prover, ProvingKey};

// 1. Load proving key (one-time setup)
let proving_key = ProvingKey::load("proving_key.bin")?;

// 2. Generate witness
let witness = run_block_generate_witness(/* ... */)?;

// 3. Create prover
let prover = Prover::new(proving_key);

// 4. Generate proof
let proof = prover.prove(&witness)?;

// 5. Serialize proof for L1 submission
let proof_bytes = bincode::serialize(&proof)?;

println!("Proof size: {} bytes", proof_bytes.len());
```

**RISC-V Circuit Constraints:**

Each RISC-V cycle generates constraints for:
- **Instruction Decode**: Validate opcode
- **Register Access**: Read/write registers
- **ALU Operation**: Arithmetic/logic
- **Memory Access**: Load/store with address validation
- **CSR Access**: Oracle reads/writes
- **PC Update**: Program counter increment/jump

**Constraint Count:**
- Per cycle: ~100-1000 constraints (depending on instruction)
- Per block: ~10 million - 10 billion constraints
- Circuit size: Proportional to cycle count

### Proof Verification

**Verification on L1:**

```solidity
// L1 verifier contract
contract ZKVerifier {
    // Verification key (public, immutable)
    VerifyingKey public verifyingKey;

    function verify(
        bytes calldata proof,
        bytes calldata publicInput
    ) external view returns (bool) {
        // 1. Parse proof
        (uint256[2] memory a, uint256[2][2] memory b, uint256[2] memory c)
            = parseProof(proof);

        // 2. Parse public input
        uint256[] memory publicSignals = parsePublicInput(publicInput);

        // 3. Verify proof (pairing check)
        bool valid = verifyingKey.verify(a, b, c, publicSignals);

        return valid;
    }

    // Pairing check (Groth16):
    // e(a, b) == e(alpha, beta) * e(L, gamma) * e(c, delta)
}
```

**Local Verification** (before L1 submission):

```rust
use zksync_airbender::verifier::{Verifier, VerifyingKey};

// 1. Load verifying key (public)
let verifying_key = VerifyingKey::load("verifying_key.bin")?;

// 2. Create verifier
let verifier = Verifier::new(verifying_key);

// 3. Verify proof locally
let valid = verifier.verify(&proof, &public_input)?;

if valid {
    println!("Proof verified successfully!");
    submit_to_l1(proof, public_input)?;
} else {
    println!("Proof verification failed!");
    return Err("Invalid proof");
}
```

**Verification Cost on L1:**
- **Gas Cost**: ~300,000 gas (for Groth16)
- **Time**: ~0.5 seconds (block time)
- **Proof Size**: ~200KB (regardless of block size)

**Proof Structure** (Groth16):

```rust
pub struct Proof {
    a: G1Point,        // 64 bytes
    b: G2Point,        // 128 bytes
    c: G1Point,        // 64 bytes
    // Total: 256 bytes
}

pub struct PublicInput {
    previous_state_root: H256,   // 32 bytes
    new_state_root: H256,        // 32 bytes
    block_hash: H256,            // 32 bytes
    da_commitment: H256,         // 32 bytes
    // Total: 128 bytes
}

// Full proof submission: ~400 bytes
```

### L1 Submission

**Submission Flow:**

```
Prover
  ↓ Generate proof
Proof (~200KB)
  ↓ Submit transaction
L1 Mempool
  ↓ Mine transaction
L1 Verifier Contract
  ↓ verify() call
Pairing Check
  ↓ Success
State Root Update
  ↓ Emit event
Block Finalized
```

**Submission Code:**

```rust
use ethers::prelude::*;

async fn submit_proof_to_l1(
    proof: Proof,
    public_input: PublicInput,
    l1_rpc_url: &str,
    private_key: &str,
) -> Result<H256, Box<dyn std::error::Error>> {
    // 1. Connect to L1
    let provider = Provider::<Http>::try_from(l1_rpc_url)?;
    let wallet = private_key.parse::<LocalWallet>()?;
    let client = SignerMiddleware::new(provider, wallet);

    // 2. Load contract
    let contract = ZKVerifier::new(
        "0x123...verifier_address".parse()?,
        Arc::new(client),
    );

    // 3. Encode proof and public input
    let proof_bytes = bincode::serialize(&proof)?;
    let public_input_bytes = bincode::serialize(&public_input)?;

    // 4. Submit transaction
    let tx = contract
        .verify_batch(
            public_input.previous_state_root,
            vec![public_input.block_hash],
            public_input.new_state_root,
            Bytes::from(proof_bytes),
        )
        .send()
        .await?;

    // 5. Wait for confirmation
    let receipt = tx.await?;

    println!("Proof submitted! TX: {:?}", receipt.transaction_hash);
    Ok(receipt.transaction_hash)
}
```

**Error Handling:**

```rust
match submit_proof_to_l1(proof, public_input, l1_rpc_url, private_key).await {
    Ok(tx_hash) => {
        println!("✓ Proof verified and submitted: {}", tx_hash);
    }
    Err(e) => {
        eprintln!("✗ Proof submission failed: {}", e);

        // Retry logic
        if is_retryable_error(&e) {
            println!("Retrying in 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
            submit_proof_to_l1(proof, public_input, l1_rpc_url, private_key).await?;
        } else {
            return Err(e);
        }
    }
}
```

**Monitoring:**

```rust
// Listen for verification events
let events = contract
    .batch_verified_filter()
    .from_block(BlockNumber::Latest)
    .subscribe()
    .await?;

while let Some(event) = events.next().await {
    println!("Batch verified:");
    println!("  Batch hash: {:?}", event.batch_hash);
    println!("  New state root: {:?}", event.new_state_root);
    println!("  Block number: {}", event.block_number);
}
```

---

## Diagrams

### Proof Generation Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│                   INPUT PREPARATION                          │
│                                                              │
│  Block Context    Storage Tree    Transactions              │
│       ↓                ↓                ↓                    │
│  BlockMetadata   InMemoryTree    TxListSource               │
│                                                              │
│  Preimages        Proof Data      DA Scheme                 │
│       ↓                ↓                ↓                    │
│  PreimageSource  ProofData<SR>  DACommitmentScheme          │
└───────────────────────┬─────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              ORACLE CONFIGURATION                            │
│                                                              │
│  ZkEENonDeterminismSource:                                  │
│  ├─ BlockMetadataResponder                                  │
│  ├─ TxDataResponder                                         │
│  ├─ ReadTreeResponder                                       │
│  ├─ ReadStorageResponder                                    │
│  ├─ GenericPreimageResponder                                │
│  ├─ ZKProofDataResponder                                    │
│  ├─ DACommitmentSchemeResponder                             │
│  ├─ ArithmeticQuery (callable oracle)                       │
│  └─ BlobKZGCommitmentQuery (callable oracle)                │
│                                                              │
│  Wrap with ReadWitnessSource (records CSR reads)            │
└───────────────────────┬─────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              RISC-V COMPILATION                              │
│                                                              │
│  Source: zksync_os/src/main.rs                              │
│       ↓                                                      │
│  cargo build --target riscv32im-unknown-none-elf            │
│       ↓                                                      │
│  dump_bin.sh --type for-tests                               │
│       ↓                                                      │
│  Output: for_tests.bin (~2-3MB)                             │
└───────────────────────┬─────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              WITNESS GENERATION (Simulation)                 │
│                                                              │
│  risc_v_simulator::run(                                     │
│    binary: "for_tests.bin",                                 │
│    oracle: ReadWitnessSource,                               │
│    cycles: 1 << 36,                                         │
│  )                                                           │
│                                                              │
│  Execution:                                                  │
│  ├─ _start_rust()                                           │
│  ├─ init_allocator(heap_start, heap_end)                    │
│  ├─ run_proving::<CSRBasedNonDeterminismSource>()           │
│  │   ├─ Query block metadata                                │
│  │   ├─ For each transaction:                               │
│  │   │   ├─ Query TX size                                   │
│  │   │   ├─ Query TX data                                   │
│  │   │   ├─ Execute in EVM                                  │
│  │   │   │   ├─ Opcode execution                            │
│  │   │   │   ├─ Storage reads (via CSR oracle)              │
│  │   │   │   └─ Storage writes                              │
│  │   │   └─ Collect results                                 │
│  │   └─ Compute public input                                │
│  └─ zksync_os_finish_success(output)                        │
│                                                              │
│  All CSR reads recorded by ReadWitnessSource                │
└───────────────────────┬─────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              WITNESS EXTRACTION                              │
│                                                              │
│  witness = read_witness_source.get_read_items()             │
│                                                              │
│  Format: Vec<u32> (flat stream)                             │
│  Size: ~1-50MB depending on block complexity                │
│  Contains: All oracle query results (deterministic inputs)  │
└───────────────────────┬─────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              ZKSNARK PROVING                                 │
│                                                              │
│  airbender::prover::prove(                                  │
│    witness: Vec<u32>,                                       │
│    proving_key: ProvingKey,                                 │
│  )                                                           │
│                                                              │
│  Steps:                                                      │
│  ├─ Convert witness to circuit constraints                  │
│  ├─ Validate all constraints satisfied                      │
│  ├─ Generate zkSNARK proof (Groth16/PLONK)                 │
│  └─ Output: Proof (~200KB)                                  │
│                                                              │
│  Time: 1-10 minutes (GPU accelerated)                       │
└───────────────────────┬─────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              VERIFICATION & SUBMISSION                       │
│                                                              │
│  Local verification (optional):                             │
│  └─ airbender::verifier::verify(proof, public_input)        │
│                                                              │
│  L1 submission:                                             │
│  └─ ZKVerifier.verify_batch(                                │
│      previous_batch_hash,                                   │
│      block_hashes,                                          │
│      new_state_root,                                        │
│      proof                                                   │
│    )                                                         │
│                                                              │
│  L1 Verification:                                           │
│  ├─ Pairing check (cryptographic verification)              │
│  ├─ Validate public input                                   │
│  ├─ Update state root                                       │
│  └─ Emit BatchVerified event                                │
└─────────────────────────────────────────────────────────────┘
                        │
                        ▼
                Block Finalized on L1
```

### CSR Communication Flow

```
┌──────────────────────────────────────────────────────────────┐
│              RISC-V CODE (Guest Environment)                  │
│                                                               │
│  fn execute_transaction() {                                  │
│    // Need to read storage                                   │
│    let value = storage_read(address, key);                   │
│  }                                                            │
│                                                               │
│  fn storage_read(address: Address, key: U256) -> U256 {      │
│    // 1. Derive flat storage key                             │
│    let flat_key = keccak256(address || key);                 │
│    let flat_key_words: [u32; 8] = flat_key.to_words();       │
└────────────────────┬──────────────────────────────────────────┘
                     │
                     ▼
    // 2. Write query ID to CSR
    csr_write(STORAGE_READ_QUERY_ID);  // 0x4000
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              SIMULATOR (Host Environment)                       │
│                                                                 │
│  CSR write detected → Oracle buffering                         │
│  query_buffer = QueryBuffer {                                  │
│    query_type: 0x4000,                                         │
│    remaining_len: None,                                        │
│    buffer: vec![],                                             │
│  }                                                              │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              RISC-V CODE                                        │
│                                                                 │
│    // 3. Write parameter count                                 │
│    csr_write(8);  // 8 u32 words for flat_key                 │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              SIMULATOR                                          │
│                                                                 │
│  query_buffer.remaining_len = Some(8);                         │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              RISC-V CODE                                        │
│                                                                 │
│    // 4. Write parameters                                      │
│    for word in flat_key_words {                                │
│      csr_write(word);                                          │
│    }                                                            │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              SIMULATOR                                          │
│                                                                 │
│  for each write:                                               │
│    query_buffer.buffer.push(word);                             │
│    query_buffer.remaining_len -= 1;                            │
│                                                                 │
│  when remaining_len == 0:                                      │
│    // Process query                                            │
│    let processor_id = ranges.get(0x4000);  // ReadStorageResp │
│    let processor = processors[processor_id];                   │
│    let results = processor.process_buffered_query(             │
│      0x4000,                                                    │
│      [flat_key_word0, ..., flat_key_word7]                     │
│    );                                                           │
│                                                                 │
│    // ReadStorageResponder:                                    │
│    let value = storage_tree.get(&flat_key)?;                   │
│    let value_words: [u32; 8] = value.to_words();               │
│    return Box::new(value_words.into_iter());                   │
│                                                                 │
│    // Prepare for reading                                      │
│    current_iterator = Some(results);                           │
│    iterator_len_to_indicate = Some(8 * 2);  // 8 usize → 16 u32│
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              RISC-V CODE                                        │
│                                                                 │
│    // 5. Read result length                                    │
│    let result_len = csr_read();  // Returns 16 (8 u64 → 16 u32)│
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              SIMULATOR                                          │
│                                                                 │
│  return iterator_len_to_indicate.take();  // 16                │
│                                                                 │
│  [ReadWitnessSource records: 16]                               │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              RISC-V CODE                                        │
│                                                                 │
│    // 6. Read result words                                     │
│    let mut result_words = Vec::with_capacity(result_len);      │
│    for _ in 0..result_len {                                    │
│      let word = csr_read();                                    │
│      result_words.push(word);                                  │
│    }                                                            │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              SIMULATOR                                          │
│                                                                 │
│  for each read:                                                │
│    let next_usize = current_iterator.next();  // u64 on host  │
│                                                                 │
│    // Split u64 into two u32 reads                             │
│    if reading_low_half:                                        │
│      high_half = Some((next_usize >> 32) as u32);              │
│      return (next_usize & 0xFFFFFFFF) as u32;                  │
│    else:                                                        │
│      return high_half.take();                                  │
│                                                                 │
│  [ReadWitnessSource records each u32 read]                     │
│  [Witness: 16, word0, word1, ..., word15]                      │
└────────────────────┬────────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│              RISC-V CODE                                        │
│                                                                 │
│    // 7. Reconstruct value from words                          │
│    let value = U256::from_words(&result_words[8..16]);         │
│    return value;                                               │
│  }                                                              │
│                                                                 │
│  // Continue execution with storage value                      │
│  interpreter.stack.push(value);                                │
└────────────────────────────────────────────────────────────────┘
```

### Forward vs Proof Comparison

```
┌─────────────────────────────────────────────────────────────┐
│                    INPUT: Block Data                         │
│  • Block metadata, transactions, storage state              │
└────────────┬────────────────────────────────┬────────────────┘
             │                                │
             ▼                                ▼
┌───────────────────────────┐   ┌───────────────────────────┐
│    FORWARD EXECUTION       │   │     PROOF EXECUTION        │
│      (Native x86/ARM)      │   │      (RISC-V Bare-Metal)   │
└───────────────────────────┘   └───────────────────────────┘
             │                                │
             ▼                                ▼
┌───────────────────────────┐   ┌───────────────────────────┐
│  • Speed: ~10ms/block     │   │  • Speed: ~10s/block      │
│  • Environment: OS        │   │  • Environment: no OS     │
│  • Determinism: No        │   │  • Determinism: Yes       │
│  • Database: Direct SQL   │   │  • Oracle: CSR only       │
│  • Validation: Full       │   │  • Validation: None       │
│  • Output: BlockOutput    │   │  • Output: Witness        │
└───────────────────────────┘   └───────────────────────────┘
             │                                │
             ▼                                ▼
┌───────────────────────────┐   ┌───────────────────────────┐
│    Entry Point:            │   │    Entry Point:            │
│    run_block()             │   │    run_block_generate_     │
│                            │   │      witness()             │
│    ↓                       │   │                            │
│  ForwardSystem::           │   │    ↓                       │
│    run_block()             │   │  Setup Oracle              │
│                            │   │  ├─ Metadata responder     │
│    ↓                       │   │  ├─ TX responder           │
│  Setup System              │   │  ├─ Storage responder      │
│  ├─ FullIO                 │   │  └─ Preimage responder     │
│  ├─ ForwardResultKeeper    │   │                            │
│  └─ Database-backed        │   │    ↓                       │
│      storage               │   │  Wrap with                 │
│                            │   │    ReadWitnessSource       │
│    ↓                       │   │                            │
│  Bootloader                │   │    ↓                       │
│  ├─ Query TXs from         │   │  Load RISC-V binary        │
│  │   TxSource              │   │                            │
│  ├─ Validate signatures    │   │    ↓                       │
│  ├─ Check nonces           │   │  RISC-V Simulator          │
│  ├─ Check balances         │   │  ├─ _start_rust()          │
│  ├─ Execute in EVM         │   │  ├─ init_allocator()       │
│  │  ├─ Storage via         │   │  ├─ run_proving()          │
│  │  │   ReadStorage        │   │  │  ├─ Query TXs from      │
│  │  │   Responder (DB)     │   │  │  │   oracle             │
│  │  └─ Fast native code    │   │  │  ├─ NO validation       │
│  └─ Collect in             │   │  │  ├─ Execute in EVM      │
│      ResultKeeper          │   │  │  │  ├─ Storage via      │
│                            │   │  │  │  │   oracle (CSR)     │
│    ↓                       │   │  │  │  └─ Slow RISC-V      │
│  Convert to BlockOutput    │   │  │  └─ Compute public      │
│  ├─ TxResults              │   │  │      input              │
│  ├─ StorageWrites          │   │  └─ finish_success()       │
│  ├─ Events                 │   │                            │
│  └─ Logs                   │   │    ↓                       │
│                            │   │  Extract Witness           │
│    ↓                       │   │  └─ Vec<u32> stream        │
│  Return BlockOutput        │   │                            │
│                            │   │    ↓                       │
│  Use Cases:                │   │  Return Witness            │
│  • Sequencer processing    │   │                            │
│  • RPC eth_call            │   │    ↓                       │
│  • Development testing     │   │  zkSNARK Prover            │
│  • Gas estimation          │   │  ├─ Circuit generation     │
│                            │   │  ├─ Witness assignment     │
└───────────────────────────┘   │  ├─ Proof computation      │
                                 │  └─ Output: Proof          │
                                 │                            │
                                 │    ↓                       │
                                 │  L1 Submission             │
                                 │  └─ verify_batch()         │
                                 │                            │
                                 │  Use Cases:                │
                                 │  • Proof generation        │
                                 │  • L1 verification         │
                                 │  • Auditing                │
                                 │                            │
                                 └───────────────────────────┘
```

---

## Summary

The zkSync OS proof system enables trustless L2 execution through RISC-V-based zero-knowledge proofs:

**Key Components:**
1. **RISC-V Execution**: Bare-metal environment with Talc allocator and CSR-based oracle
2. **Witness Generation**: Record all oracle queries during simulation for proving
3. **Oracle Communication**: CSR protocol streams data between RISC-V and host
4. **Proof Generation**: zkSNARK prover converts witness to succinct proof
5. **L1 Verification**: On-chain proof verification updates state root

**Workflow:**
1. Compile zkSync OS to RISC-V binary (`dump_bin.sh`)
2. Run RISC-V simulator with witness-recording oracle
3. Extract witness stream (`Vec<u32>`)
4. Feed witness to zkSNARK prover (airbender)
5. Submit proof to L1 verifier contract
6. L1 updates state root after verification

**Benefits:**
- **Trustless**: Anyone can verify proofs without re-executing
- **Succinct**: ~200KB proofs regardless of block complexity
- **Deterministic**: RISC-V ensures reproducible execution
- **Efficient**: Batch multiple blocks per proof

**For More Details:**
- Data flow → [DataFlow.md](DataFlow.md)
- API entry points → [API.md](API.md)
- RISC-V execution → [Architecture.md](Architecture.md)
- Storage access → [Storage.md](Storage.md)
