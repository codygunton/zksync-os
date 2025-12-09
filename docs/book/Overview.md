# zkSync OS Overview

## What is zkSync OS?

zkSync OS is a next-generation state transition function implementation for zkSync Layer 2 blockchain that combines EVM compatibility with zero-knowledge proof generation. It represents a fundamental reimagining of how blockchain execution systems can be built to achieve both performance and provability.

### Core Purpose

zkSync OS serves as the **execution layer** for zkSync's Layer 2 scaling solution. It takes blocks of transactions as input, executes them according to EVM semantics, and produces:
- A new blockchain state (storage updates, account changes)
- Transaction execution results (success/failure, gas usage, events)
- A cryptographic witness that can be proven using zero-knowledge proofs

### Architecture Philosophy

The system is designed around a unique dual-execution model:

1. **Forward Running Mode**: Native x86/ARM execution optimized for speed, used by sequencers to process transactions quickly and provide immediate user feedback
2. **Proof Running Mode**: RISC-V bare-metal execution that generates deterministic witnesses for zero-knowledge proof generation

This separation allows zkSync OS to achieve both high throughput for users and efficient provability for Layer 1 settlement.

### Key Design Goals

- **Ultra-Low Transaction Costs**: Target of $0.0001 per ERC20 transfer
- **High Throughput**: 10,000+ transactions per second
- **EVM Equivalence**: Full compatibility with existing Ethereum smart contracts
- **Provable Execution**: Every state transition can be proven using zkSNARKs
- **Multi-Environment Support**: Designed to support EVM, EraVM, and WebAssembly execution environments

### RISC-V Proving System

Unlike traditional EVM implementations, zkSync OS compiles to RISC-V instructions for proving. This approach provides several advantages:

- **Universal Provability**: RISC-V is a well-defined, simple instruction set that's efficient to prove
- **Compiler Ecosystem**: Leverages the mature Rust → RISC-V compilation toolchain
- **Execution Flexibility**: Can support multiple execution environments through a common proving base
- **Future-Proof**: Easy to upgrade and extend without changing the proving system

The RISC-V binary is generated from the same Rust codebase used for forward execution, ensuring consistency between sequencer behavior and proven execution.

## Quick Mental Model

Understanding zkSync OS requires grasping its data flow from input to output:

```
┌─────────────────────────────────────────────────────────────────────┐
│                           zkSync OS                                  │
│                                                                      │
│  INPUT                    PROCESSING                    OUTPUT       │
│  ═════                    ══════════                    ══════       │
│                                                                      │
│  ┌─────────────┐         ┌──────────────┐           ┌────────────┐ │
│  │ Block Info  │────────▶│              │──────────▶│ New State  │ │
│  │  • number   │         │  Bootloader  │           │  • storage │ │
│  │  • timestamp│         │              │           │  • accounts│ │
│  │  • chain_id │         └──────┬───────┘           └────────────┘ │
│  └─────────────┘                │                                   │
│                                  ▼                   ┌────────────┐ │
│  ┌─────────────┐         ┌──────────────┐           │ Tx Results │ │
│  │Transactions │────────▶│ Execution    │──────────▶│  • status  │ │
│  │  • tx data  │         │ Environments │           │  • gas used│ │
│  │  • from/to  │         │  ┌─────────┐ │           │  • logs    │ │
│  │  • calldata │         │  │   EVM   │ │           └────────────┘ │
│  └─────────────┘         │  └─────────┘ │                          │
│                          │  (EraVM/Wasm) │           ┌────────────┐ │
│  ┌─────────────┐         └──────┬───────┘           │ZK Witness  │ │
│  │ State Tree  │────────────────┤                   │  (RISC-V)  │ │
│  │  • storage  │                │                   │  • queries │ │
│  │  • accounts │         ┌──────▼───────┐           │  • results │ │
│  │  • bytecode │         │ Oracle Layer │           └────────────┘ │
│  └─────────────┘         │  • storage   │                          │
│                          │  • preimages │                          │
│                          │  • metadata  │                          │
│                          └──────────────┘                          │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Input: What Goes In

zkSync OS accepts three primary inputs:

1. **Block Context**: Metadata about the block being executed
   - Block number, timestamp, parent hash
   - Chain ID, coinbase address
   - L1 batch information for zkSync-specific features

2. **Transaction List**: A sequence of transactions to execute
   - Raw transaction bytes (RLP-encoded)
   - Signature data (v, r, s)
   - Transaction type (Legacy, EIP-2930, EIP-1559, EIP-4844)

3. **State Data**: The current blockchain state
   - Storage tree (Merkle-Patricia Trie or flat storage)
   - Account data (balances, nonces, code hashes)
   - Contract bytecode

### Processing: What Happens Inside

The bootloader orchestrates execution:

1. **Parse Transactions**: Decode and validate transaction structure
2. **Execute in EVM**: Run bytecode instruction by instruction
3. **Handle External Calls**: Manage contract-to-contract interactions via preemption
4. **Access Storage**: Read/write state through the oracle layer
5. **Emit Events**: Collect logs and events for transaction receipts
6. **Track Resources**: Account for both EVM gas and native RISC-V resources

The oracle layer acts as an intermediary between the execution environment and external data sources, providing:
- Storage reads from the state tree
- Preimage data for bytecode and account information
- Block metadata and transaction data
- Cryptographic operations (signatures, hashing, pairings)

### Output: What Comes Out

zkSync OS produces several outputs:

1. **Block Output**: Complete execution results
   - Per-transaction status (success/revert)
   - Gas used by each transaction
   - Storage writes (state deltas)
   - Events and logs emitted
   - L2→L1 messages

2. **ZK Witness** (Proof Mode Only): Execution trace for proving
   - All oracle queries made during execution
   - Query results and parameters
   - Non-deterministic inputs
   - Formatted as a stream of u32 values

3. **Updated State**: New blockchain state
   - Modified storage slots
   - Updated account balances and nonces
   - Deployed contract code

## Key Capabilities

### Multiple Execution Environments

zkSync OS is built around an **Execution Environment (EE) abstraction** that allows different virtual machines to coexist:

#### Current: EVM Interpreter
- Full EVM opcode support (through Shanghai hard fork)
- Stack, memory, and storage operations
- All standard precompiles (ecrecover, SHA256, blake2f, BN254 pairings, etc.)
- Extended precompiles (P256 signature verification, KZG point evaluation)
- Gas accounting matching Ethereum mainnet

**Code Reference**: `/home/cody/zksync-os/evm_interpreter/src/lib.rs` - The `Interpreter<'a, S>` struct (lines 81-112) implements the EVM execution environment.

