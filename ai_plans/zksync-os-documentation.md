# zkSync OS Documentation Implementation Plan

## Executive Summary

This plan creates comprehensive, quick-and-dirty markdown documentation for the zkSync OS library. The documentation will be structured as a readable "markdown book" focusing on understanding the system's architecture, data flow, and key components.

**Problem Statement**: The zkSync OS codebase is complex with multiple execution environments, RISC-V proving systems, and intricate data flow patterns. Without structured documentation, new contributors and users struggle to understand how data flows through the system, what the major API entry points are, and how key components interact.

**Proposed Solution**: Create 11 focused markdown documents covering different aspects of the system:
- High-level overview and architecture
- Detailed data flow analysis (special emphasis)
- API documentation (Rust API, RPC)
- Component-specific documentation (execution environments, storage, crypto)
- Key data structures reference
- Comprehensive index tying everything together

**Technical Approach**:
- Extract information from the codebase exploration already completed
- Reference specific code locations for key classes and data structures
- Include ASCII diagrams for architecture and data flow visualization
- Focus on "need-to-know" information rather than exhaustive API docs
- Emphasize how data enters, transforms, and exits the system

**Expected Outcomes**:
- A set of markdown files in `./docs/book/` that can be read sequentially or used as reference
- Clear understanding of data flow through CLI endpoints and APIs
- Quick reference for key classes, structs, and their purposes
- Navigable index for finding specific topics

---

## Goals & Objectives

### Primary Goals
- **Create accessible documentation** that enables new contributors to understand zkSync OS architecture in < 2 hours of reading
- **Document data flow comprehensively** with clear diagrams showing input → processing → output for all major entry points
- **Reference key code structures** with file paths and line numbers for important classes/structs

### Secondary Objectives
- Enable quick lookup of API entry points and their purposes
- Provide clear mental models of system architecture
- Create reusable diagrams that can be updated as system evolves
- Maintain "quick-and-dirty" approach - focus on understanding over completeness

---

## Solution Overview

### Approach
Create 11 markdown files organized by topic, each self-contained but cross-referenced. Start with high-level overview, drill into specifics, end with comprehensive index.

### Key Components
1. **Overview.md**: What zkSync OS is, its purpose, and quick-start mental model
2. **Architecture.md**: System components, crate organization, compilation targets
3. **DataFlow.md**: Detailed data flow from entry to exit (PRIMARY FOCUS)
4. **API.md**: Public APIs including Rust functions and RPC integration
5. **ExecutionEnvironments.md**: EVM interpreter and EE abstraction
6. **SystemLayer.md**: Core system primitives, IO subsystem, resource accounting
7. **Storage.md**: Storage models and state management
8. **Cryptography.md**: Crypto primitives, precompiles, callable oracles
9. **ProofSystem.md**: RISC-V execution, witness generation, proving
10. **KeyTypes.md**: Critical data structures and their roles
11. **Index.md**: Comprehensive overview and navigation guide

### Directory Structure
```
docs/book/
├── Overview.md
├── Architecture.md
├── DataFlow.md (PRIMARY - most detailed)
├── API.md
├── ExecutionEnvironments.md
├── SystemLayer.md
├── Storage.md
├── Cryptography.md
├── ProofSystem.md
├── KeyTypes.md
└── Index.md (created LAST)
```

### Expected Outcomes
- New contributors can read Overview → Architecture → DataFlow to get 80% understanding
- Existing contributors can use specific docs as reference
- Index provides quick navigation to any topic
- Code references enable jumping directly to implementation

---

## Implementation Tasks

### CRITICAL IMPLEMENTATION RULES
1. **NO PLACEHOLDER CODE**: Every markdown file must be complete and production-ready.
2. **COMPLETE DOCUMENTATION**: Each task fully documents its topic including all code references.
3. **DETAILED SPECIFICATIONS**: Each task specifies EXACTLY what sections, diagrams, and code references to include.
4. **CROSS-REFERENCES**: Link between documents where topics overlap.
5. **CODE LOCATIONS**: Always include file paths and line numbers for referenced code.

---

### Visual Dependency Tree

```
docs/book/
├── Overview.md (Task #0: Foundation - no dependencies)
├── Architecture.md (Task #0: Foundation - no dependencies)
├── KeyTypes.md (Task #0: Foundation - document critical structs)
│
├── DataFlow.md (Task #1: Requires KeyTypes context)
├── API.md (Task #1: Requires Architecture context)
├── ExecutionEnvironments.md (Task #1: Requires KeyTypes context)
│
├── SystemLayer.md (Task #2: Requires ExecutionEnvironments context)
├── Storage.md (Task #2: Requires KeyTypes context)
├── Cryptography.md (Task #2: Requires Architecture context)
├── ProofSystem.md (Task #2: Requires DataFlow context)
│
└── Index.md (Task #3: Requires ALL other docs complete)
```

---

### Execution Plan

#### Group A: Foundation Documents (Execute all in parallel)

---

