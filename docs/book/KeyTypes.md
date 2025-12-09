# Key Types Reference

## Introduction

This document serves as a comprehensive reference for the most critical data structures in zkSync OS. These types form the backbone of the system's architecture, representing core abstractions for execution, I/O operations, resource management, and state tracking.

**Purpose**: This reference enables developers to quickly understand:
- The role of each critical type in the system
- Where each type is defined in the codebase
- Key fields and methods of each type
- How types interact in the overall data flow

**How to use this document**:
- Use the table of contents to jump to specific type categories
- Follow cross-references to related documentation (DataFlow.md, ExecutionEnvironments.md, SystemLayer.md)
- Refer to exact file paths and line numbers for implementation details
- Review the data flow diagram to understand type relationships

---

## Table of Contents

1. [System Architecture Types](#system-architecture-types)
2. [Execution Environment Types](#execution-environment-types)
3. [I/O & Resource Types](#io--resource-types)
4. [Call Handling Types](#call-handling-types)
5. [Storage Types](#storage-types)
6. [Metadata Types](#metadata-types)
7. [Events & Logs Types](#events--logs-types)
8. [Error Types](#error-types)
9. [Data Flow Relationships](#data-flow-relationships)

---

## System Architecture Types

### SystemTypes Trait

**Location**: `zk_ee/src/system/mod.rs:59-74`

**Purpose**: The `SystemTypes` trait is the foundational dependency injection mechanism for zkSync OS. It defines all the associated types that a system implementation must provide, enabling compile-time polymorphism across different execution modes (forward running, proof generation) and system configurations.

**Definition**:
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
- **IO**: Implementation of the I/O subsystem for storage, events, and resource accounting
- **SystemFunctions**: Core cryptographic and system functions (ecrecover, keccak256, precompiles)
- **SystemFunctionsExt**: Extended system functions
- **Logger**: Logging interface for debugging and diagnostics
- **IOTypes**: Configuration for address/storage types (Ethereum-compatible by default)
- **Resources**: Resource accounting implementation (EVM gas + native RISC-V costs)
- **Allocator**: Memory allocator for heap management
- **Metadata**: Block and transaction metadata provider

**Usage in Data Flow**: This trait parametrizes almost all other types in the system. Every major component (`System<S>`, `Interpreter<S>`, execution environments) is generic over `S: SystemTypes`, enabling different implementations for:
- Forward execution (native x86/ARM with database access)
- Proof generation (RISC-V with oracle-based I/O)
- Testing (with mock implementations)

**Cross-references**: See [SystemLayer.md](./SystemLayer.md) for detailed system architecture.

---

### System&lt;S: SystemTypes&gt; Struct

**Location**: `zk_ee/src/system/mod.rs:77-81`

**Purpose**: The `System` struct is the central hub that execution environments interact with. It holds the I/O subsystem, metadata, and allocator, providing a unified interface for all system-level operations. Think of it as the "operating system kernel" that execution environments (like the EVM interpreter) call into.

**Definition**:
```rust
pub struct System<S: SystemTypes> {
    pub io: S::IO,
    pub metadata: S::Metadata,
    allocator: S::Allocator,
}
```

**Key Fields**:
- **io**: The I/O subsystem handling storage reads/writes, event emission, balance transfers
- **metadata**: Block and transaction metadata (block number, timestamp, gas prices, etc.)
- **allocator**: Memory allocator for dynamic allocations during execution

**Key Methods** (selected from mod.rs:87-172):
- `get_logger()`: Returns logger for debugging
- `get_tx_origin()`: Returns transaction origin address
- `get_block_number()`: Returns current block number
- `get_blockhash(block_number)`: Returns hash of historical block
- `get_chain_id()`: Returns chain ID
- `get_coinbase()`: Returns block coinbase address
- `get_eip1559_basefee()`: Returns EIP-1559 base fee
- `get_gas_limit()`: Returns block gas limit
- `get_gas_price()`: Returns transaction gas price
- `get_timestamp()`: Returns block timestamp
- `set_tx_context()`: Updates transaction-level metadata
- `start_global_frame()`: Starts a new execution frame with snapshot
- `finish_global_frame()`: Finishes frame, optionally rolling back to snapshot
- `deploy_bytecode()`: Deploys contract bytecode at address

**Usage in Data Flow**: The `System` is passed to execution environments during frame execution. When an opcode like SLOAD, CALL, or LOG is executed, the interpreter calls methods on `system.io` to perform the operation. When opcodes like TIMESTAMP or BLOCKHASH are executed, the interpreter queries `system.metadata`.

**Cross-references**: See [SystemLayer.md](./SystemLayer.md) for detailed system operations.

---

## Execution Environment Types

### ExecutionEnvironment Trait

**Location**: `zk_ee/src/system/execution_environment/mod.rs:40-101`

**Purpose**: The `ExecutionEnvironment` trait defines the interface that all execution environments must implement. This abstraction enables zkSync OS to support multiple execution environments (EVM, future EraVM, future Wasm) with the same system layer. The trait uses a cooperative multitasking model where execution environments yield control back to the system at "preemption points" (e.g., external calls).

**Key Methods**:

1. **`new(system: &mut System<S>) -> Result<Self, Self::SubsystemError>`**
   - Initializes a new empty execution environment state
   - Called once when setting up the execution environment

2. **`before_executing_frame(...) -> Result<bool, Self::SubsystemError>`**
   ```rust
   fn before_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
       system: &mut System<S>,
       frame_state: &mut ExecutionEnvironmentLaunchParams<'i, S>,
       tracer: &mut impl Tracer<S>,
   ) -> Result<bool, Self::SubsystemError>
   ```
   - Pre-checks and operations before frame execution
   - Operations here are NOT rolled back if execution fails
   - Used for: nonce increments, balance transfers, gas pre-charging
   - Returns: `Ok(true)` if frame should execute, `Ok(false)` to skip

3. **`start_executing_frame(...) -> Result<ExecutionEnvironmentPreemptionPoint, Self::SubsystemError>`**
   ```rust
   fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
       &'a mut self,
       system: &mut System<S>,
       frame_state: ExecutionEnvironmentLaunchParams<'i, S>,
       heap: SliceVec<'h, u8>,
       tracer: &mut impl Tracer<S>,
   ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
   ```
   - Begins execution of a frame with given initial state
   - Executes until hitting a preemption point (external call, completion, error)
   - Returns: Preemption point for the bootloader to handle

4. **`continue_after_preemption(...) -> Result<ExecutionEnvironmentPreemptionPoint, Self::SubsystemError>`**
   ```rust
   fn continue_after_preemption<'a, 'res: 'ee>(
       &'a mut self,
       system: &mut System<S>,
       returned_resources: S::Resources,
       call_request_result: CallResult<'res, S>,
       tracer: &mut impl Tracer<S>,
   ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
   ```
   - Continues execution after handling a preemption (e.g., external call returned)
   - Receives the call result and unused resources
   - Resumes execution until next preemption point

5. **`calculate_resources_passed_in_external_call(...) -> Result<S::Resources, Self::SubsystemError>`**
   ```rust
   fn calculate_resources_passed_in_external_call(
       resources_in_caller_frame: &mut S::Resources,
       call_request: &ExternalCallRequest<S>,
       callee_account_properties: &CalleeAccountProperties,
   ) -> Result<S::Resources, Self::SubsystemError>
   ```
   - EE decides how many resources to provide to callee frame
   - Implements EIP-150 (63/64 rule for EVM)
   - Called by bootloader before starting callee frame

**Preemption Model**: Instead of directly calling into other frames, execution environments return control to the bootloader with an `ExecutionEnvironmentPreemptionPoint::CallRequest`. The bootloader then:
1. Validates the call
2. Sets up the new frame
3. Calls `start_executing_frame` on the callee
4. Returns the result via `continue_after_preemption` to the caller

This model enables proper resource accounting, frame management, and tracer integration.

**Usage in Data Flow**: When executing a transaction:
1. Bootloader calls `before_executing_frame` (transfer value, increment nonce)
2. Bootloader calls `start_executing_frame` to begin execution
3. EE executes opcodes until CALL/CREATE/completion
4. EE returns preemption point to bootloader
5. Bootloader processes the preemption (e.g., recursively executes callee frame)
6. Bootloader calls `continue_after_preemption` with result
7. Loop until final completion

**Cross-references**: See [ExecutionEnvironments.md](./ExecutionEnvironments.md) for detailed execution flow.

---

### Interpreter&lt;'a, S: SystemTypes&gt; Struct

**Location**: `evm_interpreter/src/lib.rs:81-112`

**Purpose**: The `Interpreter` struct is the concrete implementation of the EVM execution environment. It maintains all the execution state for a single EVM frame, including the stack, memory (heap), program counter, and call parameters. This is the core EVM interpreter that executes EVM bytecode opcode by opcode.

**Definition**:
```rust
pub struct Interpreter<'a, S: SystemTypes> {
    /// Instruction pointer.
    pub instruction_pointer: usize,
    /// Implementation of gas accounting on top of system resources.
    pub gas: Gas<S>,
    /// Stack.
    pub stack: EvmStack<S::Allocator>,
    /// Caller address
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// Contract information and invoking data
    pub address: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// calldata
    pub calldata: &'a [u8],
    /// returndata is available from here if it exists
    pub returndata: &'a [u8],
    /// Heap that belongs to this interpreter, can be resided
    pub heap: SliceVec<'a, u8>,
    /// returndata location serves to save range information at various points
    pub returndata_location: Range<usize>,
    /// Bytecode
    pub bytecode: &'a [u8],
    /// Preprocessing result
    pub bytecode_preprocessing: BytecodePreprocessingData<'a, S::Allocator>,
    /// Call value
    pub call_value: U256,
    /// Is interpreter call static.
    pub is_static: bool,
    /// Is interpreter call executing construction code.
    pub is_constructor: bool,
    /// Indicating that EE is waiting for the result of some operation from the OS.
    pub pending_os_request: Option<PendingOsRequest<S>>,
}
```

**Key Fields**:
- **instruction_pointer**: Current position in bytecode (program counter)
- **gas**: Wrapper managing both EVM gas and native resource accounting
- **stack**: EVM stack implementation (max 1024 elements)
- **caller**: Address that called this contract (msg.sender)
- **address**: Address of the executing contract (address(this))
- **calldata**: Input data to the call (msg.data)
- **returndata**: Return data from last external call
- **heap**: EVM memory (sparse, grows on demand)
- **returndata_location**: Range in heap where return data is stored
- **bytecode**: Executable bytecode
- **bytecode_preprocessing**: Cached jumpdest bitmap for JUMP validation
- **call_value**: ETH/native token value sent with call (msg.value)
- **is_static**: Whether this is a STATICCALL context (no state changes allowed)
- **is_constructor**: Whether executing constructor code (CREATE/CREATE2)
- **pending_os_request**: Tracks pending CALL/CREATE waiting for OS response

**Execution Flow**:
1. Bootloader creates `Interpreter` with initial state
2. Interpreter enters main execution loop
3. For each opcode:
   - Fetch opcode at `instruction_pointer`
   - Execute opcode (stack operations, memory operations, system calls)
   - Update `instruction_pointer`
   - Check gas (exit if out of gas)
4. On CALL/CREATE:
   - Set `pending_os_request` to track the call type
   - Return `ExecutionEnvironmentPreemptionPoint::CallRequest` to bootloader
5. On completion:
   - Return `ExecutionEnvironmentPreemptionPoint::End` with result

**Opcode Categories** (implemented in `evm_interpreter/src/opcodes/`):
- **Stack**: PUSH, POP, DUP, SWAP
- **Arithmetic**: ADD, SUB, MUL, DIV, MOD, EXP
- **Comparison**: LT, GT, EQ, ISZERO
- **Bitwise**: AND, OR, XOR, NOT, SHL, SHR, SAR, BYTE
- **Memory**: MLOAD, MSTORE, MSTORE8, MSIZE
- **Storage**: SLOAD, SSTORE
- **Control**: JUMP, JUMPI, PC, JUMPDEST, STOP, RETURN, REVERT
- **System**: CALL, STATICCALL, DELEGATECALL, CALLCODE, CREATE, CREATE2
- **Logging**: LOG0, LOG1, LOG2, LOG3, LOG4
- **Context**: ADDRESS, CALLER, CALLVALUE, CALLDATALOAD, CALLDATASIZE, RETURNDATASIZE, CODESIZE, etc.
- **Block**: BLOCKHASH, COINBASE, TIMESTAMP, NUMBER, DIFFICULTY (PREVRANDAO), GASLIMIT, CHAINID, SELFBALANCE, BASEFEE

**Usage in Data Flow**: The interpreter is the workhorse of EVM execution:
```
Bootloader
    ↓
create Interpreter with frame params
    ↓
start_executing_frame()
    ↓
    [Opcode Loop]
    PUSH, ADD, MSTORE → update internal state
    SLOAD → system.io.storage_read()
    CALL → return CallRequest preemption point
    ↓
Bootloader handles CALL
    ↓
continue_after_preemption(call_result)
    ↓
    [Resume Opcode Loop]
    RETURN → return End preemption point with returndata
    ↓
Bootloader receives final result
```

**Cross-references**: See [ExecutionEnvironments.md](./ExecutionEnvironments.md) for EVM details, [DataFlow.md](./DataFlow.md) for execution pipeline.

---

## I/O & Resource Types

### IOSubsystem Trait

**Location**: `zk_ee/src/system/io.rs:27-161`

**Purpose**: The `IOSubsystem` trait defines the user-facing interface for all I/O operations in zkSync OS. It completely abstracts storage models, cost accounting, and oracle communication from execution environments. This trait is the primary way execution environments interact with persistent state and emit events.

**Key Methods**:

1. **Storage Operations**:
```rust
fn storage_read<const TRANSIENT: bool>(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
    key: &StorageKey,
) -> Result<StorageValue, SystemError>;

fn storage_write<const TRANSIENT: bool>(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
    key: &StorageKey,
    value_to_write: &StorageValue,
) -> Result<(), SystemError>;
```
   - Generic over `TRANSIENT` to handle both persistent and transient storage (EIP-1153)
   - Charges appropriate gas and native costs
   - Handles warm/cold access accounting
   - For cold access, queries oracle for initial value

2. **Account Balance Operations**:
```rust
fn get_nominal_token_balance(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
) -> Result<NominalTokenValue, SystemError>;

fn get_selfbalance(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
) -> Result<NominalTokenValue, SystemError>;
```
   - `get_nominal_token_balance`: Queries balance (cold access cost)
   - `get_selfbalance`: Optimized for SELFBALANCE opcode (assumes warm)

3. **Bytecode Operations**:
```rust
fn get_observable_bytecode_size(...) -> Result<u32, SystemError>;
fn get_observable_bytecode_hash(...) -> Result<BytecodeHashValue, SystemError>;
fn get_observable_bytecode(...) -> Result<&'static [u8], SystemError>;
```
   - Queries contract code for EXTCODESIZE, EXTCODEHASH, EXTCODECOPY

4. **Event Emission**:
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
   - Used by LOG0-LOG4 opcodes
   - Charges gas based on topic count and data length
   - Stores event in events_storage for later emission

5. **L1 Messaging**:
```rust
fn emit_l1_message(
    &mut self,
    ee_type: ExecutionEnvironmentType,
    resources: &mut Self::Resources,
    address: &Address,
    data: &[u8],
) -> Result<Bytes32, SystemError>;
```
   - Sends message from L2 to L1
   - Returns message data hash for receipt

6. **Self-Destruct**:
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
   - Marks account for destruction at end of transaction
   - Transfers balance to beneficiary
   - Returns amount transferred

7. **Frame Management**:
```rust
fn start_io_frame(&mut self) -> Result<StateSnapshot, InternalError>;
fn finish_io_frame(&mut self, rollback_handle: Option<&StateSnapshot>) -> Result<(), InternalError>;
```
   - Creates snapshot of I/O state before frame execution
   - On failure, rolls back all I/O changes to snapshot

8. **Nonce Operations**:
```rust
fn read_nonce(...) -> Result<u64, SystemError>;
fn increment_nonce(...) -> Result<u64, NonceSubsystemError>;
```
   - Reads and increments account nonces

**Usage in Data Flow**:
```
EVM Opcode Execution
    ↓
SLOAD 0x123... → interpreter calls system.io.storage_read(...)
    ↓
IOSubsystem::storage_read()
    ↓
    Check if warm → charge 100 gas
    If cold:
        ↓
        Query oracle for value → charge 2100 gas
        ↓
        Mark as warm
    ↓
    Return value to interpreter
    ↓
Interpreter pushes value to stack
```

**Cross-references**: See [SystemLayer.md](./SystemLayer.md) for I/O subsystem details, [Storage.md](./Storage.md) for storage models.

---

### IOSubsystemExt Trait

**Location**: `zk_ee/src/system/io.rs:338-508` (defined in same file as IOSubsystem)

**Purpose**: Extended I/O operations available to the system and bootloader, but not directly exposed to execution environments. These operations include oracle access, transaction lifecycle management, and finalization.

**Key Additional Methods**:
- `init_from_oracle(oracle)`: Initialize I/O subsystem with oracle
- `oracle()`: Access the underlying oracle for queries
- `begin_next_tx()`: Start processing next transaction
- `finish_tx()`: Finalize transaction (apply SELFDESTRUCT, clear transient storage)
- `storage_touch()`: Warm up storage slot (for access lists)
- `transfer_nominal_token_value()`: Transfer balance between accounts
- `touch_account()`: Warm up account (for access lists)
- `read_account_properties()`: Generic account data query
- `deploy_code()`: Store deployed bytecode
- `finish()`: Finalize block execution, return final data

---

### FullIO Struct

**Location**: `basic_system/src/system_implementation/system/io_subsystem.rs:45-62`

**Purpose**: Concrete implementation of `IOSubsystem` and `IOSubsystemExt`. This is the full-featured I/O subsystem used in both forward and proof execution. It coordinates storage models, transient storage, event/log storage, and oracle queries.

**Definition**:
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
- **A**: Allocator type (e.g., `Global` for native, `Talc` for RISC-V)
- **R**: Resources type for gas/native accounting
- **P**: Storage access policy (cost calculation)
- **SF, M**: Stack factory and frame depth for snapshots
- **O**: Oracle type for external queries
- **PROOF_ENV**: Compile-time flag for proof mode optimizations

**Key Fields**:
- **storage**: Flat storage model implementation (address+key → value mapping)
- **transient_storage**: EIP-1153 transient storage (cleared each transaction)
- **logs_storage**: L2→L1 logs (user messages, L1 tx logs)
- **events_storage**: EVM events (LOG0-LOG4)
- **allocator**: Memory allocator
- **oracle**: Interface to query external data
- **tx_number**: Current transaction number in block
- **da_commitment_scheme**: Data availability commitment scheme

**Implementation Details**:
- All `IOSubsystem` methods delegate to appropriate sub-components
- Storage operations go through `self.storage.storage_read/write()`
- Events go to `self.events_storage.push_event()`
- Logs go to `self.logs_storage.push_message()` or `push_l1_l2_tx_log()`
- Transient storage handled separately in `self.transient_storage`

**Usage in Data Flow**: `FullIO` is created at the start of block execution and holds all I/O state throughout the block. Each operation updates the appropriate internal storage, and at the end of the block, all changes are committed and returned as `BlockOutput`.

**Cross-references**: See [SystemLayer.md](./SystemLayer.md) for I/O details, [Storage.md](./Storage.md) for storage models.

---

### Resources Trait

**Location**: `zk_ee/src/system/resources.rs:128-172`

**Purpose**: The `Resources` trait defines the interface for tracking and charging computational resources. zkSync OS uses a double accounting model: **EVM gas** (for Ethereum compatibility) and **native resources** (for RISC-V cycle counting). This trait unifies both resource types into a single interface.

**Key Methods**:
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

**Resource Types**:

1. **Ergs** (EVM Gas):
   - Defined at `zk_ee/src/system/resources.rs:60-119`
   - Conversion: 1 EVM gas = 256 ergs
   - Used for EVM gas accounting (compatible with Ethereum semantics)
   - Checked on every operation, out-of-gas causes revert

2. **Native** (RISC-V Cycles):
   - Implementation-defined (typically u64)
   - Tracks actual computational cost in RISC-V environment
   - Used for:
     - Memory allocations
     - Bytecode preprocessing
     - Cryptographic operations
     - Storage operations
   - Prevents denial-of-service via excessive computation

**Double Accounting Model**:
Most operations charge BOTH ergs and native:
```rust
// Example: SLOAD cold access
let ergs = Ergs(COLD_SLOAD_COST * ERGS_PER_GAS); // 2100 gas = 537,600 ergs
let native = Native::from_computational(COLD_STORAGE_READ_NATIVE_COST); // e.g., 1000 cycles
resources.charge(&Resources::from_ergs_and_native(ergs, native))?;
```

**Why Both?**:
- **Ergs**: Ensures Ethereum compatibility (same gas costs)
- **Native**: Ensures RISC-V execution completes in reasonable time
- An operation might be cheap in EVM gas but expensive in RISC-V (e.g., memory access)
- Both limits must be satisfied, whichever exhausts first causes OOG error

**Usage in Data Flow**:
```
Transaction starts with resources:
    ergs: tx.gas_limit * 256
    native: block_native_limit

Each operation:
    ↓
    Calculate ergs cost (EVM semantics)
    Calculate native cost (RISC-V cycles)
    ↓
    resources.charge(&Resources::from_ergs_and_native(ergs, native))?
    ↓
    If either resource exhausted → OutOfGas error
    ↓
    Continue execution

At end:
    Unused ergs → refund to user
    Native consumed → recorded for proof
```

**Special Pattern: `with_infinite_ergs`**:
When the system does work on behalf of the EE (already paid for in ergs), but still needs to track native resources:
```rust
resources.with_infinite_ergs(|inf_resources| {
    // System operation that shouldn't charge more EVM gas
    system.io.deploy_code(inf_resources, address, bytecode)?;
    // This charges native but not ergs
});
```

**Cross-references**: See [SystemLayer.md](./SystemLayer.md) for resource accounting details, [ExecutionEnvironments.md](./ExecutionEnvironments.md) for gas calculation.

---

## Call Handling Types

### ExternalCallRequest&lt;'a, S: SystemTypes&gt;

**Location**: `zk_ee/src/system/execution_environment/environment_state.rs:26-38`

**Purpose**: Represents a request from an execution environment to make an external call (CALL, STATICCALL, DELEGATECALL, CREATE, CREATE2, etc.). The EE constructs this request and returns it to the bootloader via a preemption point, and the bootloader validates and executes the call.

**Definition**:
```rust
pub struct ExternalCallRequest<'a, S: SystemTypes> {
    pub available_resources: S::Resources,
    pub ergs_to_pass: Ergs,
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub callee: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub callers_caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub modifier: CallModifier,
    pub input: &'a [u8],
    pub nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    pub call_scratch_space: Option<Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], S::Allocator>>,
}
```

**Key Fields**:
- **available_resources**: Total resources available to caller frame (before call)
- **ergs_to_pass**: How many ergs caller explicitly wants to pass to callee
- **caller**: The contract making the call (msg.sender in callee frame)
- **callee**: Target address of the call
- **callers_caller**: Caller of the caller (for DELEGATECALL context)
- **modifier**: Call type (normal, static, delegate, constructor, etc.)
- **input**: Calldata to pass to callee
- **nominal_token_value**: ETH/token value to transfer with call (msg.value)
- **call_scratch_space**: Scratch space for return data (optimization)

**Call Modifiers** (from `zk_ee/src/system/call_modifiers.rs:3-14`):
```rust
pub enum CallModifier {
    NoModifier,          // Normal CALL
    Constructor,         // CREATE/CREATE2
    Delegate,            // DELEGATECALL
    Static,              // STATICCALL
    DelegateStatic,      // DELEGATECALL in static context
    ZKVMSystem,          // System call (zkSync-specific)
    ZKVMSystemStatic,    // System call in static context
    EVMCallcode,         // CALLCODE (deprecated but supported)
    EVMCallcodeStatic,   // CALLCODE in static context
}
```

**Validation Rules**:
- **Static context**: Only `Static` and `DelegateStatic` modifiers allowed, no value transfer
- **Value transfer**: Only allowed for `NoModifier`, `Constructor`, `ZKVMSystem`, `EVMCallcode`
- **DELEGATECALL**: Uses caller's storage/balance, callee's code
- **CALLCODE**: Uses caller's storage/balance, callee's code (legacy)

**Usage in Data Flow**:
```
1. EVM executing CALL opcode:
   ↓
   interpreter.execute_call() creates ExternalCallRequest
   ↓
   Set pending_os_request = Some(PendingOsRequest::Call)
   ↓
   Return ExecutionEnvironmentPreemptionPoint::CallRequest

2. Bootloader receives CallRequest:
   ↓
   Validate call parameters
   ↓
   Check stack depth < 1024
   ↓
   Query callee account properties (balance, bytecode, nonce)
   ↓
   If value transfer: check balance, perform transfer
   ↓
   Calculate resources for callee (EIP-150: 63/64 rule)
   ↓
   Create new ExecutionEnvironmentLaunchParams for callee
   ↓
   Recursively call EE.start_executing_frame(callee_params)
   ↓
   Get callee result and unused resources
   ↓
   Return CallResult to caller via continue_after_preemption()

3. Caller EE receives CallResult:
   ↓
   Update returndata
   ↓
   Reclaim unused resources
   ↓
   Continue execution
```

**Cross-references**: See [ExecutionEnvironments.md](./ExecutionEnvironments.md) for call handling, [DataFlow.md](./DataFlow.md) for call pipeline.

---

### CallResult&lt;'a, S: SystemTypes&gt;

**Location**: `zk_ee/src/system/execution_environment/call_params.rs:68-75`

**Purpose**: Represents the result of an external call after the callee frame completes. The bootloader constructs this and passes it to the caller EE via `continue_after_preemption()`.

**Definition**:
```rust
pub enum CallResult<'a, S: SystemTypes> {
    /// Call preparations failed (e.g., insufficient balance, stack too deep)
    PreparationStepFailed,
    /// Call failed after preparation (e.g., revert, out of gas)
    Failed { return_values: ReturnValues<'a, S> },
    /// Call succeeded
    Successful { return_values: ReturnValues<'a, S> },
}
```

**Variants**:
1. **PreparationStepFailed**:
   - Call never started executing (pre-flight checks failed)
   - Examples: insufficient balance for value transfer, stack depth limit, invalid nonce
   - Return data is empty
   - Used gas: only pre-call costs

2. **Failed**:
   - Call started but failed during execution
   - Examples: REVERT, out of gas, invalid opcode, stack underflow
   - May have return data (from REVERT)
   - Used gas: all gas passed to callee (none refunded)

3. **Successful**:
   - Call completed successfully
   - Has return data (from RETURN)
   - Used gas: actual gas consumed (unused gas refunded to caller)

**ReturnValues** (from `call_params.rs:40-63`):
```rust
pub struct ReturnValues<'a, S: SystemTypes> {
    pub returndata: &'a [u8],
    pub return_scratch_space: Option<Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], S::Allocator>>,
}
```
- **returndata**: Slice of data returned by callee (from RETURN or REVERT)
- **return_scratch_space**: Optional scratch space reused from call

**Usage in Data Flow**:
```
Caller EE continues after call:
    ↓
    continue_after_preemption(returned_resources, call_result)
    ↓
    match call_result:
        Successful { return_values } →
            Update interpreter.returndata = return_values.returndata
            Push 1 (success) to stack
            Reclaim returned_resources
        Failed { return_values } →
            Update interpreter.returndata = return_values.returndata
            Push 0 (failure) to stack
            Reclaim returned_resources (usually 0)
        PreparationStepFailed →
            interpreter.returndata = &[]
            Push 0 (failure) to stack
            Reclaim all passed resources
    ↓
    Continue executing caller's next opcode
```

**EVM Semantics**:
- CALL/STATICCALL/DELEGATECALL push 1 (success) or 0 (failure) to stack
- Caller must check return value (no automatic revert on failure)
- RETURNDATASIZE/RETURNDATACOPY access `returndata` regardless of success/failure
- CREATE/CREATE2 push address (success) or 0 (failure) to stack

**Cross-references**: See [ExecutionEnvironments.md](./ExecutionEnvironments.md) for call handling.

---

### ExecutionEnvironmentLaunchParams&lt;'a, S: SystemTypes&gt;

**Location**: `zk_ee/src/system/execution_environment/environment_state.rs:13-16`

**Purpose**: Contains all the parameters needed to start executing a new execution environment frame. Passed to `ExecutionEnvironment::start_executing_frame()`.

**Definition**:
```rust
pub struct ExecutionEnvironmentLaunchParams<'a, S: SystemTypes> {
    pub external_call: ExternalCallRequest<'a, S>,
    pub environment_parameters: EnvironmentParameters<'a>,
}

pub struct EnvironmentParameters<'a> {
    pub scratch_space_len: u32,
    pub callstack_depth: usize,
    pub callee_account_properties: CalleeAccountProperties<'a>,
}
```

**Components**:

1. **external_call** (`ExternalCallRequest`):
   - All call parameters (caller, callee, value, calldata, resources, modifier)
   - See `ExternalCallRequest` section above

2. **environment_parameters**:
   - **scratch_space_len**: Size of scratch space available for this frame
   - **callstack_depth**: Current call stack depth (for limiting recursion)
   - **callee_account_properties**: Information about callee account:
     - `bytecode`: Executable bytecode
     - `bytecode_hash`: Hash of bytecode
     - `observable_bytecode_hash`: Hash visible to EXTCODEHASH
     - `observable_bytecode_len`: Length visible to EXTCODESIZE
     - `code_version`: Version byte (for future EEs)
     - `is_delegated`: Is this a delegated account (EIP-7702)

**Usage in Data Flow**:
```
Bootloader preparing call:
    ↓
    Query callee account data via io.read_account_properties()
    ↓
    Create ExternalCallRequest from call parameters
    ↓
    Create EnvironmentParameters with:
        callstack_depth = current_depth + 1
        scratch_space_len = heap size
        callee_account_properties = queried data
    ↓
    Create ExecutionEnvironmentLaunchParams {
        external_call: request,
        environment_parameters: params,
    }
    ↓
    Call EE.start_executing_frame(launch_params)
```

**Cross-references**: See [ExecutionEnvironments.md](./ExecutionEnvironments.md) for execution flow.

---

## Storage Types

### WarmStorageKey

**Location**: `zk_ee/src/common_structs/warm_storage_key.rs:5-8`

**Purpose**: Represents a storage location identified by contract address and storage key. Used throughout the system to track accessed storage slots for gas accounting (warm vs. cold access).

**Definition**:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WarmStorageKey {
    pub address: B160,  // 160-bit Ethereum address
    pub key: Bytes32,   // 256-bit storage key
}
```

**Key Properties**:
- **Ord/PartialOrd**: Implements ordering (address first, then key) for BTree usage
- **Hash**: Can be used as hash map key
- **Default**: Creates zero address and zero key

**Flat Storage Key Derivation** (line 49-60):
```rust
pub fn derive_flat_storage_key(address: &B160, key: &Bytes32) -> Bytes32 {
    use crypto::blake2s::Blake2s256;
    use crypto::MiniDigest;
    let mut hasher = Blake2s256::new();
    let mut extended_address = Bytes32::ZERO;
    extended_address.as_u8_array_mut()[12..].copy_from_slice(&address.to_be_bytes());
    hasher.update(extended_address.as_u8_array_ref());
    hasher.update(key.as_u8_array_ref());
    let hash = hasher.finalize();
    Bytes32::from_array(hash)
}
```
This converts `(address, key)` → flat storage key for the merkle tree.

**Usage in Data Flow**:
```
SLOAD/SSTORE operation:
    ↓
    Create WarmStorageKey { address, key }
    ↓
    Check warmth cache:
        If warm → charge 100 gas (warm access)
        If cold →
            Query oracle for initial value
            Charge 2100 gas (cold access)
            Mark as warm in cache
    ↓
    Perform read/write operation
    ↓
    Track changes for pubdata calculation
```

**Warm vs. Cold Access** (EIP-2929):
- **Warm**: Slot already accessed in this transaction (100 gas for SLOAD, varies for SSTORE)
- **Cold**: First access in this transaction (2100 gas for SLOAD, +2000 for SSTORE)
- Warmness is per-transaction, reset at transaction start
- Access lists (EIP-2930) can pre-warm slots

**Cross-references**: See [Storage.md](./Storage.md) for storage model details, [SystemLayer.md](./SystemLayer.md) for gas accounting.

---

### WarmStorageValue

**Location**: `zk_ee/src/common_structs/warm_storage_value.rs:10-20`

**Purpose**: Tracks the lifecycle of a storage slot's value through execution, including initial value, current value, warmness, and pubdata implications. This is the cached representation of a storage slot used by the storage model.

**Definition**:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WarmStorageValue {
    pub initial_value: Bytes32,                  // Value at start of block
    pub current_value: Bytes32,                  // Current value after writes
    pub value_at_the_start_of_tx: Bytes32,       // Value at start of current tx
    pub changes_stack_depth: usize,              // Frame depth of last change
    pub last_accessed_at_tx_number: Option<u32>, // Which tx last touched this slot
    pub pubdata_diff_bytes: u8,                  // Bytes of pubdata for this slot
    pub initial_value_used: bool,                // Was initial_value ever read?
    pub is_new_storage_slot: bool,               // Is this a new slot (never had value)?
}
```

**Field Purposes**:

1. **initial_value**:
   - Value at the start of the block
   - Used to calculate storage diffs published to L1
   - Set from oracle query on first cold access

2. **current_value**:
   - Latest value after all writes
   - Returned by SLOAD operations
   - Updated by SSTORE operations

3. **value_at_the_start_of_tx**:
   - Value when current transaction started
   - Used for SSTORE gas calculation (EIP-2200)
   - Reset at start of each transaction

4. **changes_stack_depth**:
   - Call frame depth where last write occurred
   - Used for rollback on revert (restore to parent frame's value)

5. **last_accessed_at_tx_number**:
   - Transaction number of last access
   - `None` if never accessed (slot is cold)
   - Used to determine warm/cold status

6. **pubdata_diff_bytes**:
   - Number of bytes of pubdata this slot contributes
   - Calculation:
     - If `initial_value == 0 && current_value != 0`: 32 bytes (new slot)
     - If `initial_value != 0 && current_value == 0`: 0 bytes (clearing slot)
     - If both nonzero and different: 32 bytes (modification)
     - If same: 0 bytes (no change)
   - Used for pubdata gas accounting

7. **initial_value_used**:
   - True if initial_value was queried from oracle
   - Optimization: if false, no need to publish initial value

8. **is_new_storage_slot**:
   - True if this slot never had a value before
   - Affects gas calculation (first write to new slot costs more)

**SSTORE Gas Calculation** (using these fields):
```
If slot is cold:
    cost += COLD_SLOAD_COST (2100 gas)
    Mark as warm

If value_at_the_start_of_tx == new_value:
    cost += WARM_SLOAD_COST (100 gas)  // Restoring original value
    refund += SSTORE_RESET_REFUND if original was nonzero
Else if current_value != new_value:
    If value_at_the_start_of_tx != current_value:
        // Modifying already modified slot
        cost += WARM_SLOAD_COST (100 gas)
    Else:
        // First modification this tx
        If value_at_the_start_of_tx == 0:
            cost += SSTORE_SET_GAS (20000 gas)  // New slot
        Else:
            cost += SSTORE_RESET_GAS (2900 gas)  // Modify existing

        If new_value == 0:
            refund += SSTORE_CLEARS_SCHEDULE  // Clearing slot
```

**Usage in Data Flow**:
```
SLOAD address:0x123, key:0xABC
    ↓
    Create WarmStorageKey { address: 0x123, key: 0xABC }
    ↓
    Look up in warmness cache
    ↓
    If found:
        Return cached.current_value
        Charge warm gas (100)
    Else:
        Query oracle for initial value
        Charge cold gas (2100)
        Create WarmStorageValue {
            initial_value: oracle_result,
            current_value: oracle_result,
            value_at_the_start_of_tx: oracle_result,
            last_accessed_at_tx_number: Some(current_tx),
            ...
        }
        Insert into cache
        Return oracle_result

SSTORE address:0x123, key:0xABC, value:0xDEF
    ↓
    Look up in cache (cold access if not found)
    ↓
    Calculate gas cost using WarmStorageValue fields
    ↓
    Update WarmStorageValue {
        current_value: 0xDEF,
        changes_stack_depth: current_frame_depth,
        pubdata_diff_bytes: calculate_diff(initial_value, 0xDEF),
        ...
    }
    ↓
    Track change for potential rollback

On frame revert:
    ↓
    For each slot with changes_stack_depth >= reverted_frame_depth:
        Restore previous value from history
        Update changes_stack_depth to parent frame
```

**Cross-references**: See [Storage.md](./Storage.md) for storage model, [SystemLayer.md](./SystemLayer.md) for gas accounting.

---

## Metadata Types

### BlockMetadataFromOracle

**Location**: `zk_ee/src/system/metadata/zk_metadata.rs:108-127`

**Purpose**: Contains all block-level metadata required for transaction execution. This data comes from the oracle (external input) and is used by opcodes like TIMESTAMP, NUMBER, COINBASE, etc. It remains constant for all transactions in a block.

**Definition**:
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockMetadataFromOracle {
    pub chain_id: u64,                     // Chain ID (e.g., 1 for Ethereum mainnet)
    pub block_number: u64,                 // Current block number
    pub block_hashes: BlockHashes,         // Array of previous 256 block hashes
    pub timestamp: u64,                    // Block timestamp (Unix time)
    pub eip1559_basefee: U256,             // EIP-1559 base fee per gas
    pub pubdata_price: U256,               // Cost per byte of pubdata (zkSync-specific)
    pub native_price: U256,                // Native token price (zkSync-specific)
    pub coinbase: B160,                    // Block beneficiary address (miner/validator)
    pub gas_limit: u64,                    // Block gas limit
    pub pubdata_limit: u64,                // Block pubdata limit (zkSync-specific)
    pub mix_hash: U256,                    // Source of randomness (prevRandao in PoS)
}
```

**Field Details**:

1. **chain_id**: Used by CHAINID opcode, transaction signing
   - Mainnet: 1
   - Goerli: 5
   - Sepolia: 11155111
   - zkSync Era: 324
   - zkSync OS: TBD

2. **block_number**: Used by NUMBER opcode
   - Monotonically increasing
   - Used for BLOCKHASH lookups

3. **block_hashes**: Array of 256 previous block hashes
   - Used by BLOCKHASH(n) opcode
   - Only blocks in range `[current - 256, current)` are available
   - Stored in array where index `256 - (current - n)` holds hash for block `n`

4. **timestamp**: Used by TIMESTAMP opcode
   - Unix timestamp in seconds
   - Must be ≥ parent block's timestamp
   - Used by contracts for time-based logic

5. **eip1559_basefee**: Used by BASEFEE opcode
   - Dynamic base fee per gas (EIP-1559)
   - Burned (not given to miner/validator)
   - Transaction must have `max_fee_per_gas ≥ basefee`

6. **pubdata_price**: zkSync-specific
   - Cost in native token per byte of pubdata
   - Pubdata includes: storage diffs, events, L2→L1 messages, bytecode
   - Used to charge for L1 calldata costs

7. **native_price**: zkSync-specific
   - Price of native token (e.g., ETH) in some unit
   - Used for gas pricing calculations

8. **coinbase**: Used by COINBASE opcode
   - Address that receives transaction fees
   - In PoW: miner address
   - In PoS: validator address
   - In zkSync: operator address

9. **gas_limit**: Used by GASLIMIT opcode
   - Maximum gas that can be used in this block
   - Sum of all transaction gas limits must not exceed this

10. **pubdata_limit**: zkSync-specific
    - Maximum bytes of pubdata for this block
    - Prevents excessive L1 costs

11. **mix_hash**: Used by DIFFICULTY opcode (renamed to PREVRANDAO in PoS)
    - Source of randomness for contracts
    - In PoW: mining difficulty
    - In PoS: beacon chain randomness
    - In zkSync: passed through from L1

**EVM Opcodes Using Metadata**:
- `CHAINID` (0x46) → chain_id
- `NUMBER` (0x43) → block_number
- `BLOCKHASH` (0x40) → block_hashes[256 - (block_number - arg)]
- `TIMESTAMP` (0x42) → timestamp
- `BASEFEE` (0x48) → eip1559_basefee
- `COINBASE` (0x41) → coinbase
- `GASLIMIT` (0x45) → gas_limit
- `DIFFICULTY` / `PREVRANDAO` (0x44) → mix_hash

**Usage in Data Flow**:
```
Block execution starts:
    ↓
    Oracle provides BlockMetadataFromOracle
    ↓
    System initialized with metadata
    ↓
    For each transaction:
        EVM execution:
            TIMESTAMP opcode → system.get_timestamp() → metadata.timestamp
            NUMBER opcode → system.get_block_number() → metadata.block_number
            BLOCKHASH 123 → system.get_blockhash(123) → metadata.block_hashes lookup
            ...
    ↓
    Metadata remains constant throughout block
```

**Cross-references**: See [SystemLayer.md](./SystemLayer.md) for metadata access, [DataFlow.md](./DataFlow.md) for oracle queries.

---

## Events & Logs Types

### LogsStorage

**Location**: `zk_ee/src/common_structs/logs_storage.rs:162-166`

**Purpose**: Stores L2→L1 logs (messages and L1 transaction status logs) emitted during block execution. These logs are merkleized and included in the block commitment sent to L1.

**Definition**:
```rust
pub struct LogsStorage<SF: StackFactory<M>, const M: usize, A: Allocator + Clone = Global> {
    list: HistoryList<LogContent<A>, u32, SF, M, A>,
    pubdata_used_by_committed_logs: u32,
    _marker: core::marker::PhantomData<A>,
}
```

**Generic Parameters**:
- **SF**: Stack factory for creating snapshots
- **M**: Maximum frame depth for snapshots
- **A**: Allocator for dynamic allocations

**Key Fields**:
- **list**: History list of logs with rollback support
- **pubdata_used_by_committed_logs**: Tracks pubdata usage
- **_marker**: Zero-sized PhantomData for type safety

**Log Types** (stored in `LogContent`):

1. **User Messages** (L1Messenger system contract):
```rust
UserMsgData {
    address: B160,        // L1Messenger address (0x0000000000000000000000000000000000008008)
    data: Vec<u8>,        // Message content
    data_hash: Bytes32,   // Keccak256 hash of data
}
```
   - Sent via L1Messenger.sendToL1(data)
   - Allows L2 contracts to send arbitrary messages to L1
   - Published to L1 as calldata
   - Can be consumed by L1 contracts

2. **L1→L2 Transaction Logs** (Bootloader):
```rust
L1TxLog {
    tx_hash: Bytes32,     // Transaction hash
    success: bool,        // Did tx succeed or revert?
    is_priority: bool,    // Is this a priority (L1→L2) transaction?
}
```
   - Emitted by bootloader after each L1→L2 transaction execution
   - Proves execution result on L1
   - Used for L1→L2 message execution

**L2ToL1Log Format** (on L1):
```rust
struct L2ToL1Log {
    l2_shard_id: u8,          // Always 0 (deprecated)
    is_service: bool,         // Always true (deprecated)
    tx_number_in_block: u16,  // Transaction index in block
    sender: B160,             // L1Messenger (user msg) or Bootloader (L1 tx log)
    key: Bytes32,             // User msg: sender address; L1 tx log: tx hash
    value: Bytes32,           // User msg: message hash; L1 tx log: success flag
}
```

**Key Methods**:
```rust
impl LogsStorage {
    // Add user message
    pub fn push_message(
        &mut self,
        tx_number: u32,
        address: &B160,
        data: Vec<u8>,
        data_hash: Bytes32,
    ) -> Result<(), SystemError>;

    // Add L1 tx status log
    pub fn push_l1_l2_tx_log(
        &mut self,
        tx_number: u32,
        tx_hash: Bytes32,
        success: bool,
        is_priority: bool,
    ) -> Result<(), SystemError>;

    // Get number of logs
    pub fn len(&self) -> u64;

    // Start frame (for rollback)
    pub fn start_frame(&mut self) -> usize;

    // Finish frame (commit or rollback)
    pub fn finish_frame(&mut self, rollback_handle: Option<usize>);

    // Calculate merkle tree root
    pub fn tree_root(&self) -> Bytes32;
}
```

**Usage in Data Flow**:
```
User Message:
    L2 Contract calls L1Messenger.sendToL1(message_data)
    ↓
    L1Messenger system contract
    ↓
    system.io.emit_l1_message(address, message_data)
    ↓
    logs_storage.push_message(tx_number, L1Messenger_addr, data, keccak256(data))
    ↓
    Stored in logs list

L1 Transaction Log:
    Bootloader executes L1→L2 transaction
    ↓
    Execution completes (success or revert)
    ↓
    system.io.emit_l1_l2_tx_log(tx_hash, success, is_priority)
    ↓
    logs_storage.push_l1_l2_tx_log(tx_number, tx_hash, success, is_priority)
    ↓
    Stored in logs list

End of Block:
    ↓
    Compute logs merkle tree root (14-level tree, max 16384 logs)
    ↓
    Publish logs root and log data to L1
    ↓
    L1 can verify any individual log via merkle proof
```

**Pubdata Calculation**:
- Each log: 88 bytes (L2_TO_L1_LOG_SERIALIZE_SIZE)
- Each user message: 4 bytes (length) + message data bytes
- Total pubdata tracked in `pubdata_used_by_committed_logs`

**Rollback Support**:
- Snapshot created before each call frame
- On revert: rollback to snapshot (removes logs emitted in failed frame)
- On success: commit snapshot (keep logs)

**Cross-references**: See [SystemLayer.md](./SystemLayer.md) for L1 messaging, [DataFlow.md](./DataFlow.md) for log emission flow.

---

### EventsStorage

**Location**: `zk_ee/src/common_structs/events_storage.rs:52-60`

**Purpose**: Stores EVM events (from LOG0-LOG4 opcodes) emitted during block execution. These events are included in transaction receipts and can be queried via RPC methods like `eth_getLogs`.

**Definition**:
```rust
pub struct EventsStorage<
    const N: usize,
    SF: StackFactory<M>,
    const M: usize,
    A: Allocator + Clone = Global,
> {
    list: HistoryList<EventContent<N, A>, (), SF, M, A>,
    _marker: core::marker::PhantomData<A>,
}
```

**Generic Parameters**:
- **N**: Maximum number of topics (typically 4 for LOG0-LOG4)
- **SF**: Stack factory for creating snapshots
- **M**: Maximum frame depth for snapshots
- **A**: Allocator for dynamic allocations

**Event Content** (from `events_storage.rs:18-24`):
```rust
pub struct EventContent<const N: usize, A: Allocator = Global> {
    pub tx_number: u32,              // Transaction index in block
    pub address: B160,               // Contract that emitted the event
    pub topics: ArrayVec<Bytes32, N>, // Event topics (indexed parameters)
    pub data: Vec<u8>,               // Event data (non-indexed parameters)
}
```

**EVM LOG Opcodes**:
- **LOG0** (0xa0): No topics
- **LOG1** (0xa1): 1 topic
- **LOG2** (0xa2): 2 topics
- **LOG3** (0xa3): 3 topics
- **LOG4** (0xa4): 4 topics

**Solidity Event Example**:
```solidity
event Transfer(address indexed from, address indexed to, uint256 value);

// Emits:
// topics[0] = keccak256("Transfer(address,address,uint256)")
// topics[1] = from (indexed)
// topics[2] = to (indexed)
// data = abi.encode(value)
```

**Key Methods**:
```rust
impl EventsStorage {
    // Add event
    pub fn push_event(
        &mut self,
        tx_number: u32,
        address: &B160,
        topics: &ArrayVec<Bytes32, N>,
        data: Vec<u8>,
    ) -> Result<(), SystemError>;

    // Start frame (for rollback)
    pub fn start_frame(&mut self) -> usize;

    // Finish frame (commit or rollback)
    pub fn finish_frame(&mut self, rollback_handle: Option<usize>);

    // Iterate over events
    pub fn iter_net_diff(&self) -> impl Iterator<Item = &EventContent<N, A>>;
}
```

**Usage in Data Flow**:
```
EVM LOG Opcode:
    Solidity: emit Transfer(from, to, amount);
    ↓
    Compiler generates: LOG3 instruction
    ↓
    EVM Interpreter:
        Stack: [data_offset, data_size, topic1, topic2, topic3]
        ↓
        Read data from memory[data_offset:data_offset+data_size]
        ↓
        Calculate gas cost:
            base_cost = LOG (375 gas)
            topic_cost = LOGTOPIC * num_topics (375 gas per topic)
            data_cost = LOGDATA * data_size (8 gas per byte)
            total = base_cost + topic_cost + data_cost
        ↓
        system.io.emit_event(address, topics, data)
        ↓
        events_storage.push_event(tx_number, address, topics, data)
        ↓
        Event stored in list

End of Transaction:
    ↓
    Events are included in transaction receipt
    ↓
    Receipt contains:
        - status (success/revert)
        - cumulative_gas_used
        - logs_bloom (bloom filter of all event topics/addresses)
        - logs (all events from this tx)

RPC Query (eth_getLogs):
    ↓
    Filter by:
        - address (contract that emitted)
        - topics (indexed parameters)
        - block range
    ↓
    Return matching events
```

**Gas Calculation**:
```
LOG0: 375 + 8 * data_size
LOG1: 375 + 375 + 8 * data_size
LOG2: 375 + 750 + 8 * data_size
LOG3: 375 + 1125 + 8 * data_size
LOG4: 375 + 1500 + 8 * data_size
```

**Rollback Support**:
- Snapshot created before each call frame
- On revert: rollback to snapshot (removes events emitted in failed frame)
- On success: commit snapshot (keep events)

**Bloom Filter** (in receipts):
- 256-byte (2048-bit) bloom filter
- Includes:
  - Event emitter address
  - All event topics
- Enables efficient filtering without scanning all events

**Cross-references**: See [SystemLayer.md](./SystemLayer.md) for event emission, [DataFlow.md](./DataFlow.md) for execution pipeline.

---

## Error Types

### EvmError

**Location**: `zk_ee/src/system/execution_environment/evm/errors.rs:8-39`

**Purpose**: Defines all EVM-specific error conditions that can occur during bytecode execution. These errors are distinct from system errors (I/O failures, internal bugs) and represent valid but unsuccessful EVM execution outcomes.

**Definition**:
```rust
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmError {
    /// Revert caused by REVERT opcode
    Revert,
    /// Out of gas (ergs exhausted)
    OutOfGas,
    /// Invalid JUMP opcode destination (not JUMPDEST)
    InvalidJump,
    /// Attempt to access returndata with invalid index (out of bounds)
    ReturnDataOutOfBounds,
    /// Unknown opcode encountered
    InvalidOpcode(u8),
    /// Stack underflow (not enough items on stack for opcode)
    StackUnderflow,
    /// Stack overflow (>1024 items on stack)
    StackOverflow,
    /// CALL/CREATE attempted in STATICCALL context with value transfer
    CallNotAllowedInsideStatic,
    /// State-changing opcode (SSTORE, LOG, CREATE, etc.) in STATICCALL context
    StateChangeDuringStaticCall,
    /// Memory offset > u32::MAX - 31, treated as out of gas
    MemoryLimitOOG,
    /// Invalid operand (e.g. failed cast), treated as out of gas
    InvalidOperandOOG,
    /// Insufficient gas to pay for code deployment
    CodeStoreOutOfGas,
    /// [Call-specific] Callstack depth exceeds 1024
    CallTooDeep,
    /// [Call-specific] Insufficient balance for value transfer
    InsufficientBalance,
    /// [Call-specific] Attempt to deploy contract at already occupied address
    CreateCollision,
    /// [Call-specific] Caller nonce overflowed during deployment
    NonceOverflow,
    /// Deployed contract size exceeds 24KB limit
    CreateContractSizeLimit,
    /// Init code size exceeds 48KB limit (EIP-3860)
    CreateInitcodeSizeLimit,
    /// Deployed contract starts with 0xEF byte (reserved for EOF)
    CreateContractStartingWithEF,
}
```

**Error Categories**:

1. **Explicit Revert**:
   - `Revert`: REVERT opcode executed
   - Has return data (revert reason)
   - Gas refund: partial (unused gas returned)

2. **Resource Exhaustion**:
   - `OutOfGas`: Ergs (gas) exhausted during execution
   - `CodeStoreOutOfGas`: Insufficient gas for CREATE/CREATE2 deployment
   - `MemoryLimitOOG`: Memory access beyond u32::MAX (OOG result)
   - `InvalidOperandOOG`: Invalid cast/operation (OOG result)
   - Gas refund: none (all gas consumed)

3. **Invalid Bytecode**:
   - `InvalidOpcode(opcode)`: Unknown opcode byte
   - `InvalidJump`: JUMP to non-JUMPDEST location
   - All gas consumed, no return data

4. **Stack Errors**:
   - `StackUnderflow`: Opcode requires more stack items than available
   - `StackOverflow`: Stack exceeds 1024 items
   - All gas consumed, no return data

5. **Static Context Violations**:
   - `StateChangeDuringStaticCall`: SSTORE, LOG, CREATE, etc. in STATICCALL
   - `CallNotAllowedInsideStatic`: CALL with value in STATICCALL
   - All gas consumed, no return data

6. **Return Data Errors**:
   - `ReturnDataOutOfBounds`: RETURNDATACOPY beyond available data
   - All gas consumed, no return data

7. **Call-Specific Errors** (before frame starts):
   - `CallTooDeep`: Call depth ≥ 1024
   - `InsufficientBalance`: Caller lacks balance for value transfer
   - `CreateCollision`: Deploy to non-empty account
   - `NonceOverflow`: Nonce increment overflows u64
   - Gas refund: all gas returned (call never started)

8. **Deployment Errors**:
   - `CreateContractSizeLimit`: Deployed bytecode > 24KB (24576 bytes)
   - `CreateInitcodeSizeLimit`: Init code > 48KB (EIP-3860)
   - `CreateContractStartingWithEF`: Bytecode starts with 0xEF (EOF reserved)
   - All gas consumed, no return data

**Error Handling in EVM**:
```rust
match execute_opcode(opcode) {
    Ok(()) => continue,  // Next opcode
    Err(ExitCode::EvmError(err)) => {
        match err {
            EvmError::Revert => {
                // Return to caller with returndata, refund unused gas
                return CallResult::Failed { return_values }
            }
            EvmError::OutOfGas => {
                // Consume all gas, no returndata
                return CallResult::Failed { return_values: empty }
            }
            EvmError::CallTooDeep => {
                // Call never started, refund all gas
                return CallResult::PreparationStepFailed
            }
            // ... other errors
        }
    }
    Err(ExitCode::FatalError(e)) => {
        // System error (I/O failure, internal bug)
        panic!("Fatal error: {:?}", e)
    }
}
```

**Usage in Data Flow**:
```
EVM Opcode Execution:
    ↓
    ADD instruction:
        Pop 2 items from stack
        If stack has < 2 items → Err(EvmError::StackUnderflow)
        ↓
        Add values
        Push result to stack
        If stack has 1024 items → Err(EvmError::StackOverflow)
    ↓
    JUMP 0x1234:
        Check jumpdest_bitmap[0x1234]
        If not JUMPDEST → Err(EvmError::InvalidJump)
        ↓
        Set instruction_pointer = 0x1234
    ↓
    SSTORE in STATICCALL:
        If is_static → Err(EvmError::StateChangeDuringStaticCall)
        ↓
        Perform SSTORE
    ↓
    Execution continues until:
        - RETURN/STOP → Success
        - REVERT → Failed with returndata
        - Any other EvmError → Failed, all gas consumed
        - Resource exhausted → OutOfGas
```

**Revert Reasons** (with Revert error):
Solidity can encode custom revert reasons:
```solidity
require(balance >= amount, "Insufficient balance");
// Generates: REVERT with returndata = Error(string) ABI-encoded
```

Client can decode returndata to show human-readable error.

**Cross-references**: See [ExecutionEnvironments.md](./ExecutionEnvironments.md) for error handling, [DataFlow.md](./DataFlow.md) for execution flow.

---

## Data Flow Relationships

### High-Level Type Interactions

The following ASCII diagram illustrates how the key types interact during transaction execution:

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Block Execution                              │
│                                                                      │
│  Oracle provides: BlockMetadataFromOracle                           │
│                   ├─ chain_id, block_number, timestamp, etc.        │
│                   └─ Used by CHAINID, NUMBER, TIMESTAMP opcodes     │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│              SystemTypes Trait (Dependency Injection)               │
│                                                                      │
│  Parametrizes all major types:                                      │
│    - System<S: SystemTypes>                                         │
│    - Interpreter<'a, S: SystemTypes>                                │
│    - ExternalCallRequest<'a, S: SystemTypes>                        │
│    - CallResult<'a, S: SystemTypes>                                 │
│                                                                      │
│  Defines:                                                            │
│    - S::IO (e.g., FullIO)                                           │
│    - S::Resources (double accounting: ergs + native)                │
│    - S::Allocator (memory allocator)                                │
│    - S::Metadata (BlockMetadataFromOracle)                          │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│                System<S> (Central Hub)                               │
│                                                                      │
│  Fields:                                                             │
│    - io: S::IO (FullIO instance)                                    │
│    - metadata: S::Metadata (BlockMetadataFromOracle)                │
│    - allocator: S::Allocator                                        │
│                                                                      │
│  Provides:                                                           │
│    - get_timestamp() → metadata.timestamp                           │
│    - get_block_number() → metadata.block_number                     │
│    - Storage: system.io.storage_read/write()                        │
│    - Events: system.io.emit_event()                                 │
│    - Balance: system.io.get_nominal_token_balance()                 │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│                  Transaction Execution Loop                          │
│                                                                      │
│  1. Bootloader creates ExecutionEnvironmentLaunchParams:            │
│     - external_call: ExternalCallRequest                            │
│       ├─ caller, callee, value, calldata                            │
│       ├─ available_resources: S::Resources                          │
│       └─ modifier: CallModifier (Normal/Static/Delegate/etc.)       │
│     - environment_parameters:                                        │
│       ├─ callstack_depth                                            │
│       └─ callee_account_properties (bytecode, hash, balance)        │
│                                                                      │
│  2. Bootloader calls EE.start_executing_frame(launch_params)        │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│            Interpreter<'a, S> (EVM Implementation)                   │
│                                                                      │
│  State:                                                              │
│    - instruction_pointer: usize (program counter)                   │
│    - gas: Gas<S> (wraps S::Resources)                               │
│    - stack: EvmStack (max 1024 items)                               │
│    - heap: SliceVec (sparse memory)                                 │
│    - bytecode: &[u8]                                                 │
│    - calldata: &[u8]                                                 │
│    - returndata: &[u8]                                               │
│    - caller, address, call_value                                    │
│    - is_static, is_constructor                                      │
│    - pending_os_request: Option<PendingOsRequest>                   │
│                                                                      │
│  Opcode Execution Loop:                                              │
│    ┌──────────────────────────────────────────────────────┐        │
│    │ Fetch opcode at bytecode[instruction_pointer]        │        │
│    └──────────────────────────────────────────────────────┘        │
│                          ↓                                           │
│    ┌──────────────────────────────────────────────────────┐        │
│    │ Execute opcode:                                       │        │
│    │                                                       │        │
│    │ ADD:    Pop 2, push 1, gas: 3                        │        │
│    │ SLOAD:  system.io.storage_read() → WarmStorageKey   │        │
│    │         ├─ Check warm/cold                            │        │
│    │         ├─ Query oracle if cold                       │        │
│    │         └─ Return WarmStorageValue.current_value      │        │
│    │ SSTORE: system.io.storage_write() → Update cache     │        │
│    │ LOG2:   system.io.emit_event() → EventsStorage       │        │
│    │ CALL:   Create ExternalCallRequest                   │        │
│    │         Return CallRequest preemption point           │        │
│    └──────────────────────────────────────────────────────┘        │
│                          ↓                                           │
│    ┌──────────────────────────────────────────────────────┐        │
│    │ Update instruction_pointer                            │        │
│    └──────────────────────────────────────────────────────┘        │
│                          ↓                                           │
│    ┌──────────────────────────────────────────────────────┐        │
│    │ Check resources (gas.resources)                      │        │
│    │ If exhausted → Err(EvmError::OutOfGas)               │        │
│    └──────────────────────────────────────────────────────┘        │
│                          ↓                                           │
│    ┌──────────────────────────────────────────────────────┐        │
│    │ Loop until:                                           │        │
│    │   - RETURN/STOP → Success                            │        │
│    │   - REVERT → Failed with returndata                  │        │
│    │   - CALL → Preemption (external call)                │        │
│    │   - EvmError → Failed                                │        │
│    └──────────────────────────────────────────────────────┘        │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
          ┌─────────────────────────────────────────┐
          │ Preemption: ExecutionEnvironmentPreemptionPoint│
          └─────────────────────────────────────────┘
                      ↓                     ↓
        ┌─────────────────────┐   ┌──────────────────────┐
        │ CallRequest         │   │ End(CompletedExecution)│
        │ - request: External-│   │ - resources_returned  │
        │   CallRequest       │   │ - result: CallResult  │
        │ - heap: SliceVec    │   └──────────────────────┘
        └─────────────────────┘
                      ↓
┌─────────────────────────────────────────────────────────────────────┐
│              Bootloader Handles CallRequest                          │
│                                                                      │
│  1. Validate call (depth, static context, etc.)                     │
│  2. Query callee properties via system.io                           │
│  3. Transfer value if needed                                        │
│  4. Calculate resources for callee (EIP-150: 63/64 rule)            │
│  5. Create new ExecutionEnvironmentLaunchParams for callee          │
│  6. Recursively call EE.start_executing_frame(callee_params)        │
│  7. Get callee CompletedExecution                                   │
│  8. Construct CallResult:                                            │
│     - PreparationStepFailed (if validation failed)                  │
│     - Failed { return_values } (if callee reverted)                 │
│     - Successful { return_values } (if callee succeeded)            │
│  9. Call caller's EE.continue_after_preemption(result)              │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│            Interpreter Continues After Preemption                    │
│                                                                      │
│  1. Receive CallResult and returned_resources                       │
│  2. Update returndata based on CallResult                           │
│  3. Push success (1) or failure (0) to stack                        │
│  4. Reclaim returned_resources                                      │
│  5. Continue opcode execution                                       │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│              FullIO (I/O Subsystem Implementation)                   │
│                                                                      │
│  Components:                                                         │
│    ┌───────────────────────────────────────────────────┐           │
│    │ storage: FlatStorageModel                         │           │
│    │   - Cache: Map<WarmStorageKey, WarmStorageValue> │           │
│    │   - Oracle queries for cold access                │           │
│    │   - Tracks initial/current values                 │           │
│    │   - Calculates pubdata diffs                      │           │
│    └───────────────────────────────────────────────────┘           │
│    ┌───────────────────────────────────────────────────┐           │
│    │ transient_storage: GenericTransientStorage        │           │
│    │   - Map<WarmStorageKey, Bytes32>                  │           │
│    │   - Cleared at end of transaction                 │           │
│    │   - EIP-1153 TLOAD/TSTORE                         │           │
│    └───────────────────────────────────────────────────┘           │
│    ┌───────────────────────────────────────────────────┐           │
│    │ events_storage: EventsStorage                     │           │
│    │   - List<EventContent>                            │           │
│    │   - tx_number, address, topics, data              │           │
│    │   - Rollback support for reverts                  │           │
│    └───────────────────────────────────────────────────┘           │
│    ┌───────────────────────────────────────────────────┐           │
│    │ logs_storage: LogsStorage                         │           │
│    │   - List<LogContent>                              │           │
│    │   - User messages (L2→L1)                         │           │
│    │   - L1 tx status logs                             │           │
│    │   - Merkle tree root calculation                  │           │
│    └───────────────────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│                     Error Handling                                   │
│                                                                      │
│  EvmError variants:                                                  │
│    - Revert: REVERT opcode, partial gas refund                      │
│    - OutOfGas: Resources exhausted, no refund                       │
│    - InvalidJump: JUMP to non-JUMPDEST, no refund                   │
│    - StackUnderflow/Overflow: Stack error, no refund                │
│    - StateChangeDuringStaticCall: STATICCALL violation, no refund   │
│    - CallTooDeep: Depth ≥ 1024, full refund (call not started)     │
│    - InsufficientBalance: Balance check failed, full refund         │
│    - ... and others                                                  │
│                                                                      │
│  On error:                                                           │
│    - Rollback storage/events/logs to frame snapshot                 │
│    - Return CallResult::Failed or PreparationStepFailed             │
│    - Propagate to caller                                            │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│                   End of Transaction                                 │
│                                                                      │
│  1. Finalize transaction: system.io.finish_tx()                     │
│     - Apply SELFDESTRUCT (mark_for_deconstruction)                  │
│     - Clear transient storage                                       │
│     - Commit storage changes                                        │
│                                                                      │
│  2. Collect results:                                                 │
│     - events_storage → Transaction receipt logs                     │
│     - logs_storage → L2→L1 messages/logs                            │
│     - storage changes → State diffs for pubdata                     │
│     - gas used, refund counter                                      │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│                     End of Block                                     │
│                                                                      │
│  1. Finalize block: system.finish()                                 │
│     - Aggregate all transaction results                             │
│     - Calculate logs merkle tree root                               │
│     - Calculate L1 tx commitment                                    │
│     - Compile pubdata (storage diffs, events, logs, bytecode)       │
│                                                                      │
│  2. Return BlockOutput:                                              │
│     - Transaction results (status, gas, logs)                       │
│     - Storage writes                                                 │
│     - Published preimages                                            │
│     - L2→L1 messages                                                 │
│     - Pubdata for L1 submission                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

### Storage Operation Flow

Detailed flow of a storage read (SLOAD) operation:

```
SLOAD 0xABCD (storage key)
    ↓
Interpreter: system.io.storage_read<false>(EVM, resources, address, 0xABCD)
    ↓
IOSubsystem::storage_read()
    ↓
Create WarmStorageKey { address, key: 0xABCD }
    ↓
Look up in storage cache
    ↓
    ┌────────────────┬─────────────────┐
    │ Cache Hit (warm)│  Cache Miss (cold)│
    └────────────────┴─────────────────┘
             ↓                    ↓
    Charge 100 gas       Query oracle for value
    (warm access)               ↓
             ↓            Charge 2100 gas
    Get cached           (cold access)
    WarmStorageValue            ↓
             ↓            Charge native cost
    Return current_value        ↓
             ↓            Create WarmStorageValue {
             ↓                initial_value: oracle_value,
             ↓                current_value: oracle_value,
             ↓                value_at_the_start_of_tx: oracle_value,
             ↓                last_accessed_at_tx_number: Some(current_tx),
             ↓                is_new_storage_slot: oracle_value == 0,
             ↓                ...
             ↓            }
             ↓                   ↓
             ↓            Insert into cache
             ↓                   ↓
             ↓            Return oracle_value
             └───────────────────┘
                      ↓
             Return value to interpreter
                      ↓
             Push value to stack
```

---

### Call Operation Flow

Detailed flow of an external call (CALL opcode):

```
CALL (gas, addr, value, argsOffset, argsSize, retOffset, retSize)
    ↓
Interpreter creates ExternalCallRequest {
    available_resources: current resources,
    ergs_to_pass: gas * 256,
    caller: this.address,
    callee: addr,
    modifier: NoModifier (or Static if in static context),
    input: heap[argsOffset..argsOffset+argsSize],
    nominal_token_value: value,
    ...
}
    ↓
Set pending_os_request = Some(PendingOsRequest::Call)
    ↓
Return ExecutionEnvironmentPreemptionPoint::CallRequest { request, heap }
    ↓
                    [Bootloader Receives Preemption]
    ↓
Validate call:
    - Check callstack_depth < 1024 → Err(CallTooDeep)
    - Check static context allows this call → Err(CallNotAllowedInsideStatic)
    - Check value transfer allowed → Err(StateChangeDuringStaticCall)
    ↓
Query callee properties: system.io.read_account_properties(addr)
    - bytecode, bytecode_hash, balance, nonce, code_version, ...
    ↓
If value > 0:
    Check caller.balance >= value → Err(InsufficientBalance)
    system.io.transfer_nominal_token_value(caller, callee, value)
    ↓
Calculate resources for callee (EIP-150):
    callee_ergs = min(ergs_to_pass, 63/64 * available_resources.ergs())
    callee_resources = Resources::from_ergs_and_native(callee_ergs, available_native)
    ↓
Create ExecutionEnvironmentLaunchParams {
    external_call: ExternalCallRequest {
        available_resources: callee_resources,
        caller: original caller,
        callee: addr,
        input: calldata,
        ...
    },
    environment_parameters: EnvironmentParameters {
        callstack_depth: current_depth + 1,
        callee_account_properties: queried properties,
        ...
    },
}
    ↓
Start callee frame: EE.start_executing_frame(launch_params)
    ↓
                [Callee Executes]
    - New Interpreter instance
    - Executes bytecode
    - May make nested calls
    - Eventually returns ExecutionEnvironmentPreemptionPoint::End
    ↓
Receive CompletedExecution {
    resources_returned: unused resources,
    result: CallResult::Successful/Failed/PreparationStepFailed,
}
    ↓
Construct final CallResult for caller
    ↓
Continue caller: EE.continue_after_preemption(resources_returned, call_result)
    ↓
                [Caller Resumes]
    ↓
Update interpreter state:
    - returndata = call_result.return_values().returndata
    - Push success (1) or failure (0) to stack
    - Reclaim resources_returned
    - If retSize > 0: Copy returndata to heap[retOffset..retOffset+retSize]
    ↓
Continue executing caller's next opcode
```

---

## Cross-References

**For deeper understanding of these types in context, see:**

- **[DataFlow.md](./DataFlow.md)**: Comprehensive data flow through the entire system, showing how these types interact during execution
- **[ExecutionEnvironments.md](./ExecutionEnvironments.md)**: Detailed execution environment abstraction, EVM interpreter implementation, and opcode execution
- **[SystemLayer.md](./SystemLayer.md)**: System primitives, I/O subsystem details, resource accounting, and metadata management
- **[Storage.md](./Storage.md)**: Storage models (flat vs. Ethereum), storage access patterns, and state management
- **[API.md](./API.md)**: Public API entry points that use these types

---

## Summary

This document covered the 15+ most critical types in zkSync OS:

**System Architecture** (2 types):
- `SystemTypes` trait: Dependency injection for system configuration
- `System<S>`: Central hub for execution environments

**Execution Environment** (2 types):
- `ExecutionEnvironment` trait: EE abstraction with preemption model
- `Interpreter<'a, S>`: Concrete EVM implementation

**I/O & Resources** (3 types):
- `IOSubsystem` trait: User-facing I/O interface
- `FullIO`: Concrete I/O implementation
- `Resources` trait: Double accounting (ergs + native)

**Call Handling** (4 types):
- `ExternalCallRequest`: Call parameters from EE to bootloader
- `CallResult`: Call result from bootloader to EE
- `ExecutionEnvironmentLaunchParams`: Frame initialization parameters
- `CallModifier`: Call type (normal/static/delegate/constructor)

**Storage** (2 types):
- `WarmStorageKey`: Storage location identifier
- `WarmStorageValue`: Cached storage slot with lifecycle tracking

**Metadata** (1 type):
- `BlockMetadataFromOracle`: Block-level metadata for opcodes

**Events & Logs** (2 types):
- `LogsStorage`: L2→L1 logs and messages
- `EventsStorage`: EVM events (LOG0-LOG4)

**Errors** (1 type):
- `EvmError`: EVM-specific error conditions

These types form the core abstractions that enable zkSync OS to support multiple execution environments, dual execution modes (forward/proof), and efficient ZK proof generation, all while maintaining Ethereum compatibility.