#### Future: EraVM and WebAssembly
The system is designed to support:
- **EraVM**: zkSync's optimized VM for high-performance execution
- **WebAssembly**: Standard Wasm for broader language support

Each execution environment implements the `ExecutionEnvironment` trait (`/home/cody/zksync-os/zk_ee/src/system/execution_environment/mod.rs`), providing a common interface for the bootloader to orchestrate execution.

### Dual Execution Modes

zkSync OS can run in two distinct modes, sharing the same codebase:

#### Forward Running (Sequencer Mode)
- **Target**: Native x86 or ARM architecture
- **Purpose**: Fast transaction execution for sequencers
- **Optimization**: Can use system allocator, database access, network I/O
- **Output**: Block results and state updates
- **Speed**: Optimized for throughput (10,000+ TPS target)

**Code Reference**: `/home/cody/zksync-os/forward_system/src/lib.rs` - Implements optimized forward execution

#### Proof Running (RISC-V Mode)
- **Target**: RISC-V 32-bit bare-metal
- **Purpose**: Deterministic witness generation for zero-knowledge proofs
- **Constraints**: No standard library, no OS, oracle-only I/O
- **Output**: Execution witness (Vec&lt;u32&gt;) for the prover
- **Determinism**: Identical execution across all environments

**Code Reference**: `/home/cody/zksync-os/zksync_os/src/main.rs` - RISC-V entry point starting at `_start_rust()` (line 228)

The same Rust code compiles to both targets, with conditional compilation handling platform-specific differences. This ensures that forward execution and proven execution produce identical results.

### Oracle-Based External Data Access

In proof mode, zkSync OS cannot access databases, filesystems, or networks directly. Instead, it uses a **Control Status Register (CSR) based oracle protocol**:

1. **Query Issuance**: Execution writes a query ID and parameters to CSRs
2. **Query Processing**: The RISC-V simulator intercepts CSR writes
3. **Data Provision**: Oracle processors provide the requested data
4. **Result Streaming**: Results are read back via CSR reads
5. **Witness Recording**: All queries and responses are recorded for the prover

This architecture ensures:
- **Determinism**: Identical queries always return identical results
- **Provability**: All non-deterministic inputs are explicitly recorded
- **Flexibility**: Easy to add new oracle types without changing core execution

**Oracle Types**:
- `BlockMetadataResponder`: Provides block context (timestamp, number, coinbase)
- `TxDataResponder`: Streams transaction data
- `ReadTreeResponder`: Merkle proof-based storage access
- `ReadStorageResponder`: Flat storage key-value reads
- `GenericPreimageResponder`: Bytecode and account preimages
- `CallableOracles`: Expensive cryptographic operations (KZG, modular arithmetic)