- [x] **Task #0: Create Overview.md**
  - **Folder**: `docs/book/`
  - **File**: `Overview.md`
  - **Purpose**: Provide high-level introduction to zkSync OS
  - **Sections to Include**:
    1. **What is zkSync OS?**
       - State transition function for zkSync Layer 2
       - RISC-V based proving system
       - EVM equivalence with ZK proof generation
       - Goals: $0.0001 per ERC20 transfer, 10,000 TPS
    2. **Quick Mental Model**
       - Input: Blocks + Transactions
       - Processing: EVM execution in RISC-V
       - Output: New state + ZK witness
    3. **Key Capabilities**
       - Multiple execution environments (EVM, EraVM, Wasm)
       - Dual execution modes: Forward (sequencer) and Proof (RISC-V)
       - Oracle-based external data access
       - Double resource accounting (EVM gas + native resources)
    4. **Repository Structure**
       - Link to Architecture.md for details
       - Brief overview of major crates
    5. **Getting Started**
       - Point to key entry points in API.md
       - Mention compilation targets (x86, RISC-V)
  - **Code References**:
    - Reference `api/src/lib.rs` as primary entry point
    - Reference `zksync_os/src/main.rs` as RISC-V entry
  - **ASCII Diagram**: Simple input → processing → output flow
  - **Cross-references**: Link to Architecture.md, API.md, DataFlow.md
  - **Length**: ~200-300 lines (keep it concise)

---

- [x] **Task #0: Create Architecture.md**
  - **Folder**: `docs/book/`
  - **File**: `Architecture.md`
  - **Purpose**: Document system architecture and crate organization
  - **Sections to Include**:
    1. **Project Purpose & Goals**
       - zkSync state transition function
       - Cost and throughput targets
    2. **Directory Structure**
       - Complete tree with crate purposes:
         - Core: zksync_os, api, forward_system, proof_running_system
         - Execution: basic_bootloader, basic_system, zk_ee, evm_interpreter
         - Supporting: oracle_provider, callable_oracles, storage_models, crypto
         - Testing: tests/instances, tests/rig, tests/fuzzer
    3. **Major Crates**
       - Table format with columns: Crate | Purpose | Type | Key Exports
       - Include all major crates from research
    4. **Compilation Targets**
       - Native (x86/ARM) for sequencer
       - RISC-V for proving
       - Build profiles: production, for_tests, eth_runner, etc.
    5. **Execution Modes**
       - Forward Running (native optimization)
       - Proof Running (RISC-V bare metal)
       - Differences and use cases
    6. **Component Architecture**
       - ASCII diagram showing major subsystems:
         ```
         [API Layer]
              ↓
         [Bootloader]
              ↓
         [Execution Environments] ← [System Layer]
              ↓
         [Storage Models]
              ↓
         [Crypto & Oracles]
         ```
  - **Code References**:
    - Reference all major crate lib.rs files
    - Reference Cargo.toml for build profiles
  - **Cross-references**: Link to specific component docs (ExecutionEnvironments.md, Storage.md, etc.)
  - **Length**: ~400-500 lines

---

- [x] **Task #0: Create KeyTypes.md**
  - **Folder**: `docs/book/`
  - **File**: `KeyTypes.md`
  - **Purpose**: Document the top 15 critical data structures
  - **Sections to Include**:
    1. **Introduction**
       - Purpose: Quick reference for key types
       - How to use this document
    2. **Core System Types** (organize by category):
       - **System Architecture**:
         - `SystemTypes` trait (`zk_ee/src/system/mod.rs:59-74`)
           - Purpose, associated types, usage
         - `System<S: SystemTypes>` (`zk_ee/src/system/mod.rs:77-81`)
           - Fields: io, metadata, allocator
           - Role as central hub
       - **Execution Environment**:
         - `ExecutionEnvironment` trait (`zk_ee/src/system/execution_environment/mod.rs:40-101`)
           - Key methods and lifecycle
         - `Interpreter<'a, S>` (`evm_interpreter/src/lib.rs:81-112`)
           - All fields with descriptions
           - Role in EVM execution
       - **I/O & Resources**:
         - `IOSubsystem` trait (`zk_ee/src/system/io.rs:27-161`)
           - Key methods for storage, events, nonces
         - `FullIO<A,R,P,SF,M,O>` (`basic_system/src/system_implementation/system/io_subsystem.rs:45-62`)
           - Generic parameters and fields
         - `Resources` trait (`zk_ee/src/system/resources.rs`)
           - Resource accounting methods
       - **Call Handling**:
         - `ExternalCallRequest<'a, S>` (`zk_ee/src/system/execution_environment/environment_state.rs:26-38`)
           - All fields with purposes
         - `CallResult<'a, S>` (`zk_ee/src/system/execution_environment/call_params.rs:68-75`)
           - Variants and usage
         - `ExecutionEnvironmentLaunchParams<'a, S>` (`zk_ee/src/system/execution_environment/environment_state.rs:13-16`)
       - **Storage**:
         - `WarmStorageKey` & `WarmStorageValue` (`zk_ee/src/common_structs/`)
           - Fields and gas calculation role
       - **Metadata**:
         - `BlockMetadataFromOracle` (`zk_ee/src/system/metadata/zk_metadata.rs:108-127`)
           - All fields (chain_id, block_number, etc.)
       - **Events & Logs**:
         - `LogsStorage` & `EventsStorage` (`zk_ee/src/common_structs/`)
           - Purpose and methods
       - **Errors**:
         - `EvmError` (`evm_interpreter/src/errors.rs`)
           - Common variants
    3. **Data Flow Summary**
       - ASCII diagram showing how these types relate:
         ```
         BlockMetadata → System → ExecutionEnvironment
                                      ↓
                                ExternalCallRequest
                                      ↓
                                  CallResult
                                      ↓
                          IOSubsystem (storage, events)
         ```
  - **Code References**: Every type includes file path and line numbers
  - **Format**: Each type gets:
    - Location
    - Purpose (2-3 sentences)
    - Key fields/methods
    - Usage in data flow
  - **Cross-references**: Link to DataFlow.md, ExecutionEnvironments.md, SystemLayer.md
  - **Length**: ~500-600 lines

