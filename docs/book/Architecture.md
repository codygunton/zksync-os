# zkSync OS Architecture

## Table of Contents

- [Project Purpose & Goals](#project-purpose--goals)
- [Directory Structure](#directory-structure)
- [Major Crates](#major-crates)
- [Compilation Targets](#compilation-targets)
- [Execution Modes](#execution-modes)
- [Component Architecture](#component-architecture)
- [Build System](#build-system)

---

## Project Purpose & Goals

zkSync OS is a state transition function (STF) implementation for zkSync Layer 2, designed to enable multiple execution environments (EVM, EraVM, Wasm) to operate within a unified zero-knowledge proving system. The system is implemented in Rust and compiled to RISC-V for zero-knowledge proof generation using the zksync-airbender proving system.

### Core Objectives

**Primary Goal**: Provide a verifiable state transition function that processes blocks of transactions with EVM equivalence while generating succinct zero-knowledge proofs.

**Performance Targets**:
- **Cost**: Target of $0.0001 per ERC20 transfer
- **Throughput**: Target of 10,000 transactions per second (TPS)
- **Proof Generation**: Efficient witness collection for RISC-V-based zkSNARK proofs

**Key Capabilities**:
1. **Multi-Environment Support**: Abstract execution environment interface supporting EVM (currently), EraVM (planned), and WebAssembly (planned)
2. **Dual Execution Modes**:
   - Forward running for native sequencer execution (x86/ARM)
   - Proof running for verifiable RISC-V execution with witness generation
3. **Oracle-Based I/O**: All external data access flows through a query-based oracle system for deterministic replay
4. **Double Resource Accounting**: Tracks both EVM gas (for Ethereum compatibility) and native RISC-V resources (for proof costs)

---

## Directory Structure

The zkSync OS repository follows a modular crate organization with clear separation of concerns:

```
zksync-os/
├── Core System Crates
│   ├── zksync_os/              # RISC-V entry point and bare-metal runtime
│   ├── api/                    # High-level Rust API for block execution
│   ├── forward_system/         # Native x86/ARM execution system for sequencers
│   ├── proof_running_system/   # RISC-V bootloader and proof generation system
│   └── zksync_os_runner/       # RISC-V simulator runner for witness generation
│
├── Execution Layer
│   ├── basic_bootloader/       # Bootloader orchestration and main execution loop
│   ├── basic_system/           # Core system implementation (IO, storage, metadata)
│   ├── zk_ee/                  # Execution environment abstraction layer
│   └── evm_interpreter/        # EVM opcode interpreter and stack machine
│
├── Oracle & Data Access
│   ├── oracle_provider/        # Oracle infrastructure and query dispatching
│   ├── callable_oracles/       # Expensive crypto operations delegated to host
│   └── storage_models/         # Storage abstraction (flat model, Ethereum MPT)
│
├── Cryptography
│   ├── crypto/                 # Core cryptographic primitives (hashing, signatures)
│   └── system_hooks/           # EVM precompiles and system contract hooks
│
├── Supporting Crates
│   ├── supporting_crates/      # Utility crates (keccak, modexp, u256, abi)
│   └── cycle_marker/           # Performance profiling utilities
│
└── Testing Infrastructure
    ├── tests/rig/              # Test harness and utilities
    ├── tests/instances/        # Test suites (transactions, precompiles, ERC20, etc.)
    └── tests/fuzzer/           # Fuzzing infrastructure
```

### Crate Purposes

**Core System**:
- **`zksync_os`** (`/zksync_os/src/main.rs`): RISC-V entry point with `_start_rust()` function, bare-metal memory management, and CSR-based oracle interface
- **`api`** (`/api/src/lib.rs`): High-level API exposing `run_block()`, `run_block_generate_witness()`, and `simulate_tx()` functions
- **`forward_system`** (`/forward_system/src/lib.rs`): Native execution with direct database/network access for sequencer operation
- **`proof_running_system`** (`/proof_running_system/src/lib.rs`): RISC-V bootloader initialization and oracle-based execution for proving
- **`zksync_os_runner`** (`/zksync_os_runner/src/lib.rs`): RISC-V simulator integration for witness generation without real prover

**Execution Layer**:
- **`basic_bootloader`** (`/basic_bootloader/src/lib.rs`): Transaction parsing, validation, and execution orchestration loop
- **`basic_system`** (`/basic_system/src/lib.rs`): Concrete implementations of IO subsystem, storage models, and system functions
- **`zk_ee`** (`/zk_ee/src/lib.rs`): Execution environment traits (`ExecutionEnvironment`, `SystemTypes`, `IOSubsystem`) and common data structures
- **`evm_interpreter`** (`/evm_interpreter/src/lib.rs`): EVM opcode interpreter with stack, heap, and gas accounting

**Oracle & Data Access**:
- **`oracle_provider`** (`/oracle_provider/src/lib.rs`): `ZkEENonDeterminismSource` for query dispatching and `ReadWitnessSource` for witness collection
- **`callable_oracles`** (`/callable_oracles/src/lib.rs`): Expensive operations (KZG commitments, field arithmetic) delegated to host
- **`storage_models`** (`/storage_models/src/lib.rs`): Storage abstraction supporting flat key-value and Ethereum MPT models

**Cryptography**:
- **`crypto`** (`/crypto/src/lib.rs`): Hash functions (Blake2, SHA256, SHA3, RIPEMD160), signature schemes (secp256k1, P256), elliptic curves (BN254, BLS12-381)
- **`system_hooks`** (`/system_hooks/src/lib.rs`): EVM precompile implementations (ecrecover, pairing checks, etc.) and zkSync system contracts

---

## Major Crates

| Crate | Purpose | Type | Key Exports |
|-------|---------|------|-------------|
| **zksync_os** | RISC-V binary entry point | Executable | `_start_rust()`, `workload()`, CSR oracle |
| **api** | High-level block execution API | Library | `run_block()`, `run_block_generate_witness()`, `simulate_tx()` |
| **forward_system** | Native sequencer execution | Library | `run_block()`, `ForwardRunningSystem`, query processors |
| **proof_running_system** | RISC-V proving system | Library | `run_proving()`, bootloader initialization |
| **zksync_os_runner** | RISC-V simulator integration | Library | `run()` - executes RISC-V binary with witness collection |
| **basic_bootloader** | Bootloader orchestration | Library | `bootloader::run()`, transaction validation |
| **basic_system** | System implementation | Library | `FullIO`, flat/Ethereum storage models, system functions |
| **zk_ee** | Execution environment abstraction | Library | `ExecutionEnvironment` trait, `System<S>`, `IOSubsystem`, common structs |
| **evm_interpreter** | EVM opcode interpreter | Library | `Interpreter<S>`, opcode execution, gas accounting |
| **oracle_provider** | Oracle infrastructure | Library | `ZkEENonDeterminismSource`, `ReadWitnessSource`, query processors |
| **callable_oracles** | Expensive crypto delegations | Library | Arithmetic, KZG, hash-to-prime oracles |
| **storage_models** | Storage abstractions | Library | Flat storage, Ethereum MPT, storage key derivation |
| **crypto** | Cryptographic primitives | Library | Hash functions, signature verification, EC operations |
| **system_hooks** | Precompiles & system contracts | Library | `HooksStorage`, EVM precompiles (0x01-0x09+), system hooks |

---

## Compilation Targets

zkSync OS supports two primary compilation targets, each optimized for different execution contexts:

### Native Target (x86-64 / ARM64)

**Purpose**: Sequencer operation and forward execution

**Target Triple**: `x86_64-unknown-linux-gnu` (or native host architecture)

**Build Command**:
```bash
cargo build --release
```

**Characteristics**:
- Full standard library support
- Direct system allocator usage
- Access to databases, network, and OS resources
- Optimized for maximum throughput
- Used by sequencers to produce blocks quickly

**Key Features**:
- Uses `forward_system` crate for optimized execution
- Can access external state via direct DB queries
- No witness generation overhead
- Full debugging and profiling support

### RISC-V Target (riscv32i-unknown-none-elf)

**Purpose**: Provable execution with witness generation

**Target Triple**: `riscv32i-unknown-none-elf`

**Build Command**:
```bash
cd zksync_os
./dump_bin.sh --type <profile>
```

**Characteristics**:
- `no_std` bare-metal environment
- Custom allocator (Talc) for deterministic memory management
- All I/O through CSR-based oracle queries
- Generates execution witness (Vec<u32>) for prover
- Deterministic and reproducible execution

**Key Features**:
- Uses `proof_running_system` for bootloader
- CSR (Control & Status Register) communication for oracle queries
- Custom exception handling for RISC-V traps
- No operating system dependencies
- Output via RISC-V registers x10-x17

**Build Profiles** (defined in `/zksync_os/Cargo.toml` and `/zksync_os/dump_bin.sh`):

| Profile | Features | Output Binary | Use Case |
|---------|----------|---------------|----------|
| **production** | `proving,production` | `singleblock_batch.bin` | Production single-block batches |
| **for_tests** | `proving,for_tests` | `for_tests.bin` | Development and testing with additional precompiles |
| **eth_runner** | `proving,eth_runner` | `evm_replay.bin` | Ethereum mainnet block replay |
| **evm_tester** | `proving,evm_tester` | `evm_tester.bin` | EVM compatibility testing |
| **multiblock-batch** | `proving,production,multiblock-batch` | `multiblock_batch.bin` | Multi-block batch processing |
| **benchmarking** | `proving,eth_runner,benchmarking` | `evm_replay.bin` | Performance profiling |

**Feature Flags** (`/zksync_os/Cargo.toml:51-63`):
- `proving`: Enable crypto delegation for proving mode
- `production`: Production optimizations, P256 precompile enabled
- `for_tests`: Testing mode with point_eval and KZG precompiles
- `eth_runner`: Ethereum mainnet compatibility mode
- `print_debug_info`: Enable UART debug logging
- `benchmarking`: Cycle counting and profiling
- `multiblock-batch`: Process multiple blocks in single proof
- `state-diffs-pi`: Include state diffs in public input
- `aggregation`: Enable proof aggregation support

**Build Process** (`/zksync_os/dump_bin.sh`):
1. Clean previous artifacts for specific profile
2. `cargo build --features "$FEATURES" --release`
3. `cargo objcopy -- -O binary` → Extract raw binary
4. `cargo objcopy -- -R .text` → Extract ELF with sections
5. `cargo objcopy -- --only-section=.text` → Extract code section

---

## Execution Modes

zkSync OS operates in two distinct execution modes, each optimized for different purposes in the blockchain workflow:

### Forward Running (Sequencer Mode)

**Target Architecture**: Native (x86-64, ARM64)

**Entry Point**: `forward_system::run::run_block()` (`/forward_system/src/run/mod.rs:65`)

**Purpose**: Fast block execution for sequencer nodes producing new blocks

**Characteristics**:
- **Direct External Access**: Can query databases, make network calls, and access file systems
- **System Allocator**: Uses standard Rust allocator (jemalloc/system allocator)
- **No Witness**: Does not generate proof witness data
- **Optimized**: Maximum throughput with compiler optimizations
- **Flexible I/O**: Direct integration with node databases and RPC providers

**Data Flow**:
```
Block Context + Transactions
         ↓
   run_block() API
         ↓
   Oracle Processors (in-memory)
    - BlockMetadataResponder
    - TxDataResponder
    - ReadTreeResponder
    - GenericPreimageResponder
         ↓
   Bootloader Execution
         ↓
   EVM Interpreter (native)
         ↓
   ForwardRunningResultKeeper
         ↓
   BlockOutput (status, gas, logs, storage writes)
```

**Implementation Details** (`/forward_system/src/run/mod.rs`):
```rust
pub fn run_block<T: ReadStorageTree, PS: PreimageSource, TS: TxSource, TR: TxResultCallback>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError>
```

**Use Cases**:
- Sequencer producing new blocks
- RPC nodes executing `eth_call` / `eth_estimateGas`
- Development and debugging with full tooling
- Integration testing with external databases

### Proof Running (Prover Mode)

**Target Architecture**: RISC-V (riscv32i-unknown-none-elf)

**Entry Point**: `zksync_os::main::_start_rust()` → `workload()` → `run_proving()` (`/zksync_os/src/main.rs`)

**Purpose**: Deterministic execution with witness generation for zero-knowledge proofs

**Characteristics**:
- **Bare Metal**: No operating system, runs directly on RISC-V simulator
- **CSR Oracle**: All I/O via CSR (Control & Status Register) read/write operations
- **Witness Generation**: Collects all oracle queries/responses into Vec<u32>
- **Deterministic**: Identical inputs produce byte-for-byte identical witnesses
- **Isolated**: No external system access, all data via oracle queries

**Data Flow**:
```
Block Context + Transactions + Storage Tree
         ↓
   make_oracle_for_proofs() (wraps data in oracle processors)
         ↓
   ReadWitnessSource (wraps oracle to collect witness)
         ↓
   zksync_os_runner::run() - RISC-V simulator
         ↓
   CSR Communication Protocol
    - Write query ID + params to CSR
    - Read response from CSR
    - Witness records all CSR operations
         ↓
   RISC-V Bootloader (proof_running_system)
         ↓
   EVM Interpreter (RISC-V)
         ↓
   Output via x10-x17 registers
         ↓
   Vec<u32> Witness → zkSNARK Prover (airbender)
```

**CSR Communication** (`/zksync_os/src/oracle/mod.rs`):
```
Query Flow:
1. RISC-V: Write query_id to CSR
2. RISC-V: Write parameter count to CSR
3. RISC-V: Write each parameter to CSR (32 bits at a time)
4. Host: Process query via oracle processors
5. RISC-V: Read response length from CSR
6. RISC-V: Read each response word from CSR (32 bits at a time)
```

**Witness Format**:
- Flat Vec<u32> containing all CSR read operations
- Query structure: [query_id, param_count, param1_low, param1_high, ..., response_len, response1_low, response1_high, ...]
- Used by prover to reconstruct deterministic execution

**Implementation** (`/oracle_provider/src/lib.rs:294-324`):
```rust
pub struct ReadWitnessSource<M: MemorySource> {
    original_source: ZkEENonDeterminismSource<M>,
    read_items: Rc<RefCell<Vec<u32>>>,
}

impl<M: MemorySource> NonDeterminismCSRSource<M> for ReadWitnessSource<M> {
    fn read(&mut self) -> u32 {
        let item = self.original_source.read();
        self.read_items.borrow_mut().push(item);  // Record for witness
        item
    }
}
```

**Use Cases**:
- Generating ZK proofs for L1 verification
- Witness generation for prover nodes
- Deterministic replay for debugging
- Proof verification testing

### Comparison Table

| Aspect | Forward Running | Proof Running |
|--------|----------------|---------------|
| **Target** | Native (x86/ARM) | RISC-V |
| **Entry Point** | `forward_system::run_block()` | `zksync_os::main::_start_rust()` |
| **Allocator** | System allocator | Talc (custom) |
| **I/O Method** | Direct (DB, network) | CSR oracle queries |
| **Witness** | None | Vec<u32> recorded |
| **Speed** | Very fast (~10,000 TPS) | Slower (simulator overhead) |
| **External Access** | Yes (databases, files) | No (oracle only) |
| **Determinism** | Not guaranteed | Guaranteed |
| **Debugging** | Full tooling (gdb, perf) | Limited (UART logging) |
| **Memory Model** | Virtual memory (OS) | Fixed RISC-V memory map |
| **Output** | BlockOutput struct | Registers x10-x17 + witness |
| **Use Case** | Sequencer, RPC nodes | Prover nodes, L1 verification |

---

## Component Architecture

The zkSync OS architecture follows a layered design with clear separation between system layers, execution environments, and I/O subsystems:

```
┌─────────────────────────────────────────────────────────────────┐
│                          API Layer                              │
│  (api crate - High-level functions for block execution)         │
│                                                                  │
│  • run_block() - Native forward execution                       │
│  • run_block_generate_witness() - Proof execution with witness  │
│  • simulate_tx() - Transaction simulation (eth_call)            │
└────────────────────────┬────────────────────────────────────────┘
                         │
          ┌──────────────┴──────────────┐
          │                             │
          ▼                             ▼
┌──────────────────────┐    ┌──────────────────────────┐
│  Forward System      │    │  Proof Running System    │
│  (forward_system)    │    │  (proof_running_system)  │
│                      │    │                          │
│  • Native execution  │    │  • RISC-V bootloader     │
│  • Direct DB access  │    │  • CSR oracle interface  │
│  • Result keeper     │    │  • Bare metal runtime    │
└──────────┬───────────┘    └────────┬─────────────────┘
           │                         │
           │    ┌────────────────────┘
           │    │
           ▼    ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Oracle Provider                             │
│  (oracle_provider - Query dispatching and witness collection)    │
│                                                                  │
│  • ZkEENonDeterminismSource - Query router                      │
│  • ReadWitnessSource - Witness recording wrapper                │
│  • Query processors:                                             │
│    - BlockMetadataResponder (block context)                     │
│    - TxDataResponder (transaction data)                         │
│    - ReadTreeResponder (Merkle tree proofs)                     │
│    - GenericPreimageResponder (bytecode, account data)          │
│    - UARTPrintResponder (debug logging)                         │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Bootloader                                 │
│  (basic_bootloader - Transaction orchestration)                  │
│                                                                  │
│  • Parse and validate transactions                               │
│  • Manage execution frames and call stack                        │
│  • Handle preemption for external calls                          │
│  • Collect results and rollback on revert                        │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│               Execution Environments (zk_ee)                     │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  ExecutionEnvironment Trait                              │  │
│  │  • new() - Create new EE instance                        │  │
│  │  • start_executing_frame() - Begin execution             │  │
│  │  • continue_after_preemption() - Resume after call       │  │
│  └──────────────────────────────────────────────────────────┘  │
│                         │                                        │
│  ┌──────────────────────┴────────────────────┐                  │
│  │                                            │                  │
│  ▼                                            ▼                  │
│  ┌────────────────────┐         ┌─────────────────────────┐    │
│  │   EVM Interpreter  │         │   Future: EraVM/Wasm    │    │
│  │ (evm_interpreter)  │         │    (Planned)            │    │
│  │                    │         │                         │    │
│  │ • Stack machine    │         │ • EraVM interpreter     │    │
│  │ • Opcode dispatch  │         │ • Wasm runtime          │    │
│  │ • Gas accounting   │         │                         │    │
│  │ • Memory/heap      │         │                         │    │
│  └────────────────────┘         └─────────────────────────┘    │
└───────────────────────┬─────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                    System Layer (zk_ee)                          │
│                                                                  │
│  System<S: SystemTypes> - Central execution hub                 │
│  ├─ IO: IOSubsystem - All side effects                          │
│  ├─ Metadata: Block and transaction context                     │
│  └─ Allocator: Memory management                                │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              IOSubsystem (basic_system)                  │  │
│  │                                                          │  │
│  │  • storage_read/write() - Storage operations            │  │
│  │  • emit_event() - Event logging                         │  │
│  │  • emit_l1_message() - L1 communication                 │  │
│  │  • read_nonce/increment_nonce() - Nonce management      │  │
│  │  • start_io_frame/finish_io_frame() - Snapshotting     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │            Resources (Double Accounting)                 │  │
│  │                                                          │  │
│  │  • EVM Gas (Ergs) - Ethereum compatibility               │  │
│  │  • Native Resources - RISC-V cycle costs                 │  │
│  │  • charge() - Deduct resources                           │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────┬────────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
         ▼               ▼               ▼
┌──────────────┐ ┌──────────────┐ ┌─────────────────┐
│   Storage    │ │ System Hooks │ │ Callable Oracles│
│   Models     │ │  (Precompiles│ │  (Expensive Ops)│
│              │ │   & Hooks)   │ │                 │
│ • Flat       │ │              │ │ • Arithmetic    │
│ • Ethereum   │ │ • ecrecover  │ │ • KZG commits   │
│   MPT        │ │ • SHA256     │ │ • Hash-to-prime │
│ • Transient  │ │ • BN254      │ │                 │
│              │ │ • ModExp     │ │                 │
└──────────────┘ └──────────────┘ └─────────────────┘
```

### Component Interaction Flow

**Transaction Execution Example**:

1. **API Layer**: `run_block()` receives block context, transactions, and storage tree
2. **Oracle Setup**: Creates oracle processors (metadata, storage, tx data, preimages)
3. **Bootloader Initialization**: Parses transactions, sets up execution frames
4. **Execution Loop**: For each transaction:
   - Bootloader creates execution environment (EVM interpreter)
   - EE executes opcodes, accessing stack, memory, and storage
   - External calls trigger preemption → bootloader handles call → resume EE
   - IOSubsystem handles storage reads/writes, events, nonces
   - Resources (gas + native) tracked throughout
5. **Precompiles/Hooks**: Special addresses (0x01-0x09, system contracts) intercepted by `HooksStorage`
6. **Result Collection**: Per-transaction and per-block results aggregated
7. **Output**: BlockOutput with tx results, storage writes, events, gas usage

**Key Abstractions**:

- **`SystemTypes` trait**: Dependency injection for IO, allocator, logger, and resource types
- **`ExecutionEnvironment` trait**: Pluggable execution engines (EVM, EraVM, Wasm)
- **`IOSubsystem` trait**: Abstract interface for all side effects
- **Oracle pattern**: All external data flows through query-response protocol
- **Preemption model**: External calls don't recurse - they suspend and return control to bootloader

### Cross-References

- **Execution Environments**: See [ExecutionEnvironments.md](./ExecutionEnvironments.md) for detailed EVM interpreter architecture
- **System Layer**: See [SystemLayer.md](./SystemLayer.md) for IOSubsystem, Resources, and Metadata
- **Storage**: See [Storage.md](./Storage.md) for storage models and state management
- **Cryptography**: See [Cryptography.md](./Cryptography.md) for precompiles and callable oracles
- **Data Flow**: See [DataFlow.md](./DataFlow.md) for complete data flow from input to output
- **Key Types**: See [KeyTypes.md](./KeyTypes.md) for critical data structures

---

## Build System

### Workspace Structure

The zkSync OS uses Cargo workspaces with a monorepo structure (`/Cargo.toml`):

**Workspace Members** (37 crates):
- Core: `api`, `zksync_os_runner`, `forward_system`, `proof_running_system`
- Execution: `basic_bootloader`, `basic_system`, `zk_ee`, `evm_interpreter`
- Infrastructure: `oracle_provider`, `callable_oracles`, `storage_models`
- Crypto: `crypto`, `system_hooks`
- Supporting: `modexp`, `keccak`, `solidity_abi`, `u256`, `delegated_u256`
- Testing: `tests/rig`, `tests/instances/*`

**Excluded Members**:
- `zksync_os` (separate target, RISC-V)
- `crypto/src/blake2s/test_program`
- `circuit_test_program`
- `tests/evm_tester`
- `tests/instances/eth_runner`

### External Dependencies

**zkSNARK Prover Integration** (`/Cargo.toml:63-67`):
```toml
risc_v_simulator = { git = "https://github.com/matter-labs/zksync-airbender", tag = "v0.4.3" }
blake2s_u32 = { git = "https://github.com/matter-labs/zksync-airbender", tag = "v0.4.3" }
prover_examples = { git = "https://github.com/matter-labs/zksync-airbender", tag = "v0.4.3" }
execution_utils = { git = "https://github.com/matter-labs/zksync-airbender", tag = "v0.4.3" }
prover = { git = "https://github.com/matter-labs/zksync-airbender", tag = "v0.4.3" }
```

**Interface Crates**:
```toml
zksync_os_evm_errors = { version = "0.0.10" }  # EVM error types
zksync_os_interface = { version = "0.0.10" }   # Common interfaces
```

### Build Profiles

**Release Profile** (`/Cargo.toml:74-78`):
```toml
[profile.release]
opt-level = 3        # Maximum optimizations
lto = true           # Link-time optimization
codegen-units = 1    # Single codegen unit for better optimization
debug = true         # Keep debug symbols
```

**RISC-V Profiles** (`/zksync_os/Cargo.toml:23-63`):
```toml
[profile.dev]
opt-level = 0
lto = false
panic = "abort"      # No unwinding on RISC-V

[profile.release]
opt-level = 3
lto = true
panic = "abort"
```

### Development Workflow

**Setup** (one-time):
```bash
rustup target add riscv32i-unknown-none-elf
cargo install cargo-binutils
rustup component add llvm-tools-preview
```

**Build for Native**:
```bash
cargo build --release
```

**Build for RISC-V**:
```bash
cd zksync_os
./dump_bin.sh --type for-tests
```

**Run Tests**:
```bash
cargo test --release -p transactions -- --nocapture
cargo test --release --features e2e_proving -p transactions  # With proving
```

**Integration with anvil-zksync**:
```bash
# Terminal 1: Run anvil with zkOS
cargo run -- --use-zkos --zkos-bin-path=../zksync-os/zksync_os/for_tests.bin

# Terminal 2: Get witness via RPC
http POST http://127.0.0.1:8011 \
    Content-Type:application/json \
    id:=1 jsonrpc="2.0" method="zkos_getWitness" params:='[1]'
```

### Code References

- **Main API**: `/api/src/lib.rs` - Entry points for block execution
- **Forward System**: `/forward_system/src/run/mod.rs` - Native execution
- **Proof System**: `/proof_running_system/src/` - RISC-V bootloader
- **Bootloader**: `/basic_bootloader/src/bootloader/` - Transaction orchestration
- **EVM Interpreter**: `/evm_interpreter/src/lib.rs` - Opcode execution
- **System Layer**: `/zk_ee/src/system/mod.rs` - System abstraction
- **Oracle**: `/oracle_provider/src/lib.rs` - Query dispatching
- **RISC-V Entry**: `/zksync_os/src/main.rs` - Bare-metal entry point

---

## Summary

zkSync OS provides a modular, verifiable state transition function with:

- **Dual execution modes**: Native forward running (fast sequencer) and RISC-V proof running (verifiable)
- **Multi-environment support**: Abstract execution interface supporting EVM (current), EraVM (planned), Wasm (planned)
- **Oracle-based I/O**: Deterministic replay via query-response protocol
- **Comprehensive testing**: Integration tests, fuzzing, and end-to-end proving
- **Production-ready**: Deployed in zkSync Era with proven security and performance

This architecture enables zkSync to achieve high throughput while maintaining full verifiability through zero-knowledge proofs.