### Double Resource Accounting

zkSync OS tracks resources in two parallel systems:

#### 1. EVM Gas (User-Facing)
- Standard Ethereum gas costs for opcodes
- Used for transaction gas limits and pricing
- Determines transaction inclusion and fees
- Matches Ethereum mainnet for compatibility

#### 2. Native Resources (RISC-V Cycles)
- Actual computational cost in RISC-V instructions
- Prevents denial-of-service through gas mispricing
- Used for proof generation cost estimation
- Ensures economic security of the system

**Example**: A SHA3 operation consumes both:
- **30 + 6 * word_count** EVM gas (Ethereum semantics)
- **~1000s RISC-V cycles** for actual hash computation (system protection)

Transactions must have sufficient resources in both dimensions to execute successfully. This "double accounting" prevents attacks where operations are cheap in EVM gas but expensive in real computation.

**Code Reference**: `/home/cody/zksync-os/zk_ee/src/system/resources.rs` - The `Resources` trait defines the interface for resource tracking

## Repository Structure

The zkSync OS codebase is organized into logical crates, each with a specific purpose:

### Core Execution Crates

| Crate | Purpose | Key Exports |
|-------|---------|-------------|
| **`api`** | Public Rust API | `run_block()`, `run_block_generate_witness()`, `simulate_tx()` |
| **`zksync_os`** | RISC-V binary entry point | `main()` → RISC-V bootloader |
| **`forward_system`** | Native execution implementation | Forward running system, block execution |
| **`proof_running_system`** | RISC-V execution implementation | Proof generation, witness collection |
| **`basic_bootloader`** | Transaction orchestration | Bootloader loop, transaction parsing |

### Execution Environment Crates

| Crate | Purpose | Key Exports |
|-------|---------|-------------|
| **`zk_ee`** | EE abstraction and system layer | `ExecutionEnvironment` trait, `System<S>`, `IOSubsystem` |
| **`evm_interpreter`** | EVM implementation | `Interpreter<'a, S>`, opcode execution |
| **`basic_system`** | System implementations | `FullIO`, storage models, metadata providers |

### Supporting Infrastructure

| Crate | Purpose | Key Exports |
|-------|---------|-------------|
| **`oracle_provider`** | Oracle implementations | `ReadWitnessSource`, query processors |
| **`callable_oracles`** | Expensive crypto operations | KZG, modular arithmetic, hash-to-prime |
| **`storage_models`** | State management | Flat storage, Ethereum MPT |
| **`crypto`** | Cryptographic primitives | Hash functions, elliptic curves |
| **`system_hooks`** | EVM precompiles | ecrecover, sha256, BN254, P256 |

### Tools and Testing

| Crate | Purpose | Key Exports |
|-------|---------|-------------|
| **`zksync_os_runner`** | RISC-V simulator integration | `run()` - execute RISC-V binary |
| **`tests/rig`** | Test infrastructure | Common test utilities |
| **`tests/instances/*`** | Test suites | Transaction tests, ERC20, precompiles |

For detailed information about the crate organization and architecture, see **[Architecture.md](./Architecture.md)**.

### Workspace Organization

The repository uses a Cargo workspace with clear separation:

```
zksync-os/
├── api/                    # Public API entry point
├── zksync_os/              # RISC-V binary (excluded from workspace)
├── zk_ee/                  # Execution environment abstraction
├── evm_interpreter/        # EVM implementation
├── basic_bootloader/       # Transaction bootloader
├── basic_system/           # System implementations
├── forward_system/         # Forward execution mode
├── proof_running_system/   # Proof execution mode
├── oracle_provider/        # Oracle implementations
├── callable_oracles/       # Crypto oracles
├── storage_models/         # Storage abstractions
├── crypto/                 # Cryptographic primitives
├── system_hooks/           # EVM precompiles
├── zksync_os_runner/       # RISC-V simulator
├── tests/                  # Test suites and infrastructure
└── supporting_crates/      # Utility libraries
```

## Getting Started

### Understanding the Entry Points

zkSync OS has different entry points depending on your use case:

#### For Users: Public API

The primary interface is the **`zksync_os_api` crate** (`/home/cody/zksync-os/api/src/lib.rs`):