---

#### Group B: Core Documentation (Execute all in parallel after Group A)

---

- [x] **Task #1: Create DataFlow.md**
  - **Folder**: `docs/book/`
  - **File**: `DataFlow.md`
  - **Purpose**: **PRIMARY FOCUS** - Comprehensive documentation of data flow through the system
  - **Dependencies**: Requires KeyTypes.md for type references
  - **Sections to Include**:
    1. **Introduction**
       - Why data flow matters
       - Overview of major pathways
    2. **Entry Points**
       - **API Layer** (`api/src/lib.rs`):
         - `run_block()` - Direct block execution
         - `run_block_generate_witness()` - Proof generation
         - `simulate_tx()` - Call simulation
         - Complete function signatures with parameter descriptions
       - **RISC-V Entry** (`zksync_os/src/main.rs`):
         - `_start_rust()` → `workload()` → `run_proving()`
       - **Testing Entry** (`zksync_os_runner`):
         - RISC-V simulator execution
    3. **Data Input Sources**
       - Detailed breakdown of all query processors:
         - `BlockMetadataResponder` - block context
         - `TxDataResponder` - transaction data (4 query types)
         - `ReadTreeResponder` - storage with Merkle proofs
         - `ReadStorageResponder` - flat storage access
         - `GenericPreimageResponder` - bytecode and account data
         - `ZKProofDataResponder` - proof data
         - `DACommitmentSchemeResponder` - DA commitments
         - `CallableOracles` - arithmetic, KZG, etc.
       - For each: Query IDs, parameters, return types
    4. **Complete Data Flow Pipeline**
       - **Stage 1: Input Assembly**
         - BlockContext + Tree + PreimageSource + TxSource setup
         - Oracle processor registration
         - ASCII diagram of oracle architecture
       - **Stage 2: RISC-V Execution**
         - CSR-based communication protocol
         - Query buffering and streaming
         - Diagram of CSR read/write flow
       - **Stage 3: Bootloader Initialization**
         - Memory setup
         - Transaction parsing
         - Validation flow
       - **Stage 4: Transaction Execution**
         - EVM interpreter loop
         - Opcode execution
         - External call handling (preemption model)
         - Storage access
         - Event emission
         - Detailed opcode flow diagram
       - **Stage 5: Result Collection**
         - `ForwardRunningResultKeeper` accumulation
         - Per-transaction results
         - Per-block aggregation
       - **Stage 6: Output Formatting**
         - Conversion to `BlockOutput`
         - Transaction results structure
         - Storage writes
         - Events and logs
         - Published preimages
    5. **Query Processing Pipeline**
       - Detailed CSR communication flow
       - Query ID routing mechanism
       - Parameter serialization
       - Result streaming
       - Complete diagram with example
    6. **Storage Access Pipeline**
       - SLOAD/SSTORE flow
       - Flat storage key derivation
       - Oracle query generation
       - State change tracking
       - Diagram with concrete example
    7. **Output Formats**
       - **BlockOutput** structure (complete breakdown)
       - **Witness data** format (Vec<u32>)
       - **TxResult** structure
       - Example outputs with annotations
    8. **Data Transformation Examples**
       - **Example 1**: Transaction bytes → Execution → TxResult
       - **Example 2**: Storage key → Oracle query → Value → State write
       - **Example 3**: Execution → Witness collection → Proof input
       - Each with step-by-step data transformation
    9. **Query ID Reference Table**
       - Complete table of all query IDs
       - Columns: Query ID | Constant Name | Processor | Purpose
  - **ASCII Diagrams**: Minimum 5 comprehensive diagrams:
    - Overall data flow (input → output)
    - Oracle architecture
    - CSR communication protocol
    - Transaction execution pipeline
    - Storage access flow
  - **Code References**:
    - Reference all entry points with signatures
    - Reference all query processors
    - Reference key transformation points
  - **Cross-references**: Link to KeyTypes.md for type details, API.md for entry points
  - **Length**: ~800-1000 lines (most detailed document)

