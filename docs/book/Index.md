# zkSync OS Documentation Index

**Welcome to the comprehensive navigation guide for zkSync OS documentation.**

This index provides a complete map of the zkSync OS documentation suite, enabling you to quickly find information about any aspect of the system—from high-level architecture to low-level implementation details.

---

## Table of Contents

1. [Welcome](#welcome)
2. [Documentation Structure](#documentation-structure)
3. [Quick Reference](#quick-reference)
4. [Concepts Index](#concepts-index)
5. [Code Index](#code-index)
6. [Type Index](#type-index)
7. [API Index](#api-index)
8. [Diagram Index](#diagram-index)
9. [Glossary](#glossary)
10. [Further Reading](#further-reading)

---

## Welcome

### About This Documentation

This documentation provides comprehensive coverage of zkSync OS, a next-generation state transition function for zkSync Layer 2 blockchain. The system combines EVM compatibility with zero-knowledge proof generation through a unique dual-execution model:

- **Forward Running Mode**: Native x86/ARM execution optimized for sequencer throughput (~10,000 TPS)
- **Proof Running Mode**: RISC-V bare-metal execution generating deterministic witnesses for zkSNARK proofs

The documentation is organized into 10 focused documents covering different aspects of the system, from high-level architecture to detailed component implementations.

### How to Use This Documentation

**Navigation**: This index provides multiple ways to find information:
- Browse by **concept** (Alphabetical topic index)
- Browse by **code location** (Crate and component organization)
- Browse by **type** (Data structures and their roles)
- Browse by **API** (Public functions and their usage)
- Browse by **diagram** (Visual representations across docs)

**Search Strategy**: Use Ctrl+F to search this index for keywords, then follow links to detailed documentation.

### Reading Paths for Different Audiences

#### New Contributors (Understanding the System)

**Recommended path**: Get 80% understanding in 2-3 hours
1. **[Overview.md](./Overview.md)** - What zkSync OS is and why (~30 min)
2. **[Architecture.md](./Architecture.md)** - System structure and crate organization (~45 min)
3. **[DataFlow.md](./DataFlow.md)** - Complete data flow pipeline (~60 min)
4. **[KeyTypes.md](./KeyTypes.md)** - Critical data structures reference (as needed)

**Key concepts to grasp**:
- Dual execution modes (forward vs. proof)
- Oracle-based I/O abstraction
- Execution Environment trait and preemption model
- CSR-based witness generation

#### API Users (Integration Focus)

**Recommended path**: Integration-ready in 1 hour
1. **[API.md](./API.md)** - All public APIs and usage patterns (~30 min)
2. **[DataFlow.md](./DataFlow.md#entry-points)** - Entry points and data requirements (~15 min)
3. **[KeyTypes.md](./KeyTypes.md#type-reference)** - BlockContext, BlockOutput, TxResult (~15 min)

**Integration examples**:
- Sequencer: `run_block()` usage pattern
- Prover: `run_block_generate_witness()` workflow
- RPC: `simulate_tx()` for eth_call/eth_estimateGas

#### Deep Divers (Implementation Details)

**Recommended path**: Complete system understanding in 5-6 hours
1. **[KeyTypes.md](./KeyTypes.md)** - Start with type definitions (~60 min)
2. **[DataFlow.md](./DataFlow.md)** - Understand complete data flow (~90 min)
3. **[ExecutionEnvironments.md](./ExecutionEnvironments.md)** - EVM interpreter deep dive (~75 min)
4. **[SystemLayer.md](./SystemLayer.md)** - Core system primitives (~60 min)
5. **[Storage.md](./Storage.md)** - Storage models and access patterns (~45 min)
6. **[Cryptography.md](./Cryptography.md)** - Precompiles and crypto primitives (~45 min)
7. **[ProofSystem.md](./ProofSystem.md)** - RISC-V execution and witness generation (~60 min)

**Deep dive topics**:
- Opcode-level EVM execution with preemption
- Double resource accounting (EVM gas + native RISC-V)
- Flat storage model with Blake2s key derivation
- CSR-based oracle protocol for deterministic I/O

---

## Documentation Structure

### Complete Document List

| Document | Lines | Focus | Primary Audience |
|----------|-------|-------|-----------------|
| **[Overview.md](./Overview.md)** | ~580 | High-level introduction, goals, capabilities | Everyone (start here!) |
| **[Architecture.md](./Architecture.md)** | ~640 | Crate organization, compilation targets, execution modes | Developers, contributors |
| **[KeyTypes.md](./KeyTypes.md)** | ~2,100 | 15+ critical data structures with complete documentation | All developers (reference) |
| **[DataFlow.md](./DataFlow.md)** | ~1,590 | Complete data pipeline from input to output (MOST DETAILED) | Integrators, developers |
| **[API.md](./API.md)** | ~1,730 | Public Rust API, RPC integration, usage examples | API users, integrators |
| **[ExecutionEnvironments.md](./ExecutionEnvironments.md)** | ~2,300 | EVM interpreter, EE abstraction, opcode execution | EE developers, deep divers |
| **[SystemLayer.md](./SystemLayer.md)** | ~1,350 | IO subsystem, resource accounting, metadata | System developers |
| **[Storage.md](./Storage.md)** | ~1,900 | Flat storage model, Ethereum MPT, state management | Storage developers |
| **[Cryptography.md](./Cryptography.md)** | ~1,600 | Precompiles, hash functions, elliptic curves | Crypto developers |
| **[ProofSystem.md](./ProofSystem.md)** | ~2,700 | RISC-V execution, witness generation, proving workflow | Prover integrators |
| **Index.md** | ~730 | Navigation guide (this document) | Reference |

### Document Descriptions

#### [Overview.md](./Overview.md) - Your Starting Point
*What zkSync OS is, why it exists, and what it can do*

**Topics**: Core purpose, RISC-V proving system, dual execution model, key capabilities, repository structure, quick examples

**When to read**: First document for all users. Provides essential mental model and context for understanding the system.

#### [Architecture.md](./Architecture.md) - System Organization
*Project structure, crate organization, and execution modes*

**Topics**: Directory structure (37 crates), major crates table, compilation targets (native vs. RISC-V), build profiles, component architecture

**When to read**: After Overview. Essential for navigating the codebase and understanding component relationships.

#### [KeyTypes.md](./KeyTypes.md) - Data Structure Reference
*Top 15 critical types with complete field documentation*

**Topics**: SystemTypes trait, System struct, ExecutionEnvironment trait, Interpreter, IOSubsystem, FullIO, Resources, ExternalCallRequest, CallResult, WarmStorageKey/Value, BlockMetadata, EventsStorage, LogsStorage, EvmError

**When to read**: Reference document. Read sections as needed when encountering types in other docs.

#### [DataFlow.md](./DataFlow.md) - The Complete Pipeline
*How data enters, transforms, and exits the system (PRIMARY FOCUS)*

**Topics**: Entry points (API layer, RISC-V, testing), oracle architecture, query processors, complete 6-stage pipeline, storage access flow, output formats, transformation examples, query ID reference

**When to read**: After Architecture. Most detailed document showing actual data flow through execution.

#### [API.md](./API.md) - Public Interface
*All public APIs and integration patterns*

**Topics**: `run_block()`, `run_block_generate_witness()`, `simulate_tx()`, RPC integration (Alloy, anvil-zksync), internal APIs, type reference, usage patterns (sequencer/prover/simulation), error handling, code examples

**When to read**: When integrating zkSync OS into applications or services.

#### [ExecutionEnvironments.md](./ExecutionEnvironments.md) - EVM Deep Dive
*EE abstraction and EVM interpreter implementation*

**Topics**: ExecutionEnvironment trait, lifecycle methods, preemption model, EVM Interpreter struct, opcode execution (100+ opcodes), gas accounting, stack/memory/storage implementation, external call handling

**When to read**: When working on EVM execution or understanding opcode-level behavior.

#### [SystemLayer.md](./SystemLayer.md) - Core Infrastructure
*System primitives, I/O, and resource management*

**Topics**: SystemTypes trait, System struct, IOSubsystem trait, FullIO implementation, double resource accounting (ergs + native), memory management (Talc allocator), metadata system, logger

**When to read**: When working on system-level features or understanding I/O operations.

#### [Storage.md](./Storage.md) - State Management
*Storage models and access patterns*

**Topics**: Flat storage model (Blake2s key derivation), Ethereum MPT model, storage read/write flows, warm/cold access (EIP-2929), transient storage (EIP-1153), state snapshots/rollback, pubdata calculation, oracle integration

**When to read**: When working on storage features or optimizing storage access.

#### [Cryptography.md](./Cryptography.md) - Crypto Primitives
*Hash functions, signatures, precompiles, and callable oracles*

**Topics**: Hash functions (Blake2s, SHA256, Keccak256, RIPEMD160), digital signatures (secp256k1, P256), elliptic curves (BN254, BLS12-381), EVM precompiles (0x01-0x09+), callable oracles (arithmetic, KZG), build profile integration

**When to read**: When working on precompiles, cryptographic operations, or understanding crypto delegation.

#### [ProofSystem.md](./ProofSystem.md) - Zero-Knowledge Proving
*RISC-V execution, witness generation, and proof workflow*

**Topics**: RISC-V entry point, bare-metal environment, Talc allocator, CSR-based oracle protocol, witness generation (ReadWitnessSource), RISC-V simulator integration, forward vs. proof comparison, proof generation pipeline

**When to read**: When working on proof generation or understanding RISC-V execution environment.

---

## Quick Reference

### Entry Points

**API Layer** (`/api/src/lib.rs`):
- `run_block()` - Native block execution (forward mode) → [API.md#run_block](./API.md#run_block)
- `run_block_generate_witness()` - RISC-V execution with witness → [API.md#run_block_generate_witness](./API.md#run_block_generate_witness)
- `simulate_tx()` - Single transaction simulation (eth_call/eth_estimateGas) → [API.md#simulate_tx](./API.md#simulate_tx)

**RISC-V Entry** (`/zksync_os/src/main.rs`):
- `_start_rust()` → `workload()` → `run_proving()` - Bare-metal bootloader → [ProofSystem.md#entry-point](./ProofSystem.md#risc-v-execution-environment)

**Testing Entry** (`/zksync_os_runner/src/lib.rs`):
- `run()` - Execute RISC-V binary in simulator → [API.md#runner-api](./API.md#runner-api)

### Top 5 Key Types

**Complete documentation**: [KeyTypes.md](./KeyTypes.md)

1. **`SystemTypes` trait** (`zk_ee/src/system/mod.rs:59-74`) - Dependency injection for all system configuration → [KeyTypes.md#systemtypes-trait](./KeyTypes.md#systemtypes-trait)

2. **`System<S: SystemTypes>`** (`zk_ee/src/system/mod.rs:77-81`) - Central hub with I/O, metadata, allocator → [KeyTypes.md#systemsystemtypes-struct](./KeyTypes.md#systemsystemtypes-struct)

3. **`Interpreter<'a, S>`** (`evm_interpreter/src/lib.rs:81-112`) - EVM execution state (stack, heap, PC, gas) → [KeyTypes.md#interpretera-s-systemtypes-struct](./KeyTypes.md#interpretera-s-systemtypes-struct)

4. **`IOSubsystem` trait** (`zk_ee/src/system/io.rs:27-161`) - Interface for all I/O operations → [KeyTypes.md#iosubsystem-trait](./KeyTypes.md#iosubsystem-trait)

5. **`BlockMetadataFromOracle`** (`zk_ee/src/system/metadata/zk_metadata.rs:108-127`) - Block context used by EVM opcodes → [KeyTypes.md#blockmetadatafromoracle](./KeyTypes.md#blockmetadatafromoracle)

### Data Flow Sections

**Complete pipeline**: [DataFlow.md](./DataFlow.md)

- **Stage 1**: Input Assembly - Oracle processor setup → [DataFlow.md#stage-1-input-assembly](./DataFlow.md#stage-1-input-assembly)
- **Stage 2**: RISC-V Execution - CSR communication protocol → [DataFlow.md#stage-2-risc-v-execution](./DataFlow.md#stage-2-risc-v-execution)
- **Stage 3**: Bootloader Initialization - Memory and metadata setup → [DataFlow.md#stage-3-bootloader-initialization](./DataFlow.md#stage-3-bootloader-initialization)
- **Stage 4**: Transaction Execution - Opcode loop and preemption → [DataFlow.md#stage-4-transaction-execution](./DataFlow.md#stage-4-transaction-execution)
- **Stage 5**: Result Collection - ForwardRunningResultKeeper → [DataFlow.md#stage-5-result-collection](./DataFlow.md#stage-5-result-collection)
- **Stage 6**: Output Formatting - BlockOutput or witness → [DataFlow.md#stage-6-output-formatting](./DataFlow.md#stage-6-output-formatting)

### System Components

**Architecture**: [Architecture.md#component-architecture](./Architecture.md#component-architecture)

- **Bootloader**: Transaction orchestration → [DataFlow.md#stage-3-bootloader-initialization](./DataFlow.md#stage-3-bootloader-initialization)
- **Execution Environments**: EVM interpreter → [ExecutionEnvironments.md](./ExecutionEnvironments.md)
- **System Layer**: I/O, resources, metadata → [SystemLayer.md](./SystemLayer.md)
- **Storage Models**: Flat storage, Ethereum MPT → [Storage.md](./Storage.md)
- **Cryptography**: Precompiles, callable oracles → [Cryptography.md](./Cryptography.md)
- **Oracle System**: Query processors → [DataFlow.md#data-input-sources](./DataFlow.md#data-input-sources)

---

## Concepts Index

### A

**Allocator** - Memory allocation system
- Talc allocator for RISC-V → [ProofSystem.md#memory-management-with-talc-allocator](./ProofSystem.md#memory-management-with-talc-allocator), [SystemLayer.md#memory-management](./SystemLayer.md#memory-management)
- Global allocator for native → [Architecture.md#compilation-targets](./Architecture.md#compilation-targets)

**API** - Public interface functions
- Primary Rust API → [API.md#primary-rust-api](./API.md#primary-rust-api)
- RPC integration → [API.md#rpc-integration](./API.md#rpc-integration)
- Internal APIs → [API.md#internal-apis](./API.md#internal-apis)

### B

**Blake2s** - Hash function for flat storage keys
- Flat key derivation → [Storage.md#flat-storage-model](./Storage.md#flat-storage-model), [KeyTypes.md#warmstorage key](./KeyTypes.md#warmstorage key)
- Performance characteristics → [Cryptography.md#blake2-blake2s](./Cryptography.md#blake2-blake2s)

**Block Execution** - Processing transactions in a block
- Forward execution → [API.md#run_block](./API.md#run_block), [DataFlow.md#complete-data-flow-pipeline](./DataFlow.md#complete-data-flow-pipeline)
- Proof execution → [API.md#run_block_generate_witness](./API.md#run_block_generate_witness), [ProofSystem.md#witness-generation](./ProofSystem.md#witness-generation)

**BlockMetadata** - Block-level context
- Structure and fields → [KeyTypes.md#blockmetadatafromoracle](./KeyTypes.md#blockmetadatafromoracle)
- Usage in opcodes → [SystemLayer.md#metadata-system](./SystemLayer.md#metadata-system)

**Bootloader** - Transaction orchestration layer
- Initialization → [DataFlow.md#stage-3-bootloader-initialization](./DataFlow.md#stage-3-bootloader-initialization)
- Transaction loop → [DataFlow.md#stage-4-transaction-execution](./DataFlow.md#stage-4-transaction-execution)
- Call handling → [ExecutionEnvironments.md#preemption-model](./ExecutionEnvironments.md#preemption-model)

### C

**Call Handling** - External call management
- ExternalCallRequest → [KeyTypes.md#externalcallrequesta-s-systemtypes](./KeyTypes.md#externalcallrequesta-s-systemtypes)
- CallResult → [KeyTypes.md#callresulta-s-systemtypes](./KeyTypes.md#callresulta-s-systemtypes)
- Preemption flow → [ExecutionEnvironments.md#preemption-model](./ExecutionEnvironments.md#preemption-model)

**Callable Oracles** - Expensive operations delegated to host
- Arithmetic operations → [Cryptography.md#callable-oracles](./Cryptography.md#callable-oracles)
- KZG commitments → [Cryptography.md#callable-oracles](./Cryptography.md#callable-oracles)
- Query protocol → [DataFlow.md#callable-oracles](./DataFlow.md#callable-oracles)

**CSR (Control & Status Register)** - Oracle communication mechanism
- Protocol description → [ProofSystem.md#csr-based-oracle-communication-protocol](./ProofSystem.md#csr-based-oracle-communication-protocol)
- Query flow → [DataFlow.md#stage-2-risc-v-execution](./DataFlow.md#stage-2-risc-v-execution)
- Witness recording → [ProofSystem.md#witness-generation](./ProofSystem.md#witness-generation)

**Cryptography** - Cryptographic primitives and precompiles
- Hash functions → [Cryptography.md#hash-functions](./Cryptography.md#hash-functions)
- Digital signatures → [Cryptography.md#digital-signatures](./Cryptography.md#digital-signatures)
- EVM precompiles → [Cryptography.md#evm-precompiles](./Cryptography.md#evm-precompiles)

### D

**DA (Data Availability)** - L1 data submission
- Commitment schemes → [DataFlow.md#dacommitmentschemeresponder](./DataFlow.md#dacommitmentschemeresponder)
- Pubdata calculation → [Storage.md#storage-output](./Storage.md#storage-output)

**Data Flow** - Complete pipeline from input to output
- Overview → [DataFlow.md#overview-diagram](./DataFlow.md#overview-diagram)
- Six stages → [DataFlow.md#complete-data-flow-pipeline](./DataFlow.md#complete-data-flow-pipeline)
- Transformation examples → [DataFlow.md#data-transformation-examples](./DataFlow.md#data-transformation-examples)

**Double Accounting** - EVM gas + native resources
- Resource types → [KeyTypes.md#resources-trait](./KeyTypes.md#resources-trait), [SystemLayer.md#resource-accounting](./SystemLayer.md#resource-accounting)
- Why both? → [Overview.md#double-resource-accounting](./Overview.md#double-resource-accounting)

### E

**ecrecover** - ECDSA signature recovery (precompile 0x01)
- Implementation → [Cryptography.md#secp256k1-ecdsa](./Cryptography.md#secp256k1-ecdsa)
- Usage in transaction validation → [DataFlow.md#stage-4-transaction-execution](./DataFlow.md#stage-4-transaction-execution)

**EE (Execution Environment)** - Bytecode execution abstraction
- Trait definition → [ExecutionEnvironments.md#executionenvironment-trait](./ExecutionEnvironments.md#executionenvironment-trait), [KeyTypes.md#executionenvironment-trait](./KeyTypes.md#executionenvironment-trait)
- EVM implementation → [ExecutionEnvironments.md#evm-interpreter](./ExecutionEnvironments.md#evm-interpreter)
- Lifecycle methods → [ExecutionEnvironments.md#lifecycle-methods](./ExecutionEnvironments.md#lifecycle-methods)

**Ergs** - EVM gas units (1 gas = 256 ergs)
- Definition → [KeyTypes.md#resources-trait](./KeyTypes.md#resources-trait)
- Gas accounting → [SystemLayer.md#resource-accounting](./SystemLayer.md#resource-accounting)

**EVM Interpreter** - Ethereum bytecode executor
- Structure → [KeyTypes.md#interpretera-s-systemtypes-struct](./KeyTypes.md#interpretera-s-systemtypes-struct)
- Opcode execution → [ExecutionEnvironments.md#evm-interpreter](./ExecutionEnvironments.md#evm-interpreter)
- Stack/memory/storage → [ExecutionEnvironments.md#evm-interpreter](./ExecutionEnvironments.md#evm-interpreter)

**Events** - EVM LOG0-LOG4 emissions
- EventsStorage → [KeyTypes.md#eventsstorage](./KeyTypes.md#eventsstorage)
- Emission flow → [SystemLayer.md#io-subsystem](./SystemLayer.md#io-subsystem)

### F

**Flat Storage** - zkSync optimized storage model
- Key derivation (Blake2s) → [Storage.md#flat-storage-model](./Storage.md#flat-storage-model)
- Advantages → [Storage.md#flat-storage-model](./Storage.md#flat-storage-model)
- Comparison with Ethereum MPT → [Storage.md#storage-model-comparison](./Storage.md#storage-model-comparison)

**Forward Execution** - Native x86/ARM sequencer mode
- Architecture → [Architecture.md#forward-running-sequencer-mode](./Architecture.md#forward-running-sequencer-mode)
- API → [API.md#run_block](./API.md#run_block)
- Performance → [Overview.md#dual-execution-modes](./Overview.md#dual-execution-modes)

**FullIO** - Complete I/O subsystem implementation
- Structure → [KeyTypes.md#fullio-struct](./KeyTypes.md#fullio-struct)
- Storage/events/logs → [SystemLayer.md#io-subsystem](./SystemLayer.md#io-subsystem)

### G

**Gas Accounting** - EVM gas cost tracking
- Double accounting (ergs + native) → [Overview.md#double-resource-accounting](./Overview.md#double-resource-accounting)
- Opcode costs → [ExecutionEnvironments.md#evm-interpreter](./ExecutionEnvironments.md#evm-interpreter)
- Warm/cold storage → [Storage.md#storage-access-patterns](./Storage.md#storage-access-patterns)

**Glossary** - Term definitions (see [Glossary](#glossary) section below)

### H

**Heap** - Dynamic memory allocation
- Talc allocator → [ProofSystem.md#memory-management-with-talc-allocator](./ProofSystem.md#memory-management-with-talc-allocator)
- Heap management → [SystemLayer.md#memory-management](./SystemLayer.md#memory-management)

### I

**IOSubsystem** - Interface for all I/O operations
- Trait definition → [KeyTypes.md#iosubsystem-trait](./KeyTypes.md#iosubsystem-trait)
- Storage operations → [SystemLayer.md#io-subsystem](./SystemLayer.md#io-subsystem)
- Event/log emission → [KeyTypes.md#eventsstorage](./KeyTypes.md#eventsstorage), [KeyTypes.md#logsstorage](./KeyTypes.md#logsstorage)

### K

**Keccak256** - Primary Ethereum hash function
- Implementation → [Cryptography.md#sha3-keccak256-keccak512](./Cryptography.md#sha3-keccak256-keccak512)
- Usage (SHA3 opcode, addresses, Merkle trees) → [Cryptography.md#sha3-keccak256-keccak512](./Cryptography.md#sha3-keccak256-keccak512)

**KeyTypes** - Critical data structures
- Complete reference → [KeyTypes.md](./KeyTypes.md)
- Top 15 types → [KeyTypes.md#introduction](./KeyTypes.md#introduction)

### L

**Logs** - L2→L1 messages and transaction logs
- LogsStorage → [KeyTypes.md#logsstorage](./KeyTypes.md#logsstorage)
- User messages → [SystemLayer.md#io-subsystem](./SystemLayer.md#io-subsystem)
- L1 transaction logs → [KeyTypes.md#logsstorage](./KeyTypes.md#logsstorage)

### M

**Metadata** - Block and transaction context
- BlockMetadataFromOracle → [KeyTypes.md#blockmetadatafromoracle](./KeyTypes.md#blockmetadatafromoracle)
- Metadata system → [SystemLayer.md#metadata-system](./SystemLayer.md#metadata-system)
- Opcodes using metadata → [SystemLayer.md#metadata-system](./SystemLayer.md#metadata-system)

**Memory Management** - Heap/stack allocation
- Talc allocator (RISC-V) → [ProofSystem.md#memory-management-with-talc-allocator](./ProofSystem.md#memory-management-with-talc-allocator)
- Global allocator (native) → [SystemLayer.md#memory-management](./SystemLayer.md#memory-management)

**MPT (Merkle-Patricia Trie)** - Ethereum storage model
- Structure → [Storage.md#ethereum-storage-model-mpt](./Storage.md#ethereum-storage-model-mpt)
- Comparison with flat storage → [Storage.md#storage-model-comparison](./Storage.md#storage-model-comparison)

### O

**Opcodes** - EVM instruction execution
- Complete list → [ExecutionEnvironments.md#evm-interpreter](./ExecutionEnvironments.md#evm-interpreter)
- Execution loop → [DataFlow.md#stage-4-transaction-execution](./DataFlow.md#stage-4-transaction-execution)

**Oracle** - External data provider
- Architecture → [DataFlow.md#data-input-sources](./DataFlow.md#data-input-sources)
- Query processors → [DataFlow.md#query-processor-architecture](./DataFlow.md#query-processor-architecture)
- CSR protocol → [ProofSystem.md#csr-based-oracle-communication-protocol](./ProofSystem.md#csr-based-oracle-communication-protocol)

### P

**P256** - NIST elliptic curve for WebAuthn
- Implementation → [Cryptography.md#p256-ecdsa](./Cryptography.md#p256-ecdsa)
- Precompile address 0x0100 → [Cryptography.md#p256-ecdsa](./Cryptography.md#p256-ecdsa)

**Precompiles** - EVM cryptographic contracts at 0x01-0x09
- Standard precompiles → [Cryptography.md#evm-precompiles](./Cryptography.md#evm-precompiles)
- Extended precompiles → [Cryptography.md#evm-precompiles](./Cryptography.md#evm-precompiles)
- Integration → [Cryptography.md#integration-with-execution](./Cryptography.md#integration-with-execution)

**Preemption** - Cooperative call handling model
- Why preemption? → [ExecutionEnvironments.md#preemption-model](./ExecutionEnvironments.md#preemption-model)
- Preemption points → [ExecutionEnvironments.md#executionenvironmentpreemptionpoint-enum](./ExecutionEnvironments.md#executionenvironmentpreemptionpoint-enum)
- Call flow → [DataFlow.md#preemption-for-external-calls](./DataFlow.md#preemption-for-external-calls)

**Proof Generation** - Creating zkSNARK proofs
- Witness generation → [API.md#run_block_generate_witness](./API.md#run_block_generate_witness)
- RISC-V execution → [ProofSystem.md#risc-v-execution-environment](./ProofSystem.md#risc-v-execution-environment)
- Complete workflow → [ProofSystem.md#introduction](./ProofSystem.md#introduction)

**Pubdata** - Data published to L1
- Calculation → [Storage.md#storage-output](./Storage.md#storage-output)
- DA commitment schemes → [DataFlow.md#dacommitmentschemeresponder](./DataFlow.md#dacommitmentschemeresponder)

### Q

**Query Processors** - Oracle data providers
- Complete list → [DataFlow.md#data-input-sources](./DataFlow.md#data-input-sources)
- Query ID table → [DataFlow.md#query-id-reference-table](./DataFlow.md#query-id-reference-table)

### R

**Resources** - Gas and native cost tracking
- Resources trait → [KeyTypes.md#resources-trait](./KeyTypes.md#resources-trait)
- Double accounting → [SystemLayer.md#resource-accounting](./SystemLayer.md#resource-accounting)
- Why both ergs and native? → [Overview.md#double-resource-accounting](./Overview.md#double-resource-accounting)

**RISC-V** - Proving target architecture
- Entry point → [ProofSystem.md#entry-point](./ProofSystem.md#entry-point)
- Bare-metal environment → [ProofSystem.md#bare-metal-environment](./ProofSystem.md#bare-metal-environment)
- Build process → [Architecture.md#risc-v-target](./Architecture.md#risc-v-target)

**Rollback** - Reverting state changes
- Snapshot mechanism → [Storage.md#state-management](./Storage.md#state-management)
- Frame rollback → [SystemLayer.md#io-subsystem](./SystemLayer.md#io-subsystem)

### S

**secp256k1** - Ethereum standard curve
- Implementation → [Cryptography.md#secp256k1-ecdsa](./Cryptography.md#secp256k1-ecdsa)
- ecrecover (precompile 0x01) → [Cryptography.md#secp256k1-ecdsa](./Cryptography.md#secp256k1-ecdsa)

**SLOAD/SSTORE** - Storage read/write opcodes
- Read flow → [DataFlow.md#sload-flow](./DataFlow.md#sload-flow), [Storage.md#read-path-sload](./Storage.md#read-path-sload)
- Write flow → [DataFlow.md#sstore-flow](./DataFlow.md#sstore-flow), [Storage.md#write-path-sstore](./Storage.md#write-path-sstore)
- Warm/cold access → [Storage.md#storage-access-patterns](./Storage.md#storage-access-patterns)

**Stack** - EVM operand stack
- Implementation → [ExecutionEnvironments.md#evm-interpreter](./ExecutionEnvironments.md#evm-interpreter)
- 1024 element limit → [KeyTypes.md#interpretera-s-systemtypes-struct](./KeyTypes.md#interpretera-s-systemtypes-struct)

**State Transition Function** - Block processing logic
- Overview → [Overview.md#what-is-zksync-os](./Overview.md#what-is-zksync-os)
- Complete pipeline → [DataFlow.md#complete-data-flow-pipeline](./DataFlow.md#complete-data-flow-pipeline)

**Storage** - Contract storage management
- Models (flat vs. MPT) → [Storage.md#storage-models](./Storage.md#storage-models)
- Access patterns → [Storage.md#storage-access-patterns](./Storage.md#storage-access-patterns)
- WarmStorageKey/Value → [KeyTypes.md#warmstorage key](./KeyTypes.md#warmstorage key), [KeyTypes.md#warmstoragevalue](./KeyTypes.md#warmstoragevalue)

**System** - Central hub for execution
- System struct → [KeyTypes.md#systemsystemtypes-struct](./KeyTypes.md#systemsystemtypes-struct)
- System architecture → [SystemLayer.md#system-architecture](./SystemLayer.md#system-architecture)

**SystemTypes** - Dependency injection trait
- Trait definition → [KeyTypes.md#systemtypes-trait](./KeyTypes.md#systemtypes-trait)
- Associated types → [SystemLayer.md#systemtypes-trait](./SystemLayer.md#systemtypes-trait)

### T

**Talc** - RISC-V bare-metal allocator
- Implementation → [ProofSystem.md#memory-management-with-talc-allocator](./ProofSystem.md#memory-management-with-talc-allocator)
- Initialization → [ProofSystem.md#memory-management-with-talc-allocator](./ProofSystem.md#memory-management-with-talc-allocator)

**Transient Storage** - EIP-1153 transaction-scoped storage
- Implementation → [Storage.md#storage-access-patterns](./Storage.md#storage-access-patterns)
- TLOAD/TSTORE → [Storage.md#storage-access-patterns](./Storage.md#storage-access-patterns)

**Transaction Execution** - Processing individual transactions
- Execution loop → [DataFlow.md#stage-4-transaction-execution](./DataFlow.md#stage-4-transaction-execution)
- Result structure → [API.md#txresult](./API.md#txresult)

### W

**Warm/Cold Access** - EIP-2929 gas optimization
- Warm storage → [Storage.md#storage-access-patterns](./Storage.md#storage-access-patterns)
- WarmStorageKey/Value → [KeyTypes.md#warmstorage key](./KeyTypes.md#warmstorage key)
- Gas costs (100 vs. 2100) → [Storage.md#storage-access-patterns](./Storage.md#storage-access-patterns)

**Witness** - Execution trace for proving
- Generation → [ProofSystem.md#witness-generation](./ProofSystem.md#witness-generation)
- Format (Vec<u32>) → [DataFlow.md#witness-data-format](./DataFlow.md#witness-data-format)
- ReadWitnessSource → [ProofSystem.md#witness-generation](./ProofSystem.md#witness-generation)

### Z

**ZK (Zero-Knowledge)** - Cryptographic proof system
- Overview → [ProofSystem.md#introduction](./ProofSystem.md#introduction)
- Proof workflow → [ProofSystem.md#proof-generation-workflow](./ProofSystem.md#proof-generation-workflow)
- zkSNARK prover integration → [ProofSystem.md#integration-with-prover](./ProofSystem.md#integration-with-prover)

---

## Code Index

### By Crate

**Core System**:
- **api** (`/api/src/lib.rs`) → [API.md](./API.md), [DataFlow.md#api-layer-entry-points](./DataFlow.md#api-layer-entry-points)
- **zksync_os** (`/zksync_os/src/main.rs`) → [ProofSystem.md#entry-point](./ProofSystem.md#entry-point), [Architecture.md](./Architecture.md)
- **forward_system** (`/forward_system/src/`) → [Architecture.md#forward-running-sequencer-mode](./Architecture.md#forward-running-sequencer-mode), [DataFlow.md#stage-1-input-assembly](./DataFlow.md#stage-1-input-assembly)
- **proof_running_system** (`/proof_running_system/src/`) → [ProofSystem.md](./ProofSystem.md), [Architecture.md#proof-running-prover-mode](./Architecture.md#proof-running-prover-mode)
- **zksync_os_runner** (`/zksync_os_runner/src/lib.rs`) → [API.md#runner-api](./API.md#runner-api), [ProofSystem.md#risc-v-simulator](./ProofSystem.md#risc-v-simulator)

**Execution Layer**:
- **basic_bootloader** (`/basic_bootloader/src/`) → [DataFlow.md#stage-3-bootloader-initialization](./DataFlow.md#stage-3-bootloader-initialization), [Architecture.md](./Architecture.md)
- **basic_system** (`/basic_system/src/`) → [SystemLayer.md#fullio](./SystemLayer.md#fullio), [Storage.md#flat-storage-model](./Storage.md#flat-storage-model)
- **zk_ee** (`/zk_ee/src/`) → [KeyTypes.md](./KeyTypes.md), [ExecutionEnvironments.md](./ExecutionEnvironments.md), [SystemLayer.md](./SystemLayer.md)
- **evm_interpreter** (`/evm_interpreter/src/lib.rs`) → [ExecutionEnvironments.md#evm-interpreter](./ExecutionEnvironments.md#evm-interpreter), [KeyTypes.md#interpretera-s-systemtypes-struct](./KeyTypes.md#interpretera-s-systemtypes-struct)

**Oracle & Data**:
- **oracle_provider** (`/oracle_provider/src/lib.rs`) → [DataFlow.md#data-input-sources](./DataFlow.md#data-input-sources), [ProofSystem.md#witness-generation](./ProofSystem.md#witness-generation)
- **callable_oracles** (`/callable_oracles/src/`) → [Cryptography.md#callable-oracles](./Cryptography.md#callable-oracles), [DataFlow.md#callable-oracles](./DataFlow.md#callable-oracles)
- **storage_models** (`/storage_models/src/`) → [Storage.md](./Storage.md)

**Cryptography**:
- **crypto** (`/crypto/src/lib.rs`) → [Cryptography.md#cryptographic-primitives](./Cryptography.md#cryptographic-primitives)
- **system_hooks** (`/system_hooks/src/`) → [Cryptography.md#evm-precompiles](./Cryptography.md#evm-precompiles)

**Testing**:
- **tests/rig** (`/tests/rig/`) → [Architecture.md#testing-infrastructure](./Architecture.md#testing-infrastructure)
- **tests/instances/** → [Architecture.md#testing-infrastructure](./Architecture.md#testing-infrastructure)

### By Component

**Execution Environments**:
- EVM interpreter → [ExecutionEnvironments.md#evm-interpreter](./ExecutionEnvironments.md#evm-interpreter)
- ExecutionEnvironment trait → [ExecutionEnvironments.md#executionenvironment-trait](./ExecutionEnvironments.md#executionenvironment-trait)
- Future EEs (EraVM, Wasm) → [ExecutionEnvironments.md#future-execution-environments](./ExecutionEnvironments.md#future-execution-environments)

**Storage**:
- Flat storage model → [Storage.md#flat-storage-model](./Storage.md#flat-storage-model)
- Ethereum MPT model → [Storage.md#ethereum-storage-model-mpt](./Storage.md#ethereum-storage-model-mpt)
- Storage caches → [Storage.md#storage-access-patterns](./Storage.md#storage-access-patterns)

**Crypto**:
- Hash functions (Blake2s, SHA256, Keccak256, RIPEMD160) → [Cryptography.md#hash-functions](./Cryptography.md#hash-functions)
- Digital signatures (secp256k1, P256) → [Cryptography.md#digital-signatures](./Cryptography.md#digital-signatures)
- Elliptic curves (BN254, BLS12-381) → [Cryptography.md#elliptic-curve-operations](./Cryptography.md#elliptic-curve-operations)
- Precompiles → [Cryptography.md#evm-precompiles](./Cryptography.md#evm-precompiles)

**Oracles**:
- BlockMetadataResponder → [DataFlow.md#blockmetadataresponder](./DataFlow.md#blockmetadataresponder)
- TxDataResponder → [DataFlow.md#txdataresponder](./DataFlow.md#txdataresponder)
- ReadTreeResponder → [DataFlow.md#readtreeresponder](./DataFlow.md#readtreeresponder)
- ReadStorageResponder → [DataFlow.md#readstorageresponder](./DataFlow.md#readstorageresponder)
- GenericPreimageResponder → [DataFlow.md#genericpreimageresponder](./DataFlow.md#genericpreimageresponder)
- Callable oracles → [Cryptography.md#callable-oracles](./Cryptography.md#callable-oracles)

**Testing**:
- Test rig utilities → [Architecture.md#testing-infrastructure](./Architecture.md#testing-infrastructure)
- Transaction tests → [Architecture.md#testing-infrastructure](./Architecture.md#testing-infrastructure)
- Fuzzing infrastructure → [Architecture.md#testing-infrastructure](./Architecture.md#testing-infrastructure)

---

## Type Index

**Complete documentation**: [KeyTypes.md](./KeyTypes.md)

### System Types

1. **SystemTypes** trait (`zk_ee/src/system/mod.rs:59-74`) - Dependency injection → [KeyTypes.md#systemtypes-trait](./KeyTypes.md#systemtypes-trait)
2. **System&lt;S&gt;** struct (`zk_ee/src/system/mod.rs:77-81`) - Central hub → [KeyTypes.md#systemsystemtypes-struct](./KeyTypes.md#systemsystemtypes-struct)

### Execution Environment Types

3. **ExecutionEnvironment** trait (`zk_ee/src/system/execution_environment/mod.rs:40-101`) → [KeyTypes.md#executionenvironment-trait](./KeyTypes.md#executionenvironment-trait)
4. **Interpreter&lt;'a, S&gt;** struct (`evm_interpreter/src/lib.rs:81-112`) → [KeyTypes.md#interpretera-s-systemtypes-struct](./KeyTypes.md#interpretera-s-systemtypes-struct)

### I/O & Resource Types

5. **IOSubsystem** trait (`zk_ee/src/system/io.rs:27-161`) → [KeyTypes.md#iosubsystem-trait](./KeyTypes.md#iosubsystem-trait)
6. **FullIO** struct (`basic_system/src/system_implementation/system/io_subsystem.rs:45-62`) → [KeyTypes.md#fullio-struct](./KeyTypes.md#fullio-struct)
7. **Resources** trait (`zk_ee/src/system/resources.rs:128-172`) → [KeyTypes.md#resources-trait](./KeyTypes.md#resources-trait)

### Call Handling Types

8. **ExternalCallRequest&lt;'a, S&gt;** (`zk_ee/src/system/execution_environment/environment_state.rs:26-38`) → [KeyTypes.md#externalcallrequesta-s-systemtypes](./KeyTypes.md#externalcallrequesta-s-systemtypes)
9. **CallResult&lt;'a, S&gt;** (`zk_ee/src/system/execution_environment/call_params.rs:68-75`) → [KeyTypes.md#callresulta-s-systemtypes](./KeyTypes.md#callresulta-s-systemtypes)
10. **ExecutionEnvironmentLaunchParams&lt;'a, S&gt;** (`zk_ee/src/system/execution_environment/environment_state.rs:13-16`) → [KeyTypes.md#executionenvironmentlaunchparamsa-s-systemtypes](./KeyTypes.md#executionenvironmentlaunchparamsa-s-systemtypes)

### Storage Types

11. **WarmStorageKey** (`zk_ee/src/common_structs/warm_storage_key.rs:5-8`) → [KeyTypes.md#warmstorage key](./KeyTypes.md#warmstorage key)
12. **WarmStorageValue** (`zk_ee/src/common_structs/warm_storage_value.rs:10-20`) → [KeyTypes.md#warmstoragevalue](./KeyTypes.md#warmstoragevalue)

### Metadata Types

13. **BlockMetadataFromOracle** (`zk_ee/src/system/metadata/zk_metadata.rs:108-127`) → [KeyTypes.md#blockmetadatafromoracle](./KeyTypes.md#blockmetadatafromoracle)

### Events & Logs Types

14. **LogsStorage** (`zk_ee/src/common_structs/logs_storage.rs:162-166`) → [KeyTypes.md#logsstorage](./KeyTypes.md#logsstorage)
15. **EventsStorage** (`zk_ee/src/common_structs/events_storage.rs:52-60`) → [KeyTypes.md#eventsstorage](./KeyTypes.md#eventsstorage)

### Error Types

16. **EvmError** (`zk_ee/src/system/execution_environment/evm/errors.rs:8-39`) → [KeyTypes.md#evmerror](./KeyTypes.md#evmerror)

---

## API Index

**Complete documentation**: [API.md](./API.md)

### Primary Rust API (`zksync_os_api` crate)

**Block Execution**:
- `run_block()` - Native forward execution → [API.md#run_block](./API.md#run_block)
- `run_block_generate_witness()` - RISC-V with witness → [API.md#run_block_generate_witness](./API.md#run_block_generate_witness)

**Transaction Simulation**:
- `simulate_tx()` - Single transaction (eth_call/eth_estimateGas) → [API.md#simulate_tx](./API.md#simulate_tx)

### RPC Integration

**Standard Ethereum Methods** (via Alloy):
- eth_blockNumber, eth_getBlockByNumber
- eth_sendRawTransaction, eth_sendTransaction
- eth_call, eth_estimateGas
- eth_getTransactionReceipt, eth_getLogs
- eth_getBalance, eth_getCode, eth_getStorageAt

**Custom zkSync Methods**:
- zkos_getWitness - Retrieve witness for block → [API.md#zkos_getwitness](./API.md#zkos_getwitness)

### Internal APIs

**ForwardSystem** (`forward_system/src/run/mod.rs`):
- `generate_proof_input()` - Witness with proof metadata → [API.md#generate_proof_input](./API.md#generate_proof_input)
- `generate_batch_proof_input()` - Multi-block batching → [API.md#generate_batch_proof_input](./API.md#generate_batch_proof_input)
- `make_oracle_for_proofs()` - Oracle setup → [API.md#make_oracle_for_proofs](./API.md#make_oracle_for_proofs)

**ProofRunningSystem** (`proof_running_system/src/system/bootloader.rs`):
- `run_proving()` - RISC-V bootloader entry → [API.md#run_proving](./API.md#run_proving)

**Runner** (`zksync_os_runner/src/lib.rs`):
- `run()` - Execute RISC-V binary in simulator → [API.md#run](./API.md#run)
- `run_and_get_effective_cycles()` - With cycle counting → [API.md#run_and_get_effective_cycles](./API.md#run_and_get_effective_cycles)

---

## Diagram Index

This section lists all ASCII diagrams across the documentation suite for visual reference.

### Overview Diagrams

**[Overview.md](./Overview.md)**:
- zkSync OS high-level architecture (input → processing → output) - Line 46
- Repository structure tree - Line 277

### Architecture Diagrams

**[Architecture.md](./Architecture.md)**:
- Directory structure with all crates - Line 43
- Component architecture (API → System → Storage) - Line 366

### Data Flow Diagrams

**[DataFlow.md](./DataFlow.md)** (MOST DIAGRAMS):
- Complete 6-stage data flow pipeline - Line 305
- Oracle query processor architecture - Line 133
- CSR communication protocol - Line 452
- Storage SLOAD flow - Line 986
- Storage SSTORE flow - Line 1083
- Query processing pipeline - Line 852
- Call operation flow - Line 1972

### Execution Environment Diagrams

**[ExecutionEnvironments.md](./ExecutionEnvironments.md)**:
- EE vs. Bootloader relationship - Line 62
- Preemption flow (bootloader ↔ EE) - Line 446
- Opcode execution loop - Line 586

### System Layer Diagrams

**[SystemLayer.md](./SystemLayer.md)**:
- System as interface (EE → System → Resources) - Line 267
- IO subsystem architecture - Line 295

### Storage Diagrams

**[Storage.md](./Storage.md)**:
- Storage read path (warm vs. cold) - Line 263
- Storage write and rollback flow - Line 282

### Proof System Diagrams

**[ProofSystem.md](./ProofSystem.md)**:
- Proof generation workflow (4 phases) - Line 22
- RISC-V memory layout (ROM and RAM) - Line 115
- CSR communication protocol (guest ↔ host) - Line 243

### Key Types Diagrams

**[KeyTypes.md](./KeyTypes.md)**:
- Type interaction during transaction execution - Line 1689
- Storage operation flow - Line 1925
- Call operation flow - Line 1972

---

## Glossary

### A-D

**Allocator**: Memory allocation system. Talc for RISC-V bare-metal, Global for native execution.

**Blake2s**: Fast hash function used for flat storage key derivation. Produces 256-bit outputs.

**Bootloader**: Transaction orchestration layer that manages frame setup, execution, and teardown. Does not execute bytecode itself.

**CSR (Control & Status Register)**: RISC-V registers repurposed for oracle communication. Writes send queries, reads receive results.

**DA (Data Availability)**: L1 data submission ensuring state changes are publicly available. Includes storage diffs, events, logs, bytecode.

**Double Accounting**: Tracking both EVM gas (ergs) and native RISC-V resources to prevent denial-of-service attacks.

### E-H

**EE (Execution Environment)**: Bytecode interpreter abstraction. Current: EVM. Future: EraVM, Wasm.

**Ergs**: EVM gas units in zkSync OS. 1 EVM gas = 256 ergs.

**Flat Storage**: zkSync optimized storage model using Blake2s hash for key derivation, faster than Ethereum MPT.

### I-L

**IOSubsystem**: Interface for all I/O operations (storage, events, logs, balances). Abstracts storage model from execution.

**Keccak256**: Ethereum's primary hash function (not NIST SHA3). Used for addresses, Merkle trees, SHA3 opcode.

**L1 Messaging**: L2→L1 communication via LogsStorage. Enables cross-layer contract interaction.

### M-P

**Metadata**: Block and transaction context (number, timestamp, gas prices, etc.). Accessed by EVM opcodes.

**MPT (Merkle-Patricia Trie)**: Ethereum's storage structure. Supported for compatibility but slower than flat storage.

**Oracle**: External data provider using query-response protocol. Enables deterministic execution in RISC-V.

**Preemption**: Cooperative call handling where EE yields to bootloader for external calls instead of recursing directly.

**Precompiles**: EVM contracts at addresses 0x01-0x09 (and extended) providing cryptographic operations.

**Pubdata**: Data published to L1 including storage diffs, events, logs, and bytecode. Costs gas proportional to size.

### Q-T

**Query Processor**: Component handling specific oracle query types (metadata, storage, transactions, preimages).

**Resources**: Double accounting of EVM gas (ergs) and native RISC-V costs. Both limits must be satisfied.

**RISC-V**: Open instruction set used for zkSNARK proving. Enables deterministic execution and efficient circuits.

**State Transition Function**: Core logic processing blocks of transactions. Transforms old state + transactions → new state.

**SystemTypes**: Dependency injection trait parametrizing I/O, allocator, resources, metadata, and logger implementations.

**Talc**: Simple bump allocator for RISC-V bare-metal environment. Deterministic and bounded.

### U-Z

**Warm/Cold Access**: EIP-2929 optimization. Warm slots cost 100 gas (already accessed), cold slots cost 2100 gas (first access).

**Witness**: Execution trace (Vec&lt;u32&gt;) recording all oracle queries and responses. Fed to zkSNARK prover.

**ZK (Zero-Knowledge)**: Cryptographic proof system enabling trustless verification without re-execution. Proofs ~200KB regardless of computation size.

---

## Further Reading

### In-Repository Documentation

**Main Documentation** (`/docs/`):
- Additional architectural documentation
- Design decisions and rationale
- Integration guides

**Code Documentation**:
- Inline rustdoc comments in source files
- Module-level documentation in lib.rs files
- Use `cargo doc --open` to browse

### External Resources

**zkSync Official**:
- zkSync Documentation: [docs.zksync.io](https://docs.zksync.io)
- zkSync Era: [era.zksync.io](https://era.zksync.io)

**Related Projects**:
- **zkSNARK Prover (airbender)**: [github.com/matter-labs/zksync-airbender](https://github.com/matter-labs/zksync-airbender)
  - RISC-V proving system
  - Witness verification
  - Proof generation

- **Anvil-zkSync**: [github.com/matter-labs/anvil-zksync](https://github.com/matter-labs/anvil-zksync)
  - Local development node
  - RPC integration example
  - Testing infrastructure

**Ethereum Standards**:
- EVM Specification: [ethereum.github.io/yellowpaper/paper.pdf](https://ethereum.github.io/yellowpaper/paper.pdf)
- EIPs (Ethereum Improvement Proposals): [eips.ethereum.org](https://eips.ethereum.org)
  - EIP-150: Gas cost changes
  - EIP-1153: Transient storage
  - EIP-2929: Gas cost increases for state access
  - EIP-4844: Blob transactions

**RISC-V Resources**:
- RISC-V Specification: [riscv.org/technical/specifications](https://riscv.org/technical/specifications)
- RISC-V Assembly Reference: [riscv.org/wp-content/uploads/2017/05/riscv-spec-v2.2.pdf](https://riscv.org/wp-content/uploads/2017/05/riscv-spec-v2.2.pdf)

### Community and Support

**GitHub Repository**: [github.com/matter-labs/zksync-os](https://github.com/matter-labs/zksync-os)
- Issue tracker
- Pull requests
- Discussions

**Contributing**: See `/CONTRIBUTING.md` for contribution guidelines

---

## Document Statistics

**Total Documentation**:
- 11 markdown files
- ~13,300 total lines
- ~730 lines in this index

**Coverage**:
- 37 crates documented
- 15+ critical types fully documented
- 8 major query processors described
- 100+ EVM opcodes covered
- 9 EVM precompiles documented
- 6-stage data flow pipeline detailed
- 10+ ASCII diagrams

**Last Updated**: Generated for zkSync OS documentation suite

**Documentation Version**: Corresponds to zkSync OS repository state

---

*This index is the comprehensive navigation guide for all zkSync OS documentation. For questions or corrections, please refer to the GitHub repository or contact the zkSync OS team.*