```rust
// Execute a block natively (forward mode)
pub use forward_system::run::run_block;

// Execute a block in RISC-V and generate a witness (proof mode)
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

**See [API.md](./API.md)** for complete API documentation with usage examples.

#### For Proving: RISC-V Entry

The RISC-V binary starts at **`/home/cody/zksync-os/zksync_os/src/main.rs`**:

```
_start_rust() (line 228)
  └─▶ main() (line 222)
      └─▶ workload() (line 175)
          ├─▶ init_allocator() - Set up heap
          └─▶ run_proving::<CSRBasedNonDeterminismSource>() - Execute bootloader
              └─▶ Returns output[0..7] via registers x10-x17
```

#### For Testing: RISC-V Simulator

The **`zksync_os_runner` crate** (`/home/cody/zksync-os/zksync_os_runner/src/lib.rs`) provides:

```rust
pub fn run(
    bin_path: PathBuf,           // Path to RISC-V binary
    diagnostics_path: Option<&str>,
    cycle_limit: u64,
    oracle: impl IOOracle,
) -> [u32; 8]                    // Execution output
```

This loads the RISC-V binary into a simulator, provides oracle access, and returns execution results.

### Compilation Targets

zkSync OS must be built for two different targets:

#### 1. Native Platform (x86/ARM)
For forward execution and development:

```bash
# Build all crates
cargo build --release

# Run tests
cargo test --release -p transactions
```

#### 2. RISC-V Platform (riscv32i-unknown-none-elf)
For proof generation:

```bash
# One-time setup
rustup target add riscv32i-unknown-none-elf
cargo install cargo-binutils
rustup component add llvm-tools-preview

# Build RISC-V binary
cd zksync_os
./dump_bin.sh --type for-tests
```

This generates `zksync_os/for_tests.bin` - a RISC-V ELF binary that can be executed by the simulator and proven.

#### Build Profiles

The `dump_bin.sh` script supports different build profiles:

- **`for-tests`**: Includes all precompiles, full diagnostics
- **`production`**: Optimized for mainnet, P256 precompile only
- **`eth_runner`**: Ethereum compatibility mode

### Learning Path

For new contributors, we recommend this reading order:

1. **Start Here**: This Overview document (you are here!)
2. **System Architecture**: Read [Architecture.md](./Architecture.md) to understand crate organization
3. **Data Flow**: Study [DataFlow.md](./DataFlow.md) to see how data moves through the system
4. **API Usage**: Check [API.md](./API.md) for integration examples
5. **Deep Dive**: Explore component-specific docs:
   - [ExecutionEnvironments.md](./ExecutionEnvironments.md) - EVM implementation
   - [SystemLayer.md](./SystemLayer.md) - Core system primitives
   - [Storage.md](./Storage.md) - State management
   - [Cryptography.md](./Cryptography.md) - Precompiles and crypto
   - [ProofSystem.md](./ProofSystem.md) - RISC-V and witness generation

6. **Reference**: Use [KeyTypes.md](./KeyTypes.md) to look up important data structures

### Quick Examples

#### Running a Block (Forward Mode)

```rust
use zksync_os_api::run_block;
use forward_system::run::{BlockContext, InMemoryTree, InMemoryPreimageSource};
use zksync_os_interface::traits::TxListSource;

// Set up inputs
let block_context = BlockContext { /* ... */ };
let tree = InMemoryTree::default();
let preimage_source = InMemoryPreimageSource::default();
let tx_source = TxListSource::new(transactions);

// Execute block
let output = run_block(
    block_context,
    tree,
    preimage_source,
    tx_source,
    |tx_result| { /* handle each transaction result */ },
    &mut tracer,
)?;

println!("Block executed: {} transactions", output.transactions.len());
```

#### Generating a Witness (Proof Mode)

```rust
use zksync_os_api::run_block_generate_witness;

// Set up inputs (same as forward mode)
let block_context = BlockContext { /* ... */ };
let tree = InMemoryTree::default();
let preimage_source = InMemoryPreimageSource::default();
let tx_source = TxListSource::new(transactions);
let proof_data = ProofData { /* ... */ };
let da_commitment = DACommitmentScheme::default();

// Generate witness
let witness = run_block_generate_witness(
    block_context,
    tree,
    preimage_source,
    tx_source,
    proof_data,
    da_commitment,
    "./zksync_os/for_tests.bin",
);

println!("Witness generated: {} u32 values", witness.len());
// Pass witness to zkSNARK prover
```

#### Simulating a Transaction (eth_call / eth_estimateGas)

```rust
use zksync_os_api::simulate_tx;

// Set up call context
let call_request = CallRequest {
    from: Some(sender_address),
    to: Some(contract_address),
    data: Some(calldata),
    gas: Some(1_000_000),
    // ...
};

// Simulate without state changes
let result = simulate_tx(call_request, block_context, tree)?;