---

- [x] **Task #1: Create API.md**
  - **Folder**: `docs/book/`
  - **File**: `API.md`
  - **Purpose**: Document all public APIs (Rust and RPC)
  - **Dependencies**: Requires Architecture.md for context
  - **Sections to Include**:
    1. **Introduction**
       - API surface overview
       - Who uses each API
    2. **Primary Rust API** (`zksync_os_api` crate):
       - **`run_block<T, PS, TS, TR>()`**:
         - Complete signature with generics explained
         - Parameter descriptions:
           - `block_context: BlockContext` - block metadata
           - `tree: T` - storage tree implementation
           - `preimage_source: PS` - preimage provider
           - `tx_source: TS` - transaction source
           - `tx_result_callback: TR` - callback per tx
           - `tracer: &mut impl Tracer` - execution tracer
         - Return type: `Result<BlockOutput, ForwardSubsystemError>`
         - Usage examples
       - **`run_block_generate_witness()`**:
         - Complete signature
         - All parameters explained
         - Return type: `Vec<u32>` - witness data
         - When to use vs `run_block()`
       - **`simulate_tx()`**:
         - Purpose: eth_call / eth_estimateGas
         - Parameters
         - Return type
       - Code location: `api/src/lib.rs` with line numbers
    3. **RPC Integration**
       - Alloy library usage for Ethereum types
       - Integration with anvil-zksync
       - **Custom RPC methods**:
         - `zkos_getWitness` - retrieve witness data
         - Parameters and return format
       - RPC endpoints: `http://localhost:8011-8012`
       - Code references: Where RPC types are defined
    4. **Internal APIs** (for advanced users):
       - **ForwardSystem API** (`forward_system/src/`):
         - `run_block()` - native execution
         - `generate_proof_input()` - proof data prep
       - **ProofRunningSystem API** (`proof_running_system/src/`):
         - `run_proving()` - RISC-V bootloader
       - **Runner API** (`zksync_os_runner/src/`):
         - `run()` - RISC-V simulator execution
    5. **Type Reference**
       - Quick reference for key API types:
         - `BlockContext` structure
         - `BlockOutput` structure
         - `TxResult` structure
         - `ForwardSubsystemError` variants
       - Link to KeyTypes.md for full details
    6. **Usage Patterns**
       - **Sequencer pattern**: Direct `run_block()` usage
       - **Prover pattern**: `run_block_generate_witness()` flow
       - **Simulation pattern**: `simulate_tx()` for estimates
    7. **Error Handling**
       - Common error scenarios
       - Error types and their meanings
       - Recovery strategies
  - **Code References**: All API functions with file paths and signatures
  - **Examples**: At least 3 usage examples with pseudo-code
  - **Cross-references**: Link to DataFlow.md for how APIs connect to execution, KeyTypes.md for types
  - **Length**: ~400-500 lines

---

- [x] **Task #1: Create ExecutionEnvironments.md**
  - **Folder**: `docs/book/`
  - **File**: `ExecutionEnvironments.md`
  - **Purpose**: Document execution environment abstraction and EVM implementation
  - **Dependencies**: Requires KeyTypes.md for trait/struct references
  - **Sections to Include**:
    1. **Introduction**
       - What is an execution environment?
       - Why abstraction matters (EVM, EraVM, Wasm support)
    2. **ExecutionEnvironment Trait** (`zk_ee/src/system/execution_environment/mod.rs:40-101`):
       - Complete trait definition
       - **Lifecycle methods**:
         - `new()` - initialization
         - `before_executing_frame()` - pre-execution setup
         - `start_executing_frame()` - begin execution
         - `continue_after_preemption()` - resume after call
         - `calculate_resources_passed_in_external_call()` - resource allocation
       - **Preemption model**:
         - Why preemption instead of direct calls
         - `ExecutionEnvironmentPreemptionPoint` enum
         - How bootloader orchestrates calls
       - Diagram: Execution flow with preemption points
    3. **EVM Interpreter** (`evm_interpreter/src/lib.rs`):
       - **`Interpreter<'a, S>` struct**:
         - All fields explained (from KeyTypes.md)
         - State management (stack, heap, bytecode)
       - **Opcode execution**:
         - Opcode categories:
           - Stack: PUSH, POP, DUP, SWAP
           - Arithmetic: ADD, SUB, MUL, DIV, MOD
           - Comparison: LT, GT, EQ, ISZERO
           - Bitwise: AND, OR, XOR, NOT, SHL, SHR
           - Memory: MLOAD, MSTORE, MSTORE8
           - Storage: SLOAD, SSTORE
           - Control flow: JUMP, JUMPI, PC, JUMPDEST
           - System: CALL, DELEGATECALL, STATICCALL, CREATE
           - Crypto: SHA3 (Keccak256)
           - Logging: LOG0-LOG4
           - Context: ADDRESS, CALLER, CALLVALUE, etc.
         - Opcode dispatch mechanism
       - **Gas accounting**:
         - EVM gas costs per opcode
         - Native resource costs
         - Double accounting model
         - Code location: `evm_interpreter/src/gas/`
       - **Stack implementation**:
         - `EvmStack` structure
         - 1024 element limit
         - Operations: push, pop, swap, dup
       - **Memory implementation**:
         - `SliceVec<'a, u8>` - sparse memory
         - Growth and gas costs
       - **External calls**:
         - How CALL triggers preemption
         - Call modifier types: Normal, Delegate, Static, Callcode
         - Value transfer handling
    4. **Future Execution Environments**
       - Placeholder for EraVM
       - Placeholder for Wasm
       - How to implement new EEs
    5. **Execution Flow Diagram**
       - Complete diagram showing:
         - Bootloader → EE initialization
         - Opcode execution loop
         - Preemption → external call → resume
         - Return to bootloader
  - **Code References**:
    - `zk_ee/src/system/execution_environment/` - trait definition
    - `evm_interpreter/src/lib.rs` - Interpreter struct
    - `evm_interpreter/src/gas/` - gas accounting
    - `evm_interpreter/src/opcodes/` - opcode implementations
  - **Cross-references**: Link to KeyTypes.md for type details, DataFlow.md for execution pipeline
  - **Length**: ~500-600 lines

