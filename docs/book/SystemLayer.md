# System Layer

## Table of Contents

1. [Introduction](#introduction)
2. [System Architecture](#system-architecture)
3. [IO Subsystem](#io-subsystem)
4. [Resource Accounting](#resource-accounting)
5. [Memory Management](#memory-management)
6. [Metadata System](#metadata-system)
7. [Logger System](#logger-system)

---

## Introduction

The **System Layer** is the foundational infrastructure of zkSync OS that provides core services to all execution environments. It acts as the interface between high-level execution environments (like the EVM interpreter) and low-level system resources (storage, metadata, memory allocation).

### Role of the System Layer

The system layer serves as the "operating system kernel" for zkSync OS, providing:

1. **I/O Services**: Storage reads/writes, event emission, L1 messaging
2. **Resource Accounting**: Double accounting of EVM gas and native RISC-V costs
3. **Memory Management**: Dynamic memory allocation with configurable allocators
4. **Metadata Access**: Block and transaction context information
5. **Logging**: Debug output and diagnostics

**Key Principle**: The system layer is **shared across all execution environments**. Whether executing EVM bytecode, future EraVM code, or Wasm, all EEs interact with the same system layer interface.

### Why a System Layer?

**Without a system layer**:
- Each EE would need to implement storage access, gas accounting, memory management
- No consistency across VMs
- Difficult to add new features (e.g., new storage models)
- Cannot enforce system-wide constraints

**With a system layer**:
- **Separation of concerns**: EEs focus on bytecode interpretation, system handles infrastructure
- **Consistency**: All EEs use same storage model, gas rules, metadata format
- **Extensibility**: Change storage model without touching EE code
- **Enforcement**: System layer enforces invariants (e.g., static context, resource limits)

---

## System Architecture

The system layer is built around two core abstractions: **SystemTypes** and **System**.

### SystemTypes Trait

**Location**: `zk_ee/src/system/mod.rs:59-74`

The `SystemTypes` trait is the **dependency injection** mechanism for zkSync OS. It defines all the associated types that a system implementation must provide, enabling compile-time polymorphism.

```rust
pub trait SystemTypes {
    /// Handles all side effects and information from the outside world.
    type IO: IOSubsystem<IOTypes = Self::IOTypes, Resources = Self::Resources>;

    /// Common system functions implementation(ecrecover, keccak256, ecadd, etc).
    type SystemFunctions: SystemFunctions<Self::Resources>;
    type SystemFunctionsExt: SystemFunctionsExt<Self::Resources>;

    type Logger: Logger + Default;

    // These are just shorthands. They are completely defined by the above types.
    type IOTypes: SystemIOTypesConfig;
    type Resources: Resources + Default;
    type Allocator: Allocator + Clone + Default;
    type Metadata: BasicMetadata<Self::IOTypes>;
}
```

**Associated Types**:

1. **IO**: The I/O subsystem implementation (typically `FullIO`)
   - Handles storage, events, logs, balance queries
   - Abstracts storage model and cost accounting
   - See [IO Subsystem](#io-subsystem) section

2. **SystemFunctions**: Core cryptographic and system functions
   - ecrecover (ECDSA signature recovery)
   - keccak256 (Keccak-256 hashing)
   - Precompiles (BN254 curve operations, etc.)
   - See [Cryptography.md](./Cryptography.md) for details

3. **SystemFunctionsExt**: Extended system functions
   - Additional cryptographic primitives
   - ZK-specific operations

4. **Logger**: Logging interface
   - Debug output
   - Execution traces
   - See [Logger System](#logger-system) section

5. **IOTypes**: Type configuration for addresses, storage keys, etc.
   - Default: `EthereumIOTypesConfig` (20-byte addresses, 32-byte keys/values)
   - Allows future support for different address/storage schemes

6. **Resources**: Resource accounting implementation
   - Tracks EVM gas (ergs) and native RISC-V costs
   - See [Resource Accounting](#resource-accounting) section

7. **Allocator**: Memory allocator type
   - Native: `std::alloc::Global` (standard Rust allocator)
   - RISC-V: `Talc` (bare-metal allocator)
   - See [Memory Management](#memory-management) section

8. **Metadata**: Block and transaction metadata provider
   - Block number, timestamp, gas prices, etc.
   - See [Metadata System](#metadata-system) section

**Why dependency injection?**

By parametrizing all major types via `SystemTypes`, zkSync OS can support:
- **Forward execution**: Native x86/ARM with database access
- **Proof generation**: RISC-V with oracle-based I/O
- **Testing**: Mock implementations for unit tests

Example instantiations:
```rust
// Forward execution (native)
struct ForwardSystemTypes;
impl SystemTypes for ForwardSystemTypes {
    type IO = FullIO<Global, ...>;           // Standard allocator
    type Allocator = Global;                  // std::alloc::Global
    type Metadata = BlockMetadataFromOracle;
    // ...
}

// Proof generation (RISC-V)
struct ProofSystemTypes;
impl SystemTypes for ProofSystemTypes {
    type IO = FullIO<Talc, ...>;             // Bare-metal allocator
    type Allocator = Talc;                    // Custom allocator
    type Metadata = BlockMetadataFromOracle;
    // ...
}
```

**Cross-references**: See [KeyTypes.md](./KeyTypes.md#systemtypes-trait) for more details on SystemTypes.

---

### System&lt;S: SystemTypes&gt; Struct

**Location**: `zk_ee/src/system/mod.rs:77-81`

The `System` struct is the **central hub** that execution environments interact with. It holds the I/O subsystem, metadata, and allocator, providing a unified interface for all system-level operations.

```rust
pub struct System<S: SystemTypes> {
    pub io: S::IO,
    pub metadata: S::Metadata,
    allocator: S::Allocator,
}
```

**Fields**:

1. **io**: I/O subsystem instance
   - Storage operations (SLOAD/SSTORE)
   - Event emission (LOG0-LOG4)
   - Balance queries (BALANCE, SELFBALANCE)
   - Nonce management
   - L1 messaging

2. **metadata**: Block and transaction metadata
   - Block context (number, timestamp, gas limit, etc.)
   - Transaction context (origin, gas price)
   - Used by opcodes like TIMESTAMP, NUMBER, CHAINID

3. **allocator**: Memory allocator
   - For dynamic allocations during execution
   - Different implementations for native vs. RISC-V

**Key Methods** (from `zk_ee/src/system/mod.rs:87-172`):

**Metadata Access**:
```rust
pub fn get_block_number(&self) -> u64;
pub fn get_timestamp(&self) -> u64;
pub fn get_chain_id(&self) -> u64;
pub fn get_gas_limit(&self) -> u64;
pub fn get_gas_price(&self) -> U256;
pub fn get_coinbase(&self) -> Address;
pub fn get_eip1559_basefee(&self) -> U256;
pub fn get_tx_origin(&self) -> Address;
pub fn get_blockhash(&self, block_number: u64) -> Result<Bytes32, InternalError>;
```

These methods provide EVM opcodes with block/transaction context:
- `TIMESTAMP` (0x42) calls `get_timestamp()`
- `NUMBER` (0x43) calls `get_block_number()`
- `CHAINID` (0x46) calls `get_chain_id()`
- `BLOCKHASH` (0x40) calls `get_blockhash(n)`
- etc.

**Frame Management**:
```rust
pub fn start_global_frame(&mut self) -> Result<SystemFrameSnapshot<S>, InternalError>;
pub fn finish_global_frame(
    &mut self,
    rollback_handle: Option<&SystemFrameSnapshot<S>>
) -> Result<(), InternalError>;
```

Used by bootloader to create snapshots before executing frames:
- `start_global_frame()`: Create snapshot of I/O state
- `finish_global_frame(None)`: Commit changes (on success)
- `finish_global_frame(Some(snapshot))`: Rollback changes (on revert)

**Transaction Context**:
```rust
pub fn set_tx_context(
    &mut self,
    tx_level_metadata: <S::Metadata as BasicMetadata<S::IOTypes>>::TransactionMetadata
);
```

Called by bootloader before each transaction to set transaction-specific metadata (origin, gas price, etc.).

**Other Utilities**:
```rust
pub fn get_logger(&self) -> S::Logger;
pub fn get_allocator(&self) -> S::Allocator;
pub fn net_pubdata_used(&self) -> Result<u64, InternalError>;
```

**Usage Pattern**:

Execution environments receive a mutable reference to `System<S>`:

```rust
// In EVM interpreter, executing TIMESTAMP opcode
pub fn execute_timestamp(
    &mut self,
    system: &mut System<S>
) -> Result<(), ExitCode> {
    let timestamp = system.get_timestamp();
    self.stack.push(U256::from(timestamp))?;
    Ok(())
}

// In EVM interpreter, executing SLOAD opcode
pub fn execute_sload(
    &mut self,
    system: &mut System<S>
) -> Result<(), ExitCode> {
    let key = self.stack.pop()?;
    let value = system.io.storage_read::<false>(
        ExecutionEnvironmentType::EVM,
        &mut self.gas.resources,
        &self.address,
        &key
    )?;
    self.stack.push(value)?;
    Ok(())
}
```

**System as the Interface**:

```
┌──────────────────────────────────────────────────────┐
│         Execution Environment (e.g., EVM)            │
│                                                       │
│  - Opcode execution                                  │
│  - Stack/memory management                           │
│  - Program counter                                   │
└──────────────────┬───────────────────────────────────┘
                   │ system.io.storage_read()
                   │ system.get_timestamp()
                   │ system.io.emit_event()
                   ↓
┌──────────────────────────────────────────────────────┐
│              System<S> (Central Hub)                 │
│                                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────┐ │
│  │ io: S::IO    │  │ metadata     │  │ allocator │ │
│  └──────┬───────┘  └──────┬───────┘  └─────┬─────┘ │
└─────────┼──────────────────┼─────────────────┼───────┘
          │                  │                 │
          ↓                  ↓                 ↓
   ┌─────────────┐    ┌────────────┐   ┌────────────┐
   │ IOSubsystem │    │ Block/Tx   │   │ Memory     │
   │             │    │ Context    │   │ Allocation │
   │ - Storage   │    │            │   │            │
   │ - Events    │    │ - Number   │   │ - Heap     │
   │ - Logs      │    │ - Timestamp│   │ - Stack    │
   │ - Balances  │    │ - ChainID  │   │            │
   └─────────────┘    └────────────┘   └────────────┘
```

**Cross-references**: See [KeyTypes.md](./KeyTypes.md#systemsystemtypes-struct) for complete field documentation.

---

## IO Subsystem

The **IO Subsystem** is the abstraction layer that handles all side effects: storage access, event emission, balance queries, and L1 messaging. It completely hides storage model implementation and cost accounting from execution environments.

### IOSubsystem Trait

**Location**: `zk_ee/src/system/io.rs:27-161`

The `IOSubsystem` trait defines the **user-facing interface** for I/O operations. It's the primary way execution environments interact with persistent state.

```rust
pub trait IOSubsystem: Sized {
    type Resources: Resources;
    type IOTypes: SystemIOTypesConfig;
    type StateSnapshot;

    // Storage operations
    fn storage_read<const TRANSIENT: bool>(...) -> Result<StorageValue, SystemError>;
    fn storage_write<const TRANSIENT: bool>(...) -> Result<(), SystemError>;

    // Balance operations
    fn get_nominal_token_balance(...) -> Result<NominalTokenValue, SystemError>;
    fn get_selfbalance(...) -> Result<NominalTokenValue, SystemError>;

    // Bytecode operations
    fn get_observable_bytecode_size(...) -> Result<u32, SystemError>;
    fn get_observable_bytecode_hash(...) -> Result<BytecodeHashValue, SystemError>;
    fn get_observable_bytecode(...) -> Result<&'static [u8], SystemError>;

    // Event emission
    fn emit_event(...) -> Result<(), SystemError>;

    // L1 messaging
    fn emit_l1_message(...) -> Result<Bytes32, SystemError>;

    // Self-destruct
    fn mark_for_deconstruction(...) -> Result<NominalTokenValue, DeconstructionSubsystemError>;

    // Frame management
    fn start_io_frame(&mut self) -> Result<StateSnapshot, InternalError>;
    fn finish_io_frame(&mut self, rollback_handle: Option<&StateSnapshot>) -> Result<(), InternalError>;

    // Nonce operations
    fn read_nonce(...) -> Result<u64, SystemError>;
    fn increment_nonce(...) -> Result<u64, NonceSubsystemError>;

    // Refund tracking
    fn get_refund_counter(&self) -> u32;
}
```

### Key Operations

#### 1. Storage Operations

**Storage Read** (SLOAD):
```rust
fn storage_read<const TRANSIENT: bool>(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
    key: &StorageKey,
) -> Result<StorageValue, SystemError>;
```

- **Generic over TRANSIENT**: Handles both persistent (`TRANSIENT=false`) and transient storage (`TRANSIENT=true`, EIP-1153)
- **Charges gas**: 100 gas for warm access, 2100 gas for cold access
- **Queries oracle**: On cold access, queries oracle for initial value
- **Returns**: 32-byte storage value

**Flow**:
```
SLOAD key
    ↓
system.io.storage_read::<false>(EVM, resources, address, key)
    ↓
Check warmness cache:
    Warm → Charge 100 gas, return cached value
    Cold → Query oracle, charge 2100 gas, cache value, return
    ↓
Return value to EVM interpreter
```

**Storage Write** (SSTORE):
```rust
fn storage_write<const TRANSIENT: bool>(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
    key: &StorageKey,
    value_to_write: &StorageValue,
) -> Result<(), SystemError>;
```

- **Complex gas calculation**: Depends on current/original values (EIP-2200)
  - Setting zero to non-zero: 20000 gas
  - Modifying non-zero: 5000 gas
  - Clearing slot (to zero): 5000 gas + 15000 refund
- **Tracks changes**: For rollback on revert
- **Calculates pubdata**: Tracks L1 pubdata costs

**Cross-references**: See [Storage.md](./Storage.md) for detailed storage model implementation.

#### 2. Balance Operations

**Get Balance** (BALANCE opcode):
```rust
fn get_nominal_token_balance(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
) -> Result<NominalTokenValue, SystemError>;
```

- **Charges gas**: 100 gas (warm) or 2600 gas (cold)
- **Queries account properties**: Includes balance, nonce, code hash
- **Returns**: Balance in wei (or native token units)

**Get Self Balance** (SELFBALANCE opcode):
```rust
fn get_selfbalance(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
) -> Result<NominalTokenValue, SystemError>;
```

- **Optimized**: Assumes address is already warm
- **Charges gas**: 5 gas (SELFBALANCE is cheaper than BALANCE)
- **Returns**: Own balance without additional warming cost

#### 3. Event Emission

**Emit Event** (LOG0-LOG4):
```rust
fn emit_event(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
    topics: &ArrayVec<EventKey, MAX_EVENT_TOPICS>,
    data: &[u8],
) -> Result<(), SystemError>;
```

- **Gas calculation**:
  - Base: 375 gas
  - Per topic: 375 gas
  - Per data byte: 8 gas
  - Total: `375 + 375*num_topics + 8*data_length`

- **Stores event**: In `events_storage` for inclusion in transaction receipt
- **Rollback support**: Events removed if frame reverts

**Example**:
```solidity
event Transfer(address indexed from, address indexed to, uint256 value);
emit Transfer(msg.sender, recipient, amount);
```

Generates:
```
LOG3 instruction
topics[0] = keccak256("Transfer(address,address,uint256)")  // Event signature
topics[1] = from (indexed)
topics[2] = to (indexed)
data = abi.encode(amount)
```

#### 4. L1 Messaging

**Emit L1 Message**:
```rust
fn emit_l1_message(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
    data: &[u8],
) -> Result<Bytes32, SystemError>;
```

- **Purpose**: Send message from L2 to L1
- **Usage**: Called by L1Messenger system contract (address 0x8008)
- **Returns**: Message data hash (for receipt)
- **Includes in pubdata**: Messages are published to L1 as calldata

**Flow**:
```
L2 Contract calls L1Messenger.sendToL1(message)
    ↓
L1Messenger system contract
    ↓
system.io.emit_l1_message(L1Messenger_addr, message_data)
    ↓
logs_storage.push_message(tx_number, address, data, keccak256(data))
    ↓
End of block: Publish logs to L1
    ↓
L1 can verify message via merkle proof
```

#### 5. Self-Destruct

**Mark for Deconstruction** (SELFDESTRUCT):
```rust
fn mark_for_deconstruction(
    &mut self,
    from_ee: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    at_address: &Address,
    nominal_token_beneficiary: &Address,
    in_constructor: bool,
) -> Result<NominalTokenValue, DeconstructionSubsystemError>;
```

- **Marks for destruction**: Account destroyed at end of transaction
- **Transfers balance**: All balance goes to beneficiary
- **Returns**: Amount transferred
- **Gas**: 5000 base + 25000 if creating new account for beneficiary

**Note**: SELFDESTRUCT doesn't immediately delete account. Actual destruction happens in `io.finish_tx()` after transaction completes.

### Snapshots and Rollback

The IO subsystem supports **frame snapshots** for rollback on revert:

**Start Frame**:
```rust
fn start_io_frame(&mut self) -> Result<StateSnapshot, InternalError>;
```

Creates snapshot of:
- Storage changes
- Transient storage
- Events
- Logs

**Finish Frame**:
```rust
fn finish_io_frame(
    &mut self,
    rollback_handle: Option<&StateSnapshot>
) -> Result<(), InternalError>;
```

- **`rollback_handle = None`**: Commit changes (frame succeeded)
- **`rollback_handle = Some(snapshot)`**: Rollback to snapshot (frame reverted)

**Example**:
```
Frame A executing:
    snapshot_A = system.io.start_io_frame()
    ↓
    SSTORE key1, value1  → Recorded in storage cache
    LOG2 topic1, topic2  → Recorded in events_storage
    ↓
    CALL Frame B:
        snapshot_B = system.io.start_io_frame()
        ↓
        SSTORE key2, value2  → Recorded
        LOG1 topic3          → Recorded
        ↓
        REVERT  → Frame B failed!
        ↓
        system.io.finish_io_frame(Some(snapshot_B))  → Rollback Frame B changes
        ↓
        key2 reverted, LOG1 removed
    ↓
    RETURN  → Frame A succeeded
    ↓
    system.io.finish_io_frame(None)  → Commit Frame A changes
    ↓
    key1 persisted, LOG2 persisted
```

### IO Subsystem Diagram

```
┌────────────────────────────────────────────────────────────────┐
│                    Execution Environment                        │
│                                                                 │
│  SLOAD, SSTORE, LOG0-LOG4, BALANCE, CALL, etc.                │
└────────────────────────┬───────────────────────────────────────┘
                         │
                         ↓ system.io.*()
┌────────────────────────────────────────────────────────────────┐
│                   IOSubsystem Trait                             │
│                                                                 │
│  User-facing interface (hides implementation)                  │
│  - storage_read/write                                          │
│  - emit_event                                                  │
│  - get_nominal_token_balance                                   │
│  - etc.                                                        │
└────────────────────────┬───────────────────────────────────────┘
                         │
                         ↓ Implementation
┌────────────────────────────────────────────────────────────────┐
│                  FullIO Implementation                          │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ storage: FlatStorageModel                                 │ │
│  │   - Warmness cache: Map<WarmStorageKey, WarmStorageValue>│ │
│  │   - Oracle queries for cold access                        │ │
│  │   - Tracks initial/current values for pubdata             │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ transient_storage: GenericTransientStorage (EIP-1153)     │ │
│  │   - Map<WarmStorageKey, Bytes32>                          │ │
│  │   - Cleared at end of transaction                         │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ events_storage: EventsStorage                             │ │
│  │   - List<EventContent>                                    │ │
│  │   - tx_number, address, topics, data                      │ │
│  │   - Rollback support                                      │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ logs_storage: LogsStorage                                 │ │
│  │   - List<LogContent>                                      │ │
│  │   - User messages (L2→L1)                                 │ │
│  │   - L1 tx status logs                                     │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ oracle: O (IOOracle trait)                                │ │
│  │   - Query external data                                   │ │
│  │   - Storage values, account properties, etc.              │ │
│  └──────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

---

### FullIO Implementation

**Location**: `basic_system/src/system_implementation/system/io_subsystem.rs:45-62`

`FullIO` is the concrete implementation of `IOSubsystem` used in both forward and proof execution.

```rust
pub struct FullIO<
    A: Allocator + Clone + Default,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
    SF: StackFactory<M>,
    const M: usize,
    O: IOOracle,
    const PROOF_ENV: bool,
> {
    pub(crate) storage: FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, M, PROOF_ENV>,
    pub(crate) transient_storage: GenericTransientStorage<WarmStorageKey, Bytes32, SF, M, A>,
    pub(crate) logs_storage: LogsStorage<SF, M, A>,
    pub(crate) events_storage: EventsStorage<MAX_EVENT_TOPICS, SF, M, A>,
    pub(crate) allocator: A,
    pub(crate) oracle: O,
    pub(crate) tx_number: u32,
    pub(crate) da_commitment_scheme: Option<DACommitmentScheme>,
}
```

**Generic Parameters**:

1. **A: Allocator**: Memory allocator type
   - Native: `Global` (std::alloc::Global)
   - RISC-V: `Talc` (bare-metal allocator)

2. **R: Resources**: Resource accounting type
   - Tracks ergs (EVM gas) and native (RISC-V cycles)

3. **P: StorageAccessPolicy**: Cost policy for storage access
   - Determines gas costs for warm/cold reads/writes

4. **SF: StackFactory<M>**: Factory for creating snapshot stacks
   - Enables rollback on revert

5. **M: usize**: Maximum frame depth
   - Typically 1024 (EVM limit)

6. **O: IOOracle**: Oracle implementation
   - Forward: Database-backed oracle
   - Proof: CSR-based oracle (RISC-V)

7. **PROOF_ENV: bool**: Compile-time flag
   - `true` for proof generation (optimizations)
   - `false` for forward execution

**Field Details**:

- **storage**: Flat storage model implementation
  - Maps (address, key) → value
  - Tracks warm/cold access for gas calculation
  - See [Storage.md](./Storage.md) for details

- **transient_storage**: EIP-1153 transient storage
  - Transaction-scoped storage
  - Cleared at end of each transaction
  - Not persisted to state

- **logs_storage**: L2→L1 logs
  - User messages (via L1Messenger)
  - L1 transaction status logs
  - Published to L1 as calldata

- **events_storage**: EVM events
  - LOG0-LOG4 opcodes
  - Included in transaction receipts
  - Queryable via eth_getLogs RPC

- **allocator**: Memory allocator
  - For dynamic allocations
  - Different implementations for native vs. RISC-V

- **oracle**: External data provider
  - Storage values
  - Account properties (balance, code, nonce)
  - Block hashes

- **tx_number**: Current transaction number in block
  - Used for event/log ordering

- **da_commitment_scheme**: Data availability scheme
  - Determines how pubdata is committed
  - Options: Calldata, Blobs (EIP-4844)

**Implementation Pattern**:

All `IOSubsystem` methods delegate to appropriate sub-components:

```rust
impl IOSubsystem for FullIO<...> {
    fn storage_read<const TRANSIENT: bool>(...) -> Result<StorageValue, SystemError> {
        if TRANSIENT {
            self.transient_storage.read(key)
        } else {
            self.storage.storage_read(ee_type, resources, address, key)
        }
    }

    fn emit_event(...) -> Result<(), SystemError> {
        self.events_storage.push_event(tx_number, address, topics, data)
    }

    // ... other methods delegate similarly
}
```

**Cross-references**: See [KeyTypes.md](./KeyTypes.md#fullio-struct) for complete field documentation.

---

## Resource Accounting

zkSync OS uses a **double accounting** model to track computational resources:

1. **EVM Gas (Ergs)**: Ethereum-compatible gas costs
2. **Native Resources**: RISC-V cycle costs

**Why both?**
- **Ergs**: Ensures Ethereum compatibility (same gas costs as mainnet)
- **Native**: Prevents denial-of-service via operations cheap in EVM but expensive in RISC-V

### Resources Trait

**Location**: `zk_ee/src/system/resources.rs:128-172`

The `Resources` trait defines the interface for tracking and charging computational resources.

```rust
pub trait Resources: 'static + Sized + Clone + core::fmt::Debug + PartialEq + Eq + Resource {
    /// Type of native computational resource.
    type Native: Resource + Computational;

    /// Constructor from EE resource, all other resources are set to empty.
    fn from_ergs(ergs: Ergs) -> Self;

    /// Constructor from native resource, all other resources are set to empty.
    fn from_native(native: Self::Native) -> Self;

    /// Constructor from all sub-resources.
    fn from_ergs_and_native(ergs: Ergs, native: Self::Native) -> Self;

    /// Increments the EE resource.
    fn add_ergs(&mut self, to_add: Ergs);

    /// Gets the available ergs (EE resource).
    fn ergs(&self) -> Ergs;

    /// Gets the available native.
    fn native(&self) -> Self::Native;

    /// Consumes all remaining EE resource.
    fn exhaust_ergs(&mut self);

    /// Move all the native resources from [self] to [other].
    fn give_native_to(&mut self, other: &mut Self);

    /// Make a copy of [self], replacing it with the empty resources.
    fn take(&mut self) -> Self;

    /// Run a computation with "infinite" ergs but normal native resources.
    fn with_infinite_ergs<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R;
}
```

### Ergs (EVM Gas)

**Location**: `zk_ee/src/system/resources.rs:60-119`

Ergs are the EVM gas unit used in zkSync OS.

```rust
#[derive(Clone, Copy, core::fmt::Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ergs(pub u64);
```

**Conversion**: 1 EVM gas = 256 ergs

**Why ergs?** The name "ergs" (a unit of energy in physics) distinguishes zkSync OS gas from standard EVM gas, even though the conversion is straightforward.

**Operations**:
```rust
impl Resource for Ergs {
    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError> {
        if self.0 < to_charge.0 {
            self.0 = 0;
            return Err(out_of_ergs_error!());
        }
        self.0 -= to_charge.0;
        Ok(())
    }

    fn reclaim(&mut self, to_reclaim: Self) {
        self.0 += to_reclaim.0
    }

    // ... other methods
}
```

**Usage**:
```rust
// Charge 3 gas for ADD opcode
let ergs_cost = Ergs(3 * 256);  // 3 gas * 256 = 768 ergs
resources.charge(&Resources::from_ergs(ergs_cost))?;
```

### Native Resources (RISC-V Cycles)

Native resources track actual computational cost in the RISC-V execution environment.

**Type**: Implementation-defined (typically `u64`)

**Purpose**:
- Track RISC-V cycle consumption
- Prevent DoS via operations cheap in EVM but expensive in RISC-V
- Examples:
  - Memory allocations
  - Bytecode preprocessing
  - Storage operations
  - Cryptographic operations

**Code locations**: Native resource constants are defined in `evm_interpreter/src/native_resource_constants/`

### Double Accounting Model

Most operations charge **both** ergs and native resources:

```rust
// Example: SLOAD cold access
let ergs = Ergs(2100 * 256);  // 2100 gas = 537,600 ergs
let native = Native::from_computational(5000);  // ~5000 RISC-V cycles
resources.charge(&Resources::from_ergs_and_native(ergs, native))?;
```

**Both limits enforced**: Whichever exhausts first causes OutOfGas error.

**Why both?**
- EVM gas ensures Ethereum compatibility
- Native cost ensures RISC-V execution completes in reasonable time
- An operation might be cheap in EVM gas but expensive in RISC-V (e.g., memory operations)

### Gas Calculation Examples

#### EVM Opcodes

**Simple opcodes**:
```rust
ADD:    ergs = 3 * 256, native = 10
MUL:    ergs = 5 * 256, native = 15
SLOAD:  ergs = 100 * 256 (warm) or 2100 * 256 (cold), native = 1000 or 5000
```

**Storage access** (warm vs. cold):
```rust
// Warm SLOAD (slot already accessed this tx)
ergs = 100 * 256 = 25,600
native = 1000

// Cold SLOAD (first access this tx)
ergs = 2100 * 256 = 537,600
native = 5000
```

**Memory operations**:
```rust
MLOAD:  ergs = 3 * 256 + memory_expansion_cost, native = 50
MSTORE: ergs = 3 * 256 + memory_expansion_cost, native = 50
```

#### Storage Gas (SSTORE)

SSTORE gas is complex (EIP-2200):

1. **Setting zero to non-zero** (new slot):
   ```
   ergs = 20000 * 256 = 5,120,000
   native = 10000
   ```

2. **Modifying non-zero** (existing slot):
   ```
   ergs = 5000 * 256 = 1,280,000
   native = 5000
   ```

3. **Clearing slot** (to zero):
   ```
   ergs = 5000 * 256 = 1,280,000
   native = 5000
   refund = 15000 gas (applied at tx end)
   ```

4. **No-op** (setting to same value):
   ```
   ergs = 100 * 256 = 25,600 (warm access)
   native = 1000
   ```

### Resource Flow Diagram

```
Transaction starts with resources:
    ergs = tx.gas_limit * 256
    native = block_native_limit
    ↓
    ┌──────────────────────────────────┐
    │ For each opcode:                 │
    │                                  │
    │ 1. Calculate ergs cost (EVM)     │
    │ 2. Calculate native cost (RISC-V)│
    │ 3. Charge both resources         │
    │    ↓                             │
    │    resources.charge(&Resources:: │
    │      from_ergs_and_native(       │
    │        ergs, native))?           │
    │    ↓                             │
    │ 4. Check if either exhausted     │
    │    → OutOfGas error if so        │
    │    ↓                             │
    │ 5. Continue execution            │
    └──────────────────────────────────┘
    ↓
At transaction end:
    ↓
    Unused ergs → Refund to user
    Native consumed → Recorded for proof
```

### Special Pattern: with_infinite_ergs

When the system does work on behalf of the EE (already paid for in ergs), but still needs to track native resources:

```rust
resources.with_infinite_ergs(|inf_resources| {
    // System operation that shouldn't charge more EVM gas
    system.io.deploy_code(inf_resources, address, bytecode)?;
    // This charges native but not ergs
});
```

**Use cases**:
- Bytecode deployment (gas already charged by CREATE)
- Internal system operations
- Post-call processing

**Implementation**:
```rust
fn with_infinite_ergs<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
    let saved_ergs = self.ergs();
    self.set_ergs(Ergs::FORMAL_INFINITE);
    let result = f(self);
    self.set_ergs(saved_ergs);
    result
}
```

**Cross-references**: See [KeyTypes.md](./KeyTypes.md#resources-trait) for detailed resource type documentation.

---

## Memory Management

zkSync OS supports multiple memory allocators depending on the execution environment.

### Allocator Types

1. **Native Execution** (x86/ARM):
   ```rust
   type Allocator = std::alloc::Global;
   ```
   - Uses standard Rust allocator
   - Full OS support (malloc/free)
   - Efficient for forward execution

2. **RISC-V Execution** (bare-metal):
   ```rust
   type Allocator = Talc;
   ```
   - Custom bare-metal allocator
   - No OS, no standard library
   - Designed for deterministic execution

### Heap and Stack Boundaries

**EVM Memory (Heap)**:
- Managed by `SliceVec<'a, u8>` (sparse byte vector)
- Grows on-demand during execution
- Charges quadratic gas for expansion
- Frame-local (not persisted)

**Stack**:
- Fixed size per frame
- RISC-V: Configured at compile time
- Native: OS-managed

### RISC-V Memory Layout

In RISC-V bare-metal mode, memory is statically laid out:

```
┌────────────────────────────────────────┐ 0x00000000
│           Reserved (null page)          │
├────────────────────────────────────────┤
│              Code (.text)               │
│         (zkSync OS executable)          │
├────────────────────────────────────────┤
│          Read-only data (.rodata)       │
├────────────────────────────────────────┤
│              Data (.data)               │
│         (initialized globals)           │
├────────────────────────────────────────┤
│               BSS (.bss)                │
│        (uninitialized globals)          │
├────────────────────────────────────────┤
│                 Heap                    │
│         (managed by Talc)               │
│                  ↓                      │
│             (grows down)                │
├────────────────────────────────────────┤
│                 ...                     │
├────────────────────────────────────────┤
│             (grows up)                  │
│                  ↑                      │
│                Stack                    │
├────────────────────────────────────────┤ Stack top
│                                         │
└────────────────────────────────────────┘ 0xFFFFFFFF
```

**Key Points**:
- **No memory protection**: Bare-metal environment
- **Deterministic allocations**: Same input → same memory layout
- **Stack overflow detection**: Via explicit checks
- **Heap managed by Talc**: Custom allocator for RISC-V

**Cross-references**: See [ProofSystem.md](./ProofSystem.md) for RISC-V execution details.

---

## Metadata System

The metadata system provides block and transaction context to execution environments.

### BasicMetadata Trait

**Location**: `zk_ee/src/system/metadata/mod.rs`

The `BasicMetadata` trait defines the interface for accessing block and transaction metadata.

```rust
pub trait BasicMetadata<IOTypes: SystemIOTypesConfig> {
    type TransactionMetadata;

    // Block-level metadata
    fn chain_id(&self) -> u64;
    fn block_number(&self) -> u64;
    fn block_historical_hash(&self, depth: u64) -> Option<Bytes32>;
    fn block_timestamp(&self) -> u64;
    fn block_gas_limit(&self) -> u64;
    fn eip1559_basefee(&self) -> U256;
    fn coinbase(&self) -> IOTypes::Address;

    // Transaction-level metadata
    fn tx_origin(&self) -> IOTypes::Address;
    fn tx_gas_price(&self) -> U256;

    // Mutation
    fn set_transaction_metadata(&mut self, tx_metadata: Self::TransactionMetadata);
}
```

**Usage by EVM opcodes**:
- `CHAINID` (0x46) → `chain_id()`
- `NUMBER` (0x43) → `block_number()`
- `BLOCKHASH` (0x40) → `block_historical_hash(depth)`
- `TIMESTAMP` (0x42) → `block_timestamp()`
- `GASLIMIT` (0x45) → `block_gas_limit()`
- `BASEFEE` (0x48) → `eip1559_basefee()`
- `COINBASE` (0x41) → `coinbase()`
- `ORIGIN` (0x32) → `tx_origin()`
- `GASPRICE` (0x3a) → `tx_gas_price()`

### BlockMetadataFromOracle

**Location**: `zk_ee/src/system/metadata/zk_metadata.rs:108-127`

`BlockMetadataFromOracle` is the concrete implementation of block metadata used in zkSync OS.

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockMetadataFromOracle {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hashes: BlockHashes,           // Array of 256 previous block hashes
    pub timestamp: u64,
    pub eip1559_basefee: U256,
    pub pubdata_price: U256,                 // zkSync-specific
    pub native_price: U256,                  // zkSync-specific
    pub coinbase: B160,
    pub gas_limit: u64,
    pub pubdata_limit: u64,                  // zkSync-specific
    pub mix_hash: U256,                      // prevRandao (PoS)
}
```

**Field Details**:

1. **chain_id**: Chain identifier
   - Ethereum mainnet: 1
   - zkSync Era: 324
   - Used by CHAINID opcode, transaction signing

2. **block_number**: Current block number
   - Monotonically increasing
   - Used by NUMBER opcode, BLOCKHASH lookups

3. **block_hashes**: Array of 256 previous block hashes
   - Used by BLOCKHASH(n) opcode
   - Only blocks in range `[current - 256, current)` available
   - Index: `256 - (current - n)` holds hash for block `n`

4. **timestamp**: Unix timestamp in seconds
   - Used by TIMESTAMP opcode
   - Must be ≥ parent block timestamp

5. **eip1559_basefee**: EIP-1559 base fee per gas
   - Dynamic fee (adjusts based on block fullness)
   - Used by BASEFEE opcode
   - Burned (not given to validator)

6. **pubdata_price**: zkSync-specific L1 data cost
   - Cost per byte of pubdata
   - Covers L1 calldata/blob costs

7. **native_price**: zkSync-specific token pricing
   - Used for gas pricing calculations

8. **coinbase**: Block beneficiary address
   - Used by COINBASE opcode
   - Receives transaction fees (excluding base fee)

9. **gas_limit**: Block gas limit
   - Used by GASLIMIT opcode
   - Maximum gas for all transactions in block

10. **pubdata_limit**: zkSync-specific pubdata limit
    - Maximum bytes of pubdata per block

11. **mix_hash**: Source of randomness
    - PoW: Mining difficulty
    - PoS: Beacon chain randomness (prevRandao)
    - Used by DIFFICULTY/PREVRANDAO opcode

**Initialization**:

Metadata is provided by the oracle at block start:

```rust
// Forward execution
let metadata = oracle.query_block_metadata()?;

// Proof execution (RISC-V)
let metadata = read_metadata_from_csr();  // Read from Control & Status Registers
```

**Transaction Context Updates**:

Before each transaction, bootloader updates transaction-level metadata:

```rust
system.set_tx_context(TransactionMetadata {
    origin: tx.from,
    gas_price: tx.gas_price,
});
```

### ZK-Specific Metadata

**Location**: `zk_ee/src/system/metadata/zk_metadata.rs`

`ZkSpecificPricingMetadata` trait extends `BasicMetadata` with zkSync-specific pricing information:

```rust
pub trait ZkSpecificPricingMetadata: BasicMetadata<EthereumIOTypesConfig> {
    fn pubdata_price(&self) -> U256;
    fn native_price(&self) -> U256;
    fn pubdata_limit(&self) -> u64;
}
```

**Purpose**: Track L1 costs and pubdata limits specific to zkSync's rollup architecture.

**Cross-references**: See [KeyTypes.md](./KeyTypes.md#blockmetadatafromoracle) for complete field documentation.

---

## Logger System

The logger system provides debugging and diagnostic output during execution.

### Logger Trait

**Location**: `zk_ee/src/system/logger.rs`

```rust
pub trait Logger: Default {
    fn log(&self, message: &str);
    fn log_debug(&self, message: &str);
    fn log_error(&self, message: &str);
}
```

**Implementations**:

1. **NoopLogger**: Does nothing (production)
   ```rust
   struct NoopLogger;
   impl Logger for NoopLogger {
       fn log(&self, _message: &str) {}
       fn log_debug(&self, _message: &str) {}
       fn log_error(&self, _message: &str) {}
   }
   ```

2. **StdoutLogger**: Prints to stdout (native testing)
   ```rust
   struct StdoutLogger;
   impl Logger for StdoutLogger {
       fn log(&self, message: &str) {
           println!("{}", message);
       }
       // ...
   }
   ```

3. **UartLogger**: Outputs to UART (RISC-V)
   ```rust
   struct UartLogger;
   impl Logger for UartLogger {
       fn log(&self, message: &str) {
           uart_write(message.as_bytes());
       }
       // ...
   }
   ```

### UART in RISC-V

In RISC-V bare-metal mode, logging goes through the **UART** (Universal Asynchronous Receiver-Transmitter) peripheral.

**How it works**:
1. Logger formats message as bytes
2. Writes bytes to UART transmit register
3. RISC-V simulator captures UART output
4. Output displayed in simulator console

**Usage**:
```rust
let logger = system.get_logger();
logger.log("Executing transaction...");
logger.log_debug(&format!("Gas remaining: {}", gas_remaining));
logger.log_error("Out of gas!");
```

**Configuration**:
- **Production**: `Logger = NoopLogger` (no overhead)
- **Testing**: `Logger = StdoutLogger` or `UartLogger`
- Compile-time selection via `SystemTypes` trait

---

## Summary

The System Layer provides the foundational infrastructure for zkSync OS:

1. **System Architecture**:
   - `SystemTypes` trait: Dependency injection for all system types
   - `System<S>` struct: Central hub connecting I/O, metadata, and allocator

2. **IO Subsystem**:
   - `IOSubsystem` trait: User-facing interface for all I/O operations
   - `FullIO` implementation: Concrete implementation with storage, events, logs
   - Snapshots and rollback: Support for frame revert on error

3. **Resource Accounting**:
   - Double accounting: EVM gas (ergs) + native RISC-V costs
   - `Resources` trait: Unified interface for resource management
   - Prevents DoS via cheap-in-EVM-but-expensive-in-RISC-V operations

4. **Memory Management**:
   - Multiple allocators: `Global` (native) vs. `Talc` (RISC-V)
   - Heap and stack boundaries
   - RISC-V bare-metal layout

5. **Metadata System**:
   - `BasicMetadata` trait: Block and transaction context interface
   - `BlockMetadataFromOracle`: Concrete implementation with all EVM metadata
   - ZK-specific extensions: Pubdata pricing and limits

6. **Logger System**:
   - `Logger` trait: Configurable logging interface
   - Multiple implementations: Noop, Stdout, UART
   - RISC-V UART output for bare-metal diagnostics

**Key Takeaway**: The system layer is **shared across all execution environments**, providing consistent infrastructure regardless of the bytecode being executed.

**Cross-references**:
- [KeyTypes.md](./KeyTypes.md): Detailed documentation of system types
- [ExecutionEnvironments.md](./ExecutionEnvironments.md): How EEs use the system layer
- [Storage.md](./Storage.md): Storage model implementation details
- [DataFlow.md](./DataFlow.md): Complete data flow through the system
- [ProofSystem.md](./ProofSystem.md): RISC-V execution and memory layout