match result {
    Ok(return_data) => println!("Call succeeded: {:?}", return_data),
    Err(error) => println!("Call reverted: {:?}", error),
}
```

## Next Steps

### For Integration

If you're integrating zkSync OS into a sequencer or node:
1. Read **[API.md](./API.md)** for detailed API documentation
2. Study **[DataFlow.md](./DataFlow.md)** to understand data requirements
3. Review existing tests in `tests/instances/transactions/` for examples

### For Development

If you're contributing to zkSync OS:
1. Read **[Architecture.md](./Architecture.md)** to understand the codebase structure
2. Study **[KeyTypes.md](./KeyTypes.md)** for important data structures
3. Explore component docs for the area you're working on:
   - Working on EVM? See [ExecutionEnvironments.md](./ExecutionEnvironments.md)
   - Working on storage? See [Storage.md](./Storage.md)
   - Working on crypto? See [Cryptography.md](./Cryptography.md)
   - Working on proving? See [ProofSystem.md](./ProofSystem.md)

### For Understanding

If you want to understand how zkSync OS works internally:
1. Start with this **Overview.md** (you've finished it!)
2. Read **[DataFlow.md](./DataFlow.md)** - our most detailed document showing complete data flow
3. Pick a specific component that interests you and read its documentation

## Documentation Index

| Document | Focus | Audience |
|----------|-------|----------|
| **Overview.md** | High-level introduction (this document) | Everyone |
| **[Architecture.md](./Architecture.md)** | System structure and crate organization | Developers, contributors |
| **[DataFlow.md](./DataFlow.md)** | Complete data flow from input to output | Integrators, developers |
| **[API.md](./API.md)** | Public API reference and usage | Integrators, users |
| **[ExecutionEnvironments.md](./ExecutionEnvironments.md)** | EVM interpreter and EE abstraction | EE developers |
| **[SystemLayer.md](./SystemLayer.md)** | Core system primitives, I/O, resources | System developers |
| **[Storage.md](./Storage.md)** | Storage models and state management | Storage developers |
| **[Cryptography.md](./Cryptography.md)** | Precompiles and crypto primitives | Crypto developers |
| **[ProofSystem.md](./ProofSystem.md)** | RISC-V execution and witness generation | Prover integrators |
| **[KeyTypes.md](./KeyTypes.md)** | Critical data structures reference | All developers |
| **[Index.md](./Index.md)** | Complete index and navigation | Reference |

## Frequently Asked Questions

### Why RISC-V instead of direct EVM proving?

RISC-V provides a simple, well-defined instruction set that's efficient to prove. By compiling Rust to RISC-V, we can leverage the mature compiler ecosystem and support multiple execution environments (EVM, EraVM, Wasm) through a common proving base.

### How does zkSync OS differ from other EVM implementations?

Key differences:
- **Dual execution**: Forward mode for speed, proof mode for verification
- **Oracle architecture**: All external data access is explicit and recorded
- **Multi-environment**: Designed to support multiple VMs, not just EVM
- **Resource accounting**: Tracks both EVM gas and native resources

### Can I run zkSync OS on any blockchain?

zkSync OS is designed for zkSync Layer 2, but the core execution engine is blockchain-agnostic. With appropriate oracle implementations, it could be adapted to other chains.

### What's the performance compared to go-ethereum (Geth)?

- **Forward mode**: Competitive with Geth for most workloads, optimized for zkSync-specific features
- **Proof mode**: Slower due to RISC-V simulation, but generates witnesses for proving

### How do I add a new precompile?

1. Implement the precompile in `system_hooks/src/`
2. Register it at the appropriate address
3. Add gas costs in `evm_interpreter/src/gas/`
4. Update build profiles if conditional compilation is needed

See **[Cryptography.md](./Cryptography.md)** for detailed precompile documentation.

## Contributing

We welcome contributions! Before starting:

1. Read the [CONTRIBUTING.md](../../CONTRIBUTING.md) guide
2. Review the architecture documentation
3. Check existing issues and discussions
4. Write tests for new functionality

## Additional Resources

- **Repository**: [github.com/matter-labs/zksync-os](https://github.com/matter-labs/zksync-os)
- **zkSync Documentation**: [docs.zksync.io](https://docs.zksync.io)
- **Prover (airbender)**: [github.com/matter-labs/zksync-airbender](https://github.com/matter-labs/zksync-airbender)
- **Anvil-zkSync**: [github.com/matter-labs/anvil-zksync](https://github.com/matter-labs/anvil-zksync)
- **In-repo docs**: [/docs](../../docs/) directory

---

**Next**: Read [Architecture.md](./Architecture.md) to dive deeper into the system structure.