---

#### Group C: Subsystem Documentation (Execute all in parallel after Group B)

---

- [x] **Task #2: Create SystemLayer.md**
  - **Folder**: `docs/book/`
  - **File**: `SystemLayer.md`
  - **Purpose**: Document system primitives, IO subsystem, and resource accounting
  - **Dependencies**: Requires ExecutionEnvironments.md for context
  - **Sections to Include**:
    1. **Introduction**
       - Role of system layer
       - Shared across all execution environments
    2. **System Architecture**
       - **`System<S: SystemTypes>` struct**:
         - Central hub for execution
         - Fields: IO, metadata, allocator
         - Methods for accessing metadata
       - **`SystemTypes` trait**:
         - Dependency injection mechanism
         - Associated types
         - Multiple implementations
    3. **IO Subsystem** (`zk_ee/src/system/io.rs`):
       - **`IOSubsystem` trait**:
         - Storage operations: `storage_read()`, `storage_write()`
         - Transient storage: `transient: true` parameter
         - Event emission: `emit_event()`
         - L1 messaging: `emit_l1_message()`
         - Nonce management: `read_nonce()`, `increment_nonce()`
         - Frame management: `start_io_frame()`, `finish_io_frame()`
       - **`FullIO<A,R,P,SF,M,O>` implementation**:
         - All generic parameters explained
         - Fields: storage, transient_storage, logs_storage, events_storage, oracle
         - How operations are validated and charged
       - **Snapshots and Rollback**:
         - `StateSnapshot` mechanism
         - When snapshots are created
         - Rollback on revert
       - Diagram: IO operation flow
    4. **Resource Accounting** (`zk_ee/src/system/resources.rs`):
       - **Double accounting model**:
         - EVM gas (Ethereum semantics)
         - Native resources (RISC-V cycles, memory)
       - **`Resources` trait**:
         - Methods: `charge()`, `from_ergs_and_native()`, `with_infinite_ergs()`
         - Resource types: computational, native
       - **Gas calculation**:
         - Per-opcode costs
         - Storage gas (warm/cold access)
         - Memory expansion gas
       - **Native resource tracking**:
         - Code location: `evm_interpreter/src/native_resource_constants/`
         - Mapping EVM operations to RISC-V costs
    5. **Memory Management**:
       - Allocator types
       - Heap boundaries
       - Stack management
       - Memory layout in RISC-V
    6. **Metadata System** (`zk_ee/src/system/metadata/`):
       - **`BasicMetadata` trait**:
         - Block metadata interface
         - Transaction metadata interface
       - **`BlockMetadataFromOracle`**:
         - All fields (from KeyTypes.md)
         - How bootloader provides metadata
       - **ZK-specific metadata**:
         - `ZkSpecificPricingMetadata` trait
         - Pubdata pricing
         - Native token pricing
    7. **Logger System**:
       - Logging interface
       - Output handling (UART in RISC-V)
  - **Code References**:
    - `zk_ee/src/system/mod.rs` - System struct
    - `zk_ee/src/system/io.rs` - IOSubsystem trait
    - `basic_system/src/system_implementation/system/io_subsystem.rs` - FullIO
    - `zk_ee/src/system/resources.rs` - Resources trait
    - `evm_interpreter/src/gas/` - gas constants
  - **Diagrams**:
    - IO subsystem architecture
    - Resource accounting flow
  - **Cross-references**: Link to KeyTypes.md, ExecutionEnvironments.md
  - **Length**: ~500-600 lines

---

- [x] **Task #2: Create Storage.md**
  - **Folder**: `docs/book/`
  - **File**: `Storage.md`
  - **Purpose**: Document storage models and state management
  - **Dependencies**: Requires KeyTypes.md for storage types
  - **Sections to Include**:
    1. **Introduction**
       - Storage abstraction overview
       - Multiple storage models
    2. **Storage Models**:
       - **Flat Storage Model** (`basic_system/src/system_implementation/flat_storage_model/`):
         - Purpose: Efficient flat key-value storage
         - Components:
           - Account cache
           - Storage cache
           - Preimage cache
         - Flat key derivation: `derive_flat_storage_key(address, key)`
         - Merkle tree operations
         - When to use: Forward execution, proving
       - **Ethereum Storage Model** (`basic_system/src/system_implementation/ethereum_storage_model/`):
         - Purpose: Ethereum-compatible MPT (Merkle-Patricia Trie)
         - Components:
           - RLP encoding
           - Node parsing
           - Trie updates
         - When to use: Ethereum compatibility mode
       - Comparison table: Flat vs Ethereum model
    3. **Storage Access**:
       - **Read path**:
         - SLOAD opcode → IOSubsystem → StorageModel → Oracle
         - Warm vs cold access
         - Gas calculation
       - **Write path**:
         - SSTORE opcode → IOSubsystem → StorageModel
         - Journaling for rollback
         - Pubdata calculation
       - **Transient storage**:
         - EIP-1153 support
         - Transaction-scoped
         - Implementation: `GenericTransientStorage`
    4. **Storage Key Types** (reference KeyTypes.md):
       - `WarmStorageKey` structure
       - `WarmStorageValue` structure
       - Warmth tracking for gas
       - Pubdata diff calculation
    5. **State Management**:
       - **State snapshots**:
         - When created (before external calls)
         - What's captured
         - Rollback mechanism
       - **State finalization**:
         - Committing storage writes
         - Generating Merkle proofs
         - Output format in `BlockOutput`
    6. **Oracle Integration**:
       - `ReadTreeResponder` for Merkle proofs
       - `ReadStorageResponder` for initial values
       - Query protocol
    7. **Storage Output**:
       - `StorageWrite` structure in `BlockOutput`
       - Account diffs
       - Preimage publication
  - **Code References**:
    - `storage_models/` crate
    - `basic_system/src/system_implementation/flat_storage_model/`
    - `basic_system/src/system_implementation/ethereum_storage_model/`
    - `zk_ee/src/common_structs/warm_storage_*`
  - **Diagrams**:
    - Storage read flow
    - Storage write and rollback flow
    - Flat storage structure
  - **Cross-references**: Link to KeyTypes.md, DataFlow.md, SystemLayer.md
  - **Length**: ~400-500 lines

---

- [x] **Task #2: Create Cryptography.md**
  - **Folder**: `docs/book/`
  - **File**: `Cryptography.md`
  - **Purpose**: Document cryptographic primitives, precompiles, and callable oracles
  - **Dependencies**: Requires Architecture.md for crypto system context
  - **Sections to Include**:
    1. **Introduction**
       - Crypto system overview
       - Role in zkSync OS
    2. **Cryptographic Primitives** (`crypto/` crate):
       - **Hash functions**:
         - Blake2/Blake2s - code location
         - SHA256 - code location
         - SHA3 (Keccak256/Keccak512) - code location
         - RIPEMD160 - code location
         - Usage and performance notes
       - **Digital signatures**:
         - secp256k1 (ECDSA) - Bitcoin/Ethereum curve
         - P256 (ECDSA) - NIST curve
         - Implementation details
       - **Elliptic curve operations**:
         - BN254 pairing checks
         - BLS12-381 operations
         - Point arithmetic
    3. **EVM Precompiles** (`system_hooks/` crate):
       - **Standard Ethereum precompiles**:
         - Address 0x01: EC Recovery (ecrecover)
         - Address 0x02: SHA256
         - Address 0x03: RIPEMD160
         - Address 0x04: Identity (datacopy)
         - Address 0x05: ModExp (modular exponentiation)
         - Address 0x06: BN254 EC Add
         - Address 0x07: BN254 EC Mul
         - Address 0x08: BN254 Pairing Check
         - Address 0x09: Blake2F
       - **Extended precompiles**:
         - P256 signature verification
         - Point evaluation precompile (EIP-4844)
       - For each: Address, purpose, parameters, return value
    4. **Callable Oracles** (`callable_oracles/` crate):
       - **Purpose**: Operations too expensive for RISC-V
       - **Arithmetic Query**:
         - Field arithmetic operations
         - Modular arithmetic
         - Use cases
       - **Blob KZG Commitment Query**:
         - EIP-4844 blob commitments
         - KZG proving system
         - Parameters and returns
       - **Hash-to-Prime Query**:
         - Cryptographic hash-to-prime
         - Use in proofs
       - **Query protocol**:
         - How execution requests oracle computation
         - Query IDs for crypto operations
    5. **Integration with Execution**:
       - How EVM CALL to precompile address works
       - Gas costs for precompiles
       - Native resource costs
       - Precompile execution flow diagram
    6. **Proof System Crypto**:
       - Role of crypto in witness generation
       - Verification at proof time
       - KZG commitments for data availability
    7. **Build Profiles and Precompiles**:
       - Production profile: P256 enabled
       - for_tests profile: point_eval + KZG
       - How to enable/disable precompiles
  - **Code References**:
    - `crypto/` crate structure
    - `system_hooks/src/` - precompile implementations
    - `callable_oracles/src/` - oracle implementations
    - Build profile configs in Cargo.toml
  - **Tables**:
    - Precompile address table with all addresses and functions
    - Callable oracle query ID table
  - **Cross-references**: Link to DataFlow.md for oracle usage, Architecture.md
  - **Length**: ~400-500 lines

---

- [x] **Task #2: Create ProofSystem.md**
  - **Folder**: `docs/book/`
  - **File**: `ProofSystem.md`
  - **Purpose**: Document RISC-V execution, witness generation, and proving system
  - **Dependencies**: Requires DataFlow.md for witness generation context
  - **Sections to Include**:
    1. **Introduction**
       - Zero-knowledge proof system overview
       - Why RISC-V?
       - Proof generation workflow
    2. **RISC-V Execution** (`zksync_os/` crate):
       - **Entry point**: `src/main.rs`
         - `_start_rust()` function
         - `workload()` function
         - `run_proving::<NonDeterminismSource>()`
       - **Bare-metal environment**:
         - No operating system
         - Custom memory layout
         - Exception/trap handling
       - **Memory management**:
         - Talc allocator for RISC-V
         - Heap boundaries
         - Stack setup
       - **CSR-based Oracle** (`src/oracle/mod.rs`):
         - CSR (Control & Status Register) interface
         - Reading from CSR triggers oracle queries
         - Query buffering
         - Result streaming
         - Complete CSR communication protocol
    3. **Witness Generation**:
       - **What is a witness?**
         - Execution trace for prover
         - All non-deterministic inputs
       - **`ReadWitnessSource`** (`oracle_provider/src/read_witness_source.rs`):
         - Wraps any oracle
         - Records all CSR read/write operations
         - Collects witness items (Vec<u32>)
       - **Witness data format**:
         - u32 stream
         - Query ID + parameters + results
         - How prover reconstructs execution
       - **API**: `run_block_generate_witness()`
         - Parameters
         - Output: Vec<u32>
         - Flow diagram from DataFlow.md
    4. **RISC-V Simulator** (`zksync_os_runner/` crate):
       - **Purpose**: Execute RISC-V binary for witness generation
       - **`run()` function**:
         - Load binary
         - Initialize simulator
         - Configure oracle delegation
         - Execute with CSR monitoring
         - Extract output from registers x10-x17
       - **Integration with `risc_v_simulator`**:
         - From zksync-airbender repository
         - Simulator configuration
         - Cycle limits
         - Diagnostics and profiling
    5. **Forward vs Proof Execution**:
       - **Forward (Native)**:
         - Direct x86/ARM execution
         - Optimized for speed
         - Used by sequencer
         - Can access external DBs/APIs
       - **Proof (RISC-V)**:
         - Bare-metal RISC-V
         - Deterministic
         - Oracle-based I/O only
         - Generates witness
       - Comparison table
    6. **Compilation Process**:
       - Building for RISC-V target
       - `dump_bin.sh` script
       - Build profiles (production, for_tests, etc.)
       - Output: RISC-V ELF binary
    7. **Proof Data**:
       - `ProofData<StorageCommitment>` structure
       - Previous block proof
       - Batch proof generation
       - DA commitment schemes
    8. **Integration with Prover**:
       - Witness data → zkSNARK prover (airbender)
       - Proof verification
       - L1 submission
  - **Code References**:
    - `zksync_os/src/main.rs` - entry point
    - `zksync_os/src/oracle/mod.rs` - CSR oracle
    - `zksync_os_runner/src/lib.rs` - simulator
    - `oracle_provider/src/read_witness_source.rs` - witness collection
    - `proof_running_system/src/` - proof system
  - **Diagrams**:
    - Proof generation pipeline
    - CSR communication flow (reference from DataFlow.md)
    - Forward vs Proof execution comparison
  - **Cross-references**: Link to DataFlow.md, Architecture.md
  - **Length**: ~500-600 lines

---

#### Group D: Final Index (Execute after all other docs complete)

---

- [x] **Task #3: Create Index.md**
  - **Folder**: `docs/book/`
  - **File**: `Index.md`
  - **Purpose**: Comprehensive index and navigation guide for all documentation
  - **Dependencies**: ALL other documentation files must be complete
  - **Sections to Include**:
    1. **Welcome**
       - Brief introduction to zkSync OS documentation
       - How to use this documentation
       - Reading paths for different audiences:
         - New contributors: Overview → Architecture → DataFlow
         - API users: API → DataFlow
         - Deep divers: Start with KeyTypes → DataFlow → Subsystems
    2. **Documentation Structure**
       - Complete list of all docs with 1-2 sentence descriptions
    3. **Quick Reference**
       - **Entry Points**:
         - Link to API.md for all entry points
         - Quick list with line references
       - **Key Types**:
         - Top 5 most important types with links to KeyTypes.md sections
       - **Data Flow**:
         - Link to DataFlow.md major sections
       - **Subsystems**:
         - Quick links to each subsystem doc
    4. **Concepts Index** (alphabetical):
       - Allocator → SystemLayer.md
       - Bootloader → Architecture.md, DataFlow.md
       - CSR (Control & Status Register) → ProofSystem.md, DataFlow.md
       - Execution Environment → ExecutionEnvironments.md
       - EVM Interpreter → ExecutionEnvironments.md
       - Gas Accounting → SystemLayer.md
       - IO Subsystem → SystemLayer.md
       - Metadata → SystemLayer.md, KeyTypes.md
       - Oracle → DataFlow.md, ProofSystem.md
       - Precompiles → Cryptography.md
       - Resources → SystemLayer.md
       - RISC-V → Architecture.md, ProofSystem.md
       - Storage Models → Storage.md
       - System Types → KeyTypes.md, SystemLayer.md
       - Witness Generation → ProofSystem.md, DataFlow.md
       - (Include ~30-40 key concepts)
    5. **Code Index**
       - **By Crate** (links to relevant docs):
         - api → API.md
         - zksync_os → Architecture.md, ProofSystem.md
         - forward_system → Architecture.md, DataFlow.md
         - proof_running_system → ProofSystem.md
         - basic_bootloader → DataFlow.md
         - basic_system → SystemLayer.md, Storage.md
         - zk_ee → KeyTypes.md, ExecutionEnvironments.md, SystemLayer.md
         - evm_interpreter → ExecutionEnvironments.md
         - oracle_provider → DataFlow.md
         - callable_oracles → Cryptography.md
         - storage_models → Storage.md
         - crypto → Cryptography.md
         - system_hooks → Cryptography.md
         - zksync_os_runner → ProofSystem.md
       - **By Component**:
         - Execution environments
         - Storage
         - Crypto
         - Oracles
         - Testing
    6. **Type Index**
       - All 15 key types from KeyTypes.md
       - Listed with link to KeyTypes.md sections
    7. **API Index**
       - All public API functions from API.md
       - Grouped by purpose
    8. **Diagram Index**
       - List of all ASCII diagrams across all docs
       - Where to find each
    9. **Glossary**
       - Brief definitions of key terms:
         - Bootloader
         - CSR
         - DA (Data Availability)
         - EE (Execution Environment)
         - Ergs
         - Flat Storage
         - MPT (Merkle-Patricia Trie)
         - Oracle
         - Preemption
         - Pubdata
         - RISC-V
         - State Transition Function
         - Witness
         - ZK (Zero-Knowledge)
    10. **Further Reading**
        - Link to existing docs/ directory
        - External resources (if any)
        - Related projects (airbender, anvil-zksync)
  - **Format**:
    - Use markdown anchors for internal navigation
    - Clear hierarchy with proper heading levels
    - Tables where appropriate
  - **Cross-references**: Links to ALL other documentation files, multiple times
  - **Length**: ~600-800 lines

---

## Implementation Workflow

This plan file serves as the authoritative checklist for implementation. When implementing:

### Required Process
1. **Load Plan**: Read this entire plan file before starting
2. **Sync Tasks**: Create TodoWrite tasks matching the checkboxes below
3. **Execute & Update**: For each task:
   - Mark TodoWrite as `in_progress` when starting
   - Update checkbox `[ ]` to `[x]` when completing
   - Mark TodoWrite as `completed` when done
4. **Maintain Sync**: Keep this file and TodoWrite synchronized throughout

### Critical Rules
- This plan file is the source of truth for progress
- Update checkboxes in real-time as work progresses
- Never lose synchronization between plan file and TodoWrite
- Mark tasks complete only when fully implemented (no placeholders)
- Tasks in same group should be executed in parallel using separate tool calls to avoid context bloat

### Progress Tracking
The checkboxes above represent the authoritative status of each task. Keep them updated as you work.

---

## Notes

- **Group A tasks** can all run in parallel (foundation documents)
- **Group B tasks** can all run in parallel after Group A completes
- **Group C tasks** can all run in parallel after Group B completes
- **Task #3 (Index.md)** must run LAST after all other docs are complete
- Each task is self-contained with complete specifications
- Code references include file paths from codebase exploration
- ASCII diagrams are specified where needed for clarity
- Total: 11 documentation files to create
- Estimated total length: ~5500-6500 lines of documentation
