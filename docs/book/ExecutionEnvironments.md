# Execution Environments

## Table of Contents

1. [Introduction](#introduction)
2. [ExecutionEnvironment Trait](#executionenvironment-trait)
3. [EVM Interpreter](#evm-interpreter)
4. [Future Execution Environments](#future-execution-environments)
5. [Execution Flow Diagram](#execution-flow-diagram)

---

## Introduction

### What is an Execution Environment?

An **Execution Environment** (EE) in zkSync OS is an abstraction that encapsulates a specific bytecode execution model. It defines how bytecode is interpreted, how opcodes are executed, and how the execution environment interacts with the underlying system layer.

The EE abstraction serves as the bridge between:
- **High-level transaction execution logic** (handled by the bootloader)
- **Low-level system operations** (storage, events, resource accounting)

Each EE is responsible for:
1. **Bytecode interpretation**: Parsing and executing instructions
2. **State management**: Maintaining execution state (stack, memory, program counter)
3. **Resource consumption**: Tracking gas and native resource usage
4. **External call coordination**: Requesting calls to other contracts via preemption
5. **Result reporting**: Returning execution outcomes to the bootloader

**Code Location**: The ExecutionEnvironment trait is defined at `zk_ee/src/system/execution_environment/mod.rs:40-101`

### Why Execution Environment Abstraction?

The EE abstraction enables zkSync OS to support multiple execution models within the same system:

1. **EVM (Ethereum Virtual Machine)**: Currently implemented
   - Full Ethereum compatibility
   - Executes EVM bytecode (opcodes 0x00-0xFF)
   - Supports all EVM features (CALL, CREATE, logs, etc.)

2. **EraVM** (Future): Planned zkSync-specific VM
   - Optimized for ZK proving
   - Custom instruction set
   - Native elliptic curve operations

3. **Wasm** (Future): WebAssembly execution
   - Portable bytecode format
   - High performance
   - Widely supported tooling

By abstracting the execution model, zkSync OS can:
- **Add new VMs** without changing core system logic
- **Run multiple VMs** in the same block (different transactions can use different EEs)
- **Optimize independently** (EVM optimizations don't affect EraVM)
- **Maintain compatibility** (EVM contracts work alongside future VM contracts)

### Execution Environment vs. Bootloader

The relationship between EEs and the bootloader is crucial:

```
┌──────────────────────────────────────────────────────────────┐
│                      Bootloader (Orchestrator)                │
│                                                               │
│  - Manages transaction lifecycle                             │
│  - Handles frame setup and teardown                          │
│  - Performs value transfers                                  │
│  - Manages call stack (recursion)                            │
│  - Enforces system-wide constraints (depth, static context)  │
└──────────────────────────────────────────────────────────────┘
                            ↕
                    (Preemption Points)
                            ↕
┌──────────────────────────────────────────────────────────────┐
│            Execution Environment (Bytecode Executor)          │
│                                                               │
│  - Interprets bytecode                                       │
│  - Manages execution state (stack, memory, PC)               │
│  - Executes opcodes                                          │
│  - Yields control on external calls (preemption)             │
│  - Reports execution results                                 │
└──────────────────────────────────────────────────────────────┘
                            ↕
                    (System Calls)
                            ↕
┌──────────────────────────────────────────────────────────────┐
│                   System Layer (I/O, Resources)              │
│                                                               │
│  - Storage reads/writes (SLOAD/SSTORE)                       │
│  - Event emission (LOG0-LOG4)                                │
│  - Balance queries (BALANCE)                                 │
│  - Bytecode access (EXTCODECOPY)                             │
│  - Resource accounting (gas/native)                          │
└──────────────────────────────────────────────────────────────┘
```

**Key Principle**: The bootloader manages *what* to execute, while the EE manages *how* to execute it.

---

## ExecutionEnvironment Trait

### Trait Definition

**Location**: `zk_ee/src/system/execution_environment/mod.rs:40-101`

```rust
pub trait ExecutionEnvironment<'ee, S: SystemTypes, Es: Subsystem>: Sized {
    const NEEDS_SCRATCH_SPACE: bool;
    const EE_VERSION_BYTE: u8;

    type UsageError = <Es as Subsystem>::Interface;
    type SubsystemError = SubsystemError<Es>;

    /// Initialize a new (empty) EE state.
    fn new(system: &mut System<S>) -> Result<Self, Self::SubsystemError>;

    /// Pre-checks and operations that should not be rolled back if actual frame execution fails.
    fn before_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        system: &mut System<S>,
        frame_state: &mut ExecutionEnvironmentLaunchParams<'i, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<bool, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt;

    /// Start the execution of an EE frame in a given initial state.
    /// Returns a preemption point for the runner to handle.
    fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        frame_state: ExecutionEnvironmentLaunchParams<'i, S>,
        heap: SliceVec<'h, u8>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt;

    /// EE can decide how to provide resources to the callee frame on external call.
    /// Returns resources for the callee frame. Native resource handled by OS itself.
    fn calculate_resources_passed_in_external_call(
        resources_in_caller_frame: &mut S::Resources,
        call_request: &ExternalCallRequest<S>,
        callee_account_properties: &CalleeAccountProperties,
    ) -> Result<S::Resources, Self::SubsystemError>;

    /// Continue the execution of an EE frame after preemtion.
    /// Returns a preemption point for the runner to handle.
    fn continue_after_preemption<'a, 'res: 'ee>(
        &'a mut self,
        system: &mut System<S>,
        returned_resources: S::Resources,
        call_request_result: CallResult<'res, S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
    where
        S::IO: IOSubsystemExt;
}
```

### Lifecycle Methods

The ExecutionEnvironment trait defines a clear lifecycle for frame execution:

#### 1. `new()` - Initialization

```rust
fn new(system: &mut System<S>) -> Result<Self, Self::SubsystemError>;
```

**Purpose**: Create a new, empty execution environment state. This is typically called once at the start of block execution to initialize the EE.

**When called**: Once per EE type at system initialization

**What it does**:
- Allocates internal data structures (e.g., empty stack, empty memory)
- Sets up execution state
- Does NOT start executing any bytecode

**Example (EVM)**: Creates an empty `Interpreter` struct with zeroed fields.

---

#### 2. `before_executing_frame()` - Pre-Execution Setup

```rust
fn before_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
    system: &mut System<S>,
    frame_state: &mut ExecutionEnvironmentLaunchParams<'i, S>,
    tracer: &mut impl Tracer<S>,
) -> Result<bool, Self::SubsystemError>
```

**Purpose**: Perform operations that should happen *before* frame execution begins and should NOT be rolled back if the frame fails.

**When called**: Right before starting a new frame, after frame parameters are validated

**Critical behavior**: Operations here are NOT reverted if execution fails!

**Common operations**:
- **Nonce increment**: Increment sender nonce (happens even if transaction fails)
- **Value transfer**: Transfer ETH/tokens from caller to callee (irreversible)
- **Access list warming**: Pre-warm storage slots and addresses
- **Gas pre-charging**: Charge intrinsic gas costs

**Return value**:
- `Ok(true)`: Frame should execute normally
- `Ok(false)`: Skip frame execution (e.g., nonce overflow, insufficient balance)
- `Err(...)`: Fatal system error

**Why separate?** EIP-161 and other EIPs specify that some operations (like nonce increments) must happen even if the transaction reverts. By separating them into `before_executing_frame`, we ensure they're not rolled back.

**Example flow**:
```
Transaction to contract with 1 ETH value transfer
    ↓
before_executing_frame():
    1. Increment sender nonce (0 → 1)
    2. Check sender balance ≥ 1 ETH
    3. Transfer 1 ETH from sender to receiver
    ↓
If these succeed → Return Ok(true) → Frame executes
If balance check fails → Return Ok(false) → Frame skipped, but nonce still incremented
```

---

#### 3. `start_executing_frame()` - Begin Execution

```rust
fn start_executing_frame<'a, 'i: 'ee, 'h: 'ee>(
    &'a mut self,
    system: &mut System<S>,
    frame_state: ExecutionEnvironmentLaunchParams<'i, S>,
    heap: SliceVec<'h, u8>,
    tracer: &mut impl Tracer<S>,
) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
```

**Purpose**: Start executing a new frame with the given parameters. This is where bytecode interpretation begins.

**When called**: After `before_executing_frame` returns `Ok(true)`

**Parameters**:
- `self`: Mutable reference to EE state (will be updated during execution)
- `system`: System context (I/O, metadata, allocator)
- `frame_state`: All frame parameters (caller, callee, calldata, value, etc.)
- `heap`: Pre-allocated memory for this frame
- `tracer`: Execution tracer for debugging/diagnostics

**What it does**:
1. Extract frame parameters (bytecode, calldata, caller, etc.)
2. Initialize execution state (PC=0, empty stack, etc.)
3. Enter main execution loop
4. Execute opcodes sequentially until:
   - **RETURN/STOP**: Execution completes successfully
   - **REVERT**: Execution fails with return data
   - **CALL/CREATE**: External call needed (preemption)
   - **Error**: Invalid opcode, out of gas, stack error, etc.

**Return value**: `ExecutionEnvironmentPreemptionPoint` indicating why execution stopped:
- `End(CompletedExecution)`: Frame completed (success or failure)
- `CallRequest { request, heap }`: Frame needs to make external call

**Execution is ATOMIC from bootloader's perspective**: Once this method returns, the frame has either:
- Completed entirely
- Or yielded for an external call (which the bootloader must handle)

---

#### 4. `continue_after_preemption()` - Resume After External Call

```rust
fn continue_after_preemption<'a, 'res: 'ee>(
    &'a mut self,
    system: &mut System<S>,
    returned_resources: S::Resources,
    call_request_result: CallResult<'res, S>,
    tracer: &mut impl Tracer<S>,
) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, Self::SubsystemError>
```

**Purpose**: Resume execution after the bootloader has handled an external call (CALL, STATICCALL, DELEGATECALL, CREATE, etc.).

**When called**: After bootloader processes a `CallRequest` preemption point

**Parameters**:
- `self`: Mutable reference to EE state (contains suspended frame)
- `system`: System context
- `returned_resources`: Unused resources returned by callee
- `call_request_result`: Result of the external call (success/failure/preparation failed)
- `tracer`: Execution tracer

**What it does**:
1. Resume from saved execution state
2. Process call result:
   - Update returndata
   - Push success (1) or failure (0) to stack
   - Reclaim unused gas
3. Continue executing opcodes from where it left off
4. Run until next preemption point or completion

**Return value**: Another `ExecutionEnvironmentPreemptionPoint`:
- Could immediately hit another CALL (nested calls)
- Could complete execution
- Could hit an error

**Example flow**:
```
Frame A executing:
    PUSH 0x123        // Push target address
    PUSH 100          // Push gas
    CALL              // Make external call
    ↓
    start_executing_frame() returns CallRequest
    ↓
Bootloader receives CallRequest:
    Execute Frame B (callee)
    ↓
    Frame B completes: CallResult::Successful { returndata: [...] }
    ↓
Bootloader calls continue_after_preemption(Frame A, B's result)
    ↓
Frame A resumes:
    CALL result: Push 1 to stack (success)
    Update returndata
    POP               // Next opcode continues
    ...
```

---

#### 5. `calculate_resources_passed_in_external_call()` - Resource Allocation

```rust
fn calculate_resources_passed_in_external_call(
    resources_in_caller_frame: &mut S::Resources,
    call_request: &ExternalCallRequest<S>,
    callee_account_properties: &CalleeAccountProperties,
) -> Result<S::Resources, Self::SubsystemError>;
```

**Purpose**: Allow the EE to decide how many resources (gas) to pass to a callee frame.

**When called**: By bootloader when processing a `CallRequest`, before starting callee frame

**Parameters**:
- `resources_in_caller_frame`: Available resources in caller (will be deducted)
- `call_request`: The call request containing desired gas
- `callee_account_properties`: Information about callee (bytecode, balance, etc.)

**What it does**: Calculate actual resources to pass to callee, applying EE-specific rules.

**EVM Implementation (EIP-150)**: The "63/64 rule"
```rust
// EIP-150: Keep 1/64 of remaining gas for caller
let max_passable_gas = available_gas - (available_gas / 64);
let actual_gas = min(requested_gas, max_passable_gas);
```

**Why?** Prevents a callee from consuming all gas, ensuring the caller has enough to complete post-call operations (like logging the call result).

**Example**:
```
Caller has 64000 gas remaining
CALL instruction requests 60000 gas
    ↓
calculate_resources_passed_in_external_call():
    max_passable = 64000 - (64000 / 64) = 64000 - 1000 = 63000
    actual = min(60000, 63000) = 60000
    ↓
Callee gets 60000 gas
Caller retains 4000 gas (for post-call operations)
```

---

### Preemption Model

The ExecutionEnvironment uses a **cooperative multitasking** model based on **preemption points**. Instead of directly calling into other contracts, an EE yields control back to the bootloader when it needs to make an external call.

#### Why Preemption?

**Direct calling** (what we DON'T do):
```rust
// ❌ Direct call model (not used)
fn execute_call_opcode() {
    let result = callee.execute();  // Directly enter callee
    handle_result(result);
}
```

**Problems with direct calling**:
1. **Frame management**: Hard to track call stack depth
2. **Resource accounting**: Difficult to enforce 63/64 rule and track refunds
3. **Tracer integration**: Can't trace call boundaries cleanly
4. **Validation**: Can't enforce constraints (static context, value transfers) before entering callee

**Preemption model** (what we DO):
```rust
// ✅ Preemption model (used)
fn execute_call_opcode() -> PreemptionPoint {
    let request = create_call_request();
    PreemptionPoint::CallRequest { request, heap }
    // Return to bootloader, don't execute callee yet
}
```

**Benefits of preemption**:
1. **Clear separation**: Bootloader handles orchestration, EE handles execution
2. **Proper validation**: Bootloader validates calls before executing
3. **Resource control**: Bootloader enforces 63/64 rule and depth limits
4. **Tracer integration**: Clean trace of call tree
5. **Flexibility**: Bootloader can inject logic (e.g., cross-VM calls)

#### ExecutionEnvironmentPreemptionPoint Enum

**Location**: `zk_ee/src/system/execution_environment/environment_state.rs:18-24`

```rust
pub enum ExecutionEnvironmentPreemptionPoint<'a, S: SystemTypes> {
    CallRequest {
        request: ExternalCallRequest<'a, S>,
        heap: SliceVec<'a, u8>,
    },
    End(CompletedExecution<'a, S>),
}
```

**Variants**:

1. **`CallRequest`**: EE needs to make an external call
   - **request**: All call parameters (caller, callee, value, calldata, gas, modifier)
   - **heap**: Current frame's heap (returned to enable heap reuse)
   - **Interpretation**: "I need to call this contract, please handle it"

2. **`End(CompletedExecution)`**: EE has completed execution
   - **CompletedExecution** contains:
     - `resources_returned`: Unused gas/native resources
     - `result`: CallResult (success/failure/preparation failed)
   - **Interpretation**: "I'm done, here's the result"

#### Preemption Flow Diagram

```
┌────────────────────────────────────────────────────────────────┐
│                        Bootloader                               │
│                                                                 │
│  1. Create ExecutionEnvironmentLaunchParams                    │
│  2. Call EE.start_executing_frame(params)                      │
└────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────┐
│                   Execution Environment                         │
│                                                                 │
│  Opcode Loop:                                                   │
│    ┌─────────────────────────────────────┐                    │
│    │ PUSH, ADD, MSTORE, SLOAD, etc.     │                    │
│    │ (Regular opcodes execute normally) │                    │
│    └─────────────────────────────────────┘                    │
│                    ↓                                            │
│    ┌─────────────────────────────────────┐                    │
│    │ CALL opcode encountered             │                    │
│    │ - Create ExternalCallRequest        │                    │
│    │ - Save current state                │                    │
│    │ - Return CallRequest preemption     │                    │
│    └─────────────────────────────────────┘                    │
└────────────────────────────────────────────────────────────────┘
                            ↓
                 PreemptionPoint::CallRequest
                            ↓
┌────────────────────────────────────────────────────────────────┐
│                    Bootloader Receives CallRequest              │
│                                                                 │
│  3. Validate call:                                             │
│     - Check callstack_depth < 1024                             │
│     - Check static context allows this call                    │
│     - Check value transfer is allowed                          │
│                                                                 │
│  4. Query callee properties:                                   │
│     - system.io.read_account_properties(callee_address)        │
│     - Get bytecode, balance, nonce, code_version               │
│                                                                 │
│  5. If value > 0:                                              │
│     - Check caller.balance >= value                            │
│     - system.io.transfer(caller, callee, value)                │
│                                                                 │
│  6. Calculate callee resources:                                │
│     - EE.calculate_resources_passed_in_external_call()         │
│     - Apply 63/64 rule (EVM)                                   │
│                                                                 │
│  7. Create callee ExecutionEnvironmentLaunchParams             │
│                                                                 │
│  8. Recursively call EE.start_executing_frame(callee_params)   │
└────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────┐
│                   Callee Execution Environment                  │
│                                                                 │
│  Execute callee bytecode...                                     │
│    ┌─────────────────────────────────────┐                    │
│    │ May make nested calls               │                    │
│    │ (recursive preemption)              │                    │
│    └─────────────────────────────────────┘                    │
│                    ↓                                            │
│    ┌─────────────────────────────────────┐                    │
│    │ RETURN opcode                       │                    │
│    │ - Return PreemptionPoint::End       │                    │
│    └─────────────────────────────────────┘                    │
└────────────────────────────────────────────────────────────────┘
                            ↓
             PreemptionPoint::End(CompletedExecution)
                            ↓
┌────────────────────────────────────────────────────────────────┐
│                  Bootloader Receives Completion                 │
│                                                                 │
│  9. Extract callee result:                                     │
│     - CallResult::Successful { returndata } OR                 │
│     - CallResult::Failed { returndata } OR                     │
│     - CallResult::PreparationStepFailed                        │
│                                                                 │
│  10. Resume caller:                                            │
│      EE.continue_after_preemption(unused_gas, call_result)     │
└────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────┐
│                Caller Execution Environment Resumes             │
│                                                                 │
│  11. Process call result:                                      │
│      - Update returndata                                       │
│      - Push 1 (success) or 0 (failure) to stack                │
│      - Reclaim unused_gas                                      │
│                                                                 │
│  12. Continue opcode loop:                                     │
│      - Next opcode after CALL                                  │
│      - May hit more CALL opcodes (repeat preemption)           │
│      - Eventually returns PreemptionPoint::End                 │
└────────────────────────────────────────────────────────────────┘
                            ↓
                    (Repeat until top-level completion)
```

#### Preemption Example

**Solidity code**:
```solidity
contract A {
    function foo() public {
        uint256 x = 1 + 2;              // Regular opcodes
        B(0x123...).bar();              // External call (preemption)
        uint256 y = x + 3;              // Resumes here after call
    }
}
```

**EVM execution flow**:
```
Frame A execution:
    PUSH 1
    PUSH 2
    ADD              → Stack: [3]
    PUSH 0x123...    → Stack: [3, 0x123...]
    PUSH 0          → Stack: [3, 0x123..., 0] (value = 0)
    PUSH 32         → Stack: [3, 0x123..., 0, 32] (retSize)
    ...
    CALL            → Preemption! Return CallRequest to bootloader
                       State saved: PC=after CALL, stack=[3], ...

Bootloader handles CallRequest:
    Validate call (depth, static context, etc.)
    Query 0x123... properties
    Calculate gas for callee (63/64 rule)
    start_executing_frame(Frame B)

Frame B execution:
    ... execute bar() ...
    PUSH "result"
    PUSH 32
    RETURN         → Preemption! Return End(CompletedExecution) to bootloader

Bootloader receives Frame B completion:
    Construct CallResult::Successful { returndata: "result" }
    continue_after_preemption(Frame A, result)

Frame A resumes:
    Load saved state: PC=after CALL, stack=[3]
    Process call result: Push 1 to stack → Stack: [3, 1]
    Continue execution:
        PUSH 3     → Stack: [3, 1, 3]
        ADD        → Stack: [3, 4]
        POP        → Stack: [3]
        ...
```

---

## EVM Interpreter

The **EVM Interpreter** is the concrete implementation of the `ExecutionEnvironment` trait for Ethereum Virtual Machine bytecode execution. It's the workhorse of EVM transaction processing in zkSync OS.

**Code Location**: `evm_interpreter/src/lib.rs:81-112`

### Interpreter Struct

```rust
pub struct Interpreter<'a, S: SystemTypes> {
    /// Instruction pointer (program counter).
    pub instruction_pointer: usize,

    /// Implementation of gas accounting on top of system resources.
    pub gas: Gas<S>,

    /// Stack (max 1024 elements).
    pub stack: EvmStack<S::Allocator>,

    /// Caller address (msg.sender in callee context)
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,

    /// Contract address (address(this))
    pub address: <S::IOTypes as SystemIOTypesConfig>::Address,

    /// Input data (msg.data)
    pub calldata: &'a [u8],

    /// Return data from last external call
    pub returndata: &'a [u8],

    /// Heap (EVM memory), sparse vector
    pub heap: SliceVec<'a, u8>,

    /// Range in heap where returndata is stored
    pub returndata_location: Range<usize>,

    /// Executable bytecode
    pub bytecode: &'a [u8],

    /// Preprocessing result (jumpdest bitmap for JUMP validation)
    pub bytecode_preprocessing: BytecodePreprocessingData<'a, S::Allocator>,

    /// Call value (msg.value)
    pub call_value: U256,

    /// Is interpreter call static (STATICCALL context)
    pub is_static: bool,

    /// Is interpreter call executing construction code (CREATE/CREATE2)
    pub is_constructor: bool,

    /// Pending external call request (for preemption)
    pub pending_os_request: Option<PendingOsRequest<S>>,
}
```

### Key Fields Explained

#### instruction_pointer
- **Type**: `usize`
- **Purpose**: Program counter, tracks current position in bytecode
- **Range**: `0..bytecode.len()`
- **Updated by**: Most opcodes increment by 1, JUMP/JUMPI set to arbitrary value, PUSH increments by 1 + push_size
- **Special values**: At `bytecode.len()` means execution reached end without RETURN/STOP (invalid, treated as error)

#### gas
- **Type**: `Gas<S>` (wrapper around `S::Resources`)
- **Purpose**: Tracks both EVM gas (ergs) and native resources
- **Updated by**: Every opcode charges gas via `gas.charge(...)`
- **Checked**: Before each opcode, exit with OutOfGas if exhausted
- **Double accounting**: EVM gas (Ethereum compatibility) + native resources (RISC-V cycles)

#### stack
- **Type**: `EvmStack<S::Allocator>`
- **Capacity**: 1024 elements (EVM spec)
- **Element type**: `U256` (256-bit unsigned integer)
- **Operations**: push, pop, swap, dup
- **Errors**: `StackOverflow` if push to full stack, `StackUnderflow` if pop from empty stack
- **Layout**: Top of stack is at highest index (stack[len-1])

#### caller, address, call_value
- **caller**: The account that made this call (msg.sender)
- **address**: The account whose code is executing (address(this))
- **call_value**: ETH/native tokens transferred with this call (msg.value)
- **Used by opcodes**: CALLER, ADDRESS, CALLVALUE

**DELEGATECALL special case**:
- Normal CALL: `caller = A, address = B` (A calls B)
- DELEGATECALL: `caller = A, address = A` (A executes B's code in A's context)

#### calldata, returndata
- **calldata**: Input data to this call (msg.data)
  - Used by: CALLDATALOAD, CALLDATASIZE, CALLDATACOPY
  - Immutable during execution
- **returndata**: Output from last external call
  - Used by: RETURNDATASIZE, RETURNDATACOPY
  - Updated by: continue_after_preemption when call completes
  - Empty at frame start

#### heap
- **Type**: `SliceVec<'a, u8>` (sparse byte vector)
- **Purpose**: EVM memory
- **Growth**: On-demand, grows to accommodate accesses
- **Gas**: Charges quadratic cost for expansion (EIP-150)
- **Operations**: MLOAD, MSTORE, MSTORE8, RETURN, REVERT, CALL (for calldata/returndata copy)
- **Layout**: Byte-addressable, index 0 is first byte
- **Persistence**: Frame-local, not persisted after frame ends

#### bytecode, bytecode_preprocessing
- **bytecode**: The executable code (e.g., deployed contract bytecode)
- **bytecode_preprocessing**: Pre-computed jumpdest bitmap
  - **Purpose**: JUMP/JUMPI can only jump to JUMPDEST (0x5b) locations
  - **Bitmap**: One bit per byte, set if byte is a valid JUMPDEST
  - **Validation**: `bytecode_preprocessing.is_valid_jumpdest(target)` on JUMP
  - **Performance**: Pre-computed during deployment, cached for subsequent calls

#### is_static, is_constructor
- **is_static**: `true` if in STATICCALL context
  - **Restrictions**: No state changes allowed (SSTORE, LOG, CREATE, CALL with value)
  - **Enforcement**: Opcodes check `is_static` and return `StateChangeDuringStaticCall` error
  - **Propagation**: Child calls inherit `is_static` if parent is static
- **is_constructor**: `true` if executing constructor code (CREATE/CREATE2)
  - **Special behavior**: RETURN in constructor deploys bytecode instead of returning data
  - **Max size**: 24KB deployed code limit, 48KB constructor code limit (EIP-3860)

#### pending_os_request
- **Type**: `Option<PendingOsRequest<S>>`
- **Purpose**: Tracks pending external operation (CALL, CREATE)
- **Set by**: CALL/STATICCALL/DELEGATECALL/CREATE/CREATE2 opcodes
- **Cleared by**: `continue_after_preemption` when call completes
- **Variants**: `Call`, `Create`
- **Why needed?** Allows interpreter to remember what kind of call it was waiting for

### Opcode Categories

The EVM interpreter implements all 140+ EVM opcodes organized into functional categories:

#### Stack Manipulation

**PUSH1-PUSH32** (0x60-0x7f):
```
PUSH1 0x42    → Stack: [0x42]
PUSH2 0x1234  → Stack: [0x1234]
...
PUSH32 0xABCD...  → Stack: [0xABCD...] (32 bytes)
```
- Push 1-32 bytes from bytecode onto stack
- Gas: 3
- Most common opcodes in EVM bytecode

**POP** (0x50):
```
Stack: [A, B, C]
POP
Stack: [A, B]
```
- Remove top stack item
- Gas: 2

**DUP1-DUP16** (0x80-0x8f):
```
Stack: [A, B, C]
DUP1  (duplicate 1st item)
Stack: [A, B, C, C]

DUP3  (duplicate 3rd item)
Stack: [A, B, C, A]
```
- Duplicate stack item at position N
- Gas: 3

**SWAP1-SWAP16** (0x90-0x9f):
```
Stack: [A, B, C, D]
SWAP1  (swap top two)
Stack: [A, B, D, C]

SWAP3  (swap top with 4th)
Stack: [D, B, C, A]
```
- Swap top stack item with item at position N+1
- Gas: 3

#### Arithmetic & Logic

**Arithmetic** (ADD, SUB, MUL, DIV, MOD, EXP):
```
Stack: [5, 3]
ADD
Stack: [8]

Stack: [10, 3]
DIV
Stack: [3]  (integer division)

Stack: [2, 10]
EXP
Stack: [1024]  (2^10)
```
- Gas: 3 (ADD/SUB/MUL), 5 (DIV/MOD), 10-50 (EXP, depends on exponent)

**Comparison** (LT, GT, EQ, ISZERO):
```
Stack: [5, 3]
LT
Stack: [0]  (5 < 3 is false)

Stack: [5, 5]
EQ
Stack: [1]  (5 == 5 is true)

Stack: [0]
ISZERO
Stack: [1]  (0 == 0 is true)
```
- Gas: 3
- Return 0 (false) or 1 (true)

**Bitwise** (AND, OR, XOR, NOT, SHL, SHR, SAR, BYTE):
```
Stack: [0xFF, 0x0F]
AND
Stack: [0x0F]

Stack: [0x1234, 8]
SHL
Stack: [0x123400]  (shift left 8 bits)

Stack: [0xABCDEF, 1]
BYTE
Stack: [0xCD]  (get byte at index 1)
```
- Gas: 3

#### Memory Operations

**MLOAD** (0x51):
```
Stack: [0x40]
MLOAD
Stack: [memory[0x40:0x60]]  (32 bytes)
```
- Load 32 bytes from memory at offset
- Gas: 3 + memory expansion cost

**MSTORE** (0x52):
```
Stack: [0x40, 0x123...]
MSTORE
Stack: []
// memory[0x40:0x60] = 0x123... (32 bytes)
```
- Store 32 bytes to memory at offset
- Gas: 3 + memory expansion cost

**MSTORE8** (0x53):
```
Stack: [0x80, 0xFF]
MSTORE8
Stack: []
// memory[0x80] = 0xFF (1 byte)
```
- Store 1 byte to memory at offset
- Gas: 3 + memory expansion cost

**MSIZE** (0x59):
```
MSIZE
Stack: [memory_size_in_bytes]
```
- Return current memory size
- Gas: 2
- Memory size always multiple of 32

#### Storage Operations

**SLOAD** (0x54):
```
Stack: [storage_key]
SLOAD
Stack: [storage_value]
```
- Load 32 bytes from persistent storage
- Gas: 100 (warm) or 2100 (cold)
- Queries: `system.io.storage_read(address, key)`

**SSTORE** (0x55):
```
Stack: [storage_key, value]
SSTORE
Stack: []
```
- Store 32 bytes to persistent storage
- Gas: Complex (100-22100, depends on current/original values, see EIP-2200)
- Updates: `system.io.storage_write(address, key, value)`

**Gas calculation for SSTORE** (simplified):
- Setting zero to non-zero: 20000 gas (new slot)
- Setting non-zero to non-zero: 5000 gas (modification)
- Setting non-zero to zero: 5000 gas + 15000 refund (clearing)
- Setting to same value: 100 gas (warm no-op)

#### Control Flow

**JUMP** (0x56):
```
Stack: [target_offset]
JUMP
Stack: []
// instruction_pointer = target_offset
```
- Unconditional jump to target
- Gas: 8
- Target MUST be JUMPDEST (0x5b), otherwise `InvalidJump` error

**JUMPI** (0x57):
```
Stack: [target_offset, condition]
JUMPI
Stack: []
// if condition != 0: instruction_pointer = target_offset
// else: instruction_pointer += 1
```
- Conditional jump
- Gas: 10
- Target MUST be JUMPDEST if jump taken

**JUMPDEST** (0x5b):
```
JUMPDEST
```
- Valid jump destination (marker opcode)
- Gas: 1
- Does nothing except mark valid jump target

**PC** (0x58):
```
PC
Stack: [current_instruction_pointer]
```
- Push current program counter
- Gas: 2
- Value is offset of PC opcode itself

**STOP** (0x00):
```
STOP
```
- Halt execution successfully
- Return empty data
- Gas: 0

**RETURN** (0xf3):
```
Stack: [offset, length]
RETURN
// Return memory[offset:offset+length]
```
- Halt execution successfully
- Return specified memory range as output
- Gas: 0 + memory expansion cost

**REVERT** (0xfd):
```
Stack: [offset, length]
REVERT
// Revert with memory[offset:offset+length] as error data
```
- Halt execution with failure
- Return error data (e.g., revert reason)
- Gas: 0 + memory expansion cost (unused gas refunded)

#### External Calls

**CALL** (0xf1):
```
Stack: [gas, addr, value, argsOffset, argsSize, retOffset, retSize]
CALL
Stack: [success]  (1 or 0)
```
- Call external contract
- Transfers `value` wei to `addr`
- Sends `memory[argsOffset:argsOffset+argsSize]` as calldata
- Stores returndata in `memory[retOffset:retOffset+retSize]`
- Gas: 100-9000 base + gas passed to callee
- **Preempts**: Returns `CallRequest` to bootloader

**STATICCALL** (0xfa):
```
Stack: [gas, addr, argsOffset, argsSize, retOffset, retSize]
STATICCALL
Stack: [success]  (1 or 0)
```
- Like CALL but:
  - No value transfer (static context)
  - Callee cannot modify state
- Gas: 100-9000 base + gas passed to callee

**DELEGATECALL** (0xf4):
```
Stack: [gas, addr, argsOffset, argsSize, retOffset, retSize]
DELEGATECALL
Stack: [success]  (1 or 0)
```
- Like CALL but:
  - No value transfer
  - Executes code at `addr` in current contract's context
  - `msg.sender` and `address(this)` remain caller's values
- Gas: 100-9000 base + gas passed to callee

**CALLCODE** (0xf2):
```
Stack: [gas, addr, value, argsOffset, argsSize, retOffset, retSize]
CALLCODE
Stack: [success]  (1 or 0)
```
- Deprecated (use DELEGATECALL instead)
- Like DELEGATECALL but transfers value
- Gas: 100-9000 base + gas passed to callee

**CREATE** (0xf0):
```
Stack: [value, offset, length]
CREATE
Stack: [new_contract_address or 0]
```
- Deploy new contract
- Sends `value` wei to new contract
- Constructor code: `memory[offset:offset+length]`
- Returns new address on success, 0 on failure
- Gas: 32000 + constructor gas
- **Preempts**: Returns `CallRequest` with modifier `Constructor`

**CREATE2** (0xf5):
```
Stack: [value, offset, length, salt]
CREATE2
Stack: [new_contract_address or 0]
```
- Like CREATE but with deterministic address
- Address = keccak256(0xff ++ sender ++ salt ++ keccak256(init_code))
- Gas: 32000 + constructor gas

#### Logging (Events)

**LOG0-LOG4** (0xa0-0xa4):
```
LOG0: Stack: [offset, length]
LOG1: Stack: [offset, length, topic1]
LOG2: Stack: [offset, length, topic1, topic2]
LOG3: Stack: [offset, length, topic1, topic2, topic3]
LOG4: Stack: [offset, length, topic1, topic2, topic3, topic4]

After LOG:
Stack: []
```
- Emit EVM event
- Data: `memory[offset:offset+length]`
- Topics: 0-4 indexed parameters (32 bytes each)
- Gas: 375 + 375*num_topics + 8*data_length
- Calls: `system.io.emit_event(address, topics, data)`

**Solidity event example**:
```solidity
event Transfer(address indexed from, address indexed to, uint256 value);

emit Transfer(msg.sender, recipient, amount);
// Generates LOG3:
//   topic0 = keccak256("Transfer(address,address,uint256)")
//   topic1 = from
//   topic2 = to
//   data = abi.encode(amount)
```

#### Context/Environment Opcodes

**ADDRESS** (0x30):
```
ADDRESS
Stack: [address(this)]
```
- Push current contract address
- Gas: 2

**BALANCE** (0x31):
```
Stack: [addr]
BALANCE
Stack: [balance_of_addr]
```
- Query balance of address
- Gas: 100 (warm) or 2600 (cold)

**ORIGIN** (0x32):
```
ORIGIN
Stack: [tx.origin]
```
- Push transaction originator (not msg.sender!)
- Gas: 2

**CALLER** (0x33):
```
CALLER
Stack: [msg.sender]
```
- Push caller address
- Gas: 2

**CALLVALUE** (0x34):
```
CALLVALUE
Stack: [msg.value]
```
- Push call value (wei transferred)
- Gas: 2

**CALLDATALOAD** (0x35):
```
Stack: [offset]
CALLDATALOAD
Stack: [calldata[offset:offset+32]]
```
- Load 32 bytes from calldata
- Gas: 3
- Out of bounds: returns 0

**CALLDATASIZE** (0x36):
```
CALLDATASIZE
Stack: [len(calldata)]
```
- Push calldata size
- Gas: 2

**CALLDATACOPY** (0x37):
```
Stack: [destOffset, offset, length]
CALLDATACOPY
Stack: []
// memory[destOffset:destOffset+length] = calldata[offset:offset+length]
```
- Copy calldata to memory
- Gas: 3 + 3*length + memory expansion

**CODESIZE** (0x38):
```
CODESIZE
Stack: [len(bytecode)]
```
- Push bytecode size
- Gas: 2

**CODECOPY** (0x39):
```
Stack: [destOffset, offset, length]
CODECOPY
Stack: []
// memory[destOffset:destOffset+length] = bytecode[offset:offset+length]
```
- Copy bytecode to memory
- Gas: 3 + 3*length + memory expansion

**EXTCODESIZE** (0x3b):
```
Stack: [addr]
EXTCODESIZE
Stack: [size_of_code_at_addr]
```
- Query code size at address
- Gas: 100 (warm) or 2600 (cold)

**EXTCODECOPY** (0x3c):
```
Stack: [addr, destOffset, offset, length]
EXTCODECOPY
Stack: []
// memory[destOffset:destOffset+length] = code_at_addr[offset:offset+length]
```
- Copy external bytecode to memory
- Gas: 100-2600 + 3*length + memory expansion

**EXTCODEHASH** (0x3f):
```
Stack: [addr]
EXTCODEHASH
Stack: [keccak256(code_at_addr)]
```
- Query code hash at address
- Returns 0 if account is empty or has no code
- Gas: 100 (warm) or 2600 (cold)

**RETURNDATASIZE** (0x3d):
```
RETURNDATASIZE
Stack: [len(returndata)]
```
- Push size of returndata from last call
- Gas: 2

**RETURNDATACOPY** (0x3e):
```
Stack: [destOffset, offset, length]
RETURNDATACOPY
Stack: []
// memory[destOffset:destOffset+length] = returndata[offset:offset+length]
```
- Copy returndata to memory
- Gas: 3 + 3*length + memory expansion
- Error if offset+length > len(returndata)

#### Block Context Opcodes

**BLOCKHASH** (0x40):
```
Stack: [block_number]
BLOCKHASH
Stack: [hash_of_block or 0]
```
- Query hash of historical block
- Only available for last 256 blocks
- Gas: 20

**COINBASE** (0x41):
```
COINBASE
Stack: [block.coinbase]
```
- Push block beneficiary address
- Gas: 2

**TIMESTAMP** (0x42):
```
TIMESTAMP
Stack: [block.timestamp]
```
- Push block timestamp (Unix seconds)
- Gas: 2

**NUMBER** (0x43):
```
NUMBER
Stack: [block.number]
```
- Push block number
- Gas: 2

**DIFFICULTY / PREVRANDAO** (0x44):
```
DIFFICULTY
Stack: [block.difficulty or block.prevrandao]
```
- PoW: mining difficulty
- PoS: beacon chain randomness (prevRandao)
- Gas: 2

**GASLIMIT** (0x45):
```
GASLIMIT
Stack: [block.gaslimit]
```
- Push block gas limit
- Gas: 2

**CHAINID** (0x46):
```
CHAINID
Stack: [chain_id]
```
- Push chain ID (e.g., 1 for Ethereum mainnet)
- Gas: 2

**SELFBALANCE** (0x47):
```
SELFBALANCE
Stack: [balance(address(this))]
```
- Query own balance (cheaper than BALANCE)
- Gas: 5

**BASEFEE** (0x48):
```
BASEFEE
Stack: [block.basefee]
```
- Push EIP-1559 base fee per gas
- Gas: 2

#### Cryptography

**SHA3 / KECCAK256** (0x20):
```
Stack: [offset, length]
SHA3
Stack: [keccak256(memory[offset:offset+length])]
```
- Compute Keccak-256 hash of memory region
- Gas: 30 + 6*length (per word)

#### Other

**GAS** (0x5a):
```
GAS
Stack: [remaining_gas]
```
- Push remaining gas
- Gas: 2
- Returns gas BEFORE charging for this opcode

**INVALID** (0xfe):
```
INVALID
```
- Designated invalid opcode
- Consumes all gas
- Used by Solidity for unreachable code

**SELFDESTRUCT** (0xff):
```
Stack: [beneficiary_addr]
SELFDESTRUCT
Stack: []
```
- Mark contract for destruction at end of transaction
- Transfer all balance to beneficiary
- Gas: 5000 + 25000 (if creating new account for beneficiary)
- Actual destruction happens in `io.finish_tx()`

### Gas Accounting

The EVM interpreter implements a **double accounting** model for resources:

1. **EVM Gas (Ergs)**: Ethereum-compatible gas costs
2. **Native Resources**: RISC-V cycle costs

**Why both?**
- **Ergs**: Ensures Ethereum compatibility (same gas costs as mainnet)
- **Native**: Prevents DoS via operations that are cheap in EVM but expensive in RISC-V

**Conversion**: 1 EVM gas = 256 ergs

**Code location**: Gas accounting constants and logic are implemented throughout the interpreter, with core definitions in `evm_interpreter/src/lib.rs` and gas cost constants following the Ethereum Yellow Paper.

#### Gas Charging Flow

```rust
// Pseudocode for opcode execution with gas
fn execute_opcode(&mut self, opcode: u8) -> Result<()> {
    // 1. Calculate gas cost
    let ergs_cost = match opcode {
        ADD => Ergs(3 * 256),                    // 3 gas
        SLOAD => self.calculate_sload_cost()?,   // 100 or 2100 gas
        SHA3 => self.calculate_sha3_cost()?,     // 30 + 6*words gas
        ...
    };

    // 2. Calculate native cost
    let native_cost = match opcode {
        ADD => Native::from_computational(10),        // ~10 RISC-V cycles
        SLOAD => Native::from_computational(5000),    // ~5000 cycles
        SHA3 => self.calculate_sha3_native_cost()?,   // Depends on length
        ...
    };

    // 3. Charge both resources
    let resources = Resources::from_ergs_and_native(ergs_cost, native_cost);
    self.gas.resources.charge(&resources)?;  // Error if out of gas

    // 4. Execute opcode
    match opcode {
        ADD => {
            let a = self.stack.pop()?;
            let b = self.stack.pop()?;
            self.stack.push(a + b)?;
        }
        ...
    }

    Ok(())
}
```

#### Common Gas Costs

**Arithmetic & Logic**:
- ADD, SUB, MUL, DIV, MOD, LT, GT, EQ, ISZERO, AND, OR, XOR, NOT, BYTE, SHL, SHR, SAR: **3 gas**
- EXP: **10 gas + 50 gas per byte** in exponent

**Stack & Memory**:
- POP: **2 gas**
- PUSH1-PUSH32: **3 gas**
- DUP1-DUP16, SWAP1-SWAP16: **3 gas**
- MLOAD, MSTORE: **3 gas** + memory expansion cost
- MSTORE8: **3 gas** + memory expansion cost

**Storage**:
- SLOAD (warm): **100 gas**
- SLOAD (cold): **2100 gas**
- SSTORE: **100-22100 gas** (complex, see EIP-2200)

**Control Flow**:
- JUMP: **8 gas**
- JUMPI: **10 gas**
- PC: **2 gas**
- JUMPDEST: **1 gas**

**Calls**:
- CALL, CALLCODE, DELEGATECALL, STATICCALL: **100 base gas** + address access cost (2600 cold, 100 warm) + value transfer cost (9000 if value > 0) + gas passed to callee
- CREATE, CREATE2: **32000 gas** + constructor gas

**Logging**:
- LOG0: **375 gas** + 8 gas per byte
- LOG1: **375 + 375 = 750 gas** + 8 gas per byte
- LOG2: **375 + 750 = 1125 gas** + 8 gas per byte
- LOG3: **375 + 1125 = 1500 gas** + 8 gas per byte
- LOG4: **375 + 1500 = 1875 gas** + 8 gas per byte

**Context/Environment**:
- ADDRESS, CALLER, CALLVALUE, CALLDATASIZE, CODESIZE, RETURNDATASIZE, ORIGIN, GASPRICE, COINBASE, TIMESTAMP, NUMBER, DIFFICULTY, GASLIMIT, CHAINID, BASEFEE: **2 gas**
- SELFBALANCE: **5 gas**
- BALANCE, EXTCODESIZE, EXTCODEHASH: **100 gas** (warm) or **2600 gas** (cold)

**Memory Expansion**:
When accessing memory beyond current size:
```
new_size = ceil((highest_accessed_offset + 1) / 32) * 32
memory_cost = (new_size / 32)^2 / 512 + 3 * (new_size / 32)
expansion_cost = memory_cost - previous_memory_cost
```

**Example**: Expanding from 0 to 64 bytes:
```
new_size = 64
memory_cost = (64/32)^2 / 512 + 3 * (64/32) = 4/512 + 6 = 6.0078 ≈ 6 gas
```

### Stack Implementation

**Type**: `EvmStack<S::Allocator>`
**Capacity**: 1024 elements (EVM specification)
**Element size**: 32 bytes (U256)

**Operations**:
```rust
pub trait StackInterface {
    fn push(&mut self, value: U256) -> Result<(), StackOverflow>;
    fn pop(&mut self) -> Result<U256, StackUnderflow>;
    fn swap(&mut self, depth: usize) -> Result<(), StackError>;
    fn dup(&mut self, depth: usize) -> Result<(), StackError>;
    fn peek(&self, depth: usize) -> Result<&U256, StackError>;
    fn len(&self) -> usize;
}
```

**Stack layout**:
```
Index 0 (bottom):  [First pushed value]
Index 1:           [Second pushed value]
...
Index N-1 (top):   [Last pushed value]
```

**DUP semantics**:
- DUP1: Duplicate top element (index N-1)
- DUP2: Duplicate 2nd element (index N-2)
- ...
- DUP16: Duplicate 16th element (index N-16)

**SWAP semantics**:
- SWAP1: Swap top two elements (N-1 ↔ N-2)
- SWAP2: Swap top with 3rd (N-1 ↔ N-3)
- ...
- SWAP16: Swap top with 17th (N-1 ↔ N-17)

### Memory Implementation

**Type**: `SliceVec<'a, u8>` (sparse byte vector)

**Characteristics**:
- **Byte-addressable**: Access any byte via offset
- **Sparse**: Only allocates pages that are written to
- **Growable**: Automatically expands on access
- **Temporary**: Exists only for duration of frame
- **Gas costs**: Quadratic memory expansion cost

**Operations**:
- MLOAD: Read 32 bytes at offset
- MSTORE: Write 32 bytes at offset
- MSTORE8: Write 1 byte at offset
- CALLDATACOPY, CODECOPY, RETURNDATACOPY, EXTCODECOPY: Copy ranges

**Example**:
```
Initial: memory = []

MSTORE(0x40, 0x123...)
    → Expand to 0x60 (96 bytes)
    → memory[0x40:0x60] = 0x123...

MLOAD(0x20)
    → Read memory[0x20:0x40]
    → memory already large enough, no expansion

MSTORE(0x1000, 0xABC...)
    → Expand to 0x1020 (4128 bytes)
    → Charge memory expansion gas
    → memory[0x1000:0x1020] = 0xABC...
```

### External Call Handling

When the interpreter encounters a CALL, STATICCALL, DELEGATECALL, CREATE, or CREATE2 opcode:

1. **Prepare call request**:
```rust
let request = ExternalCallRequest {
    available_resources: self.gas.resources.clone(),
    ergs_to_pass: gas_requested,
    caller: self.address,
    callee: target_address,
    callers_caller: self.caller,
    modifier: CallModifier::NoModifier,  // or Static, Delegate, Constructor
    input: calldata,
    nominal_token_value: value,
    call_scratch_space: Some(scratch),
};
```

2. **Set pending request**:
```rust
self.pending_os_request = Some(PendingOsRequest::Call);
```

3. **Return preemption**:
```rust
return Ok(ExecutionEnvironmentPreemptionPoint::CallRequest {
    request,
    heap: self.heap,
});
```

4. **Bootloader handles call** (see [Preemption Flow Diagram](#preemption-flow-diagram))

5. **Resume via `continue_after_preemption`**:
```rust
pub fn continue_after_preemption(
    &mut self,
    system: &mut System<S>,
    returned_resources: S::Resources,
    call_request_result: CallResult<S>,
    tracer: &mut impl Tracer<S>,
) -> Result<ExecutionEnvironmentPreemptionPoint<S>, SubsystemError> {
    // 1. Clear pending request
    self.pending_os_request = None;

    // 2. Update returndata
    match &call_request_result {
        CallResult::Successful { return_values } => {
            self.returndata = return_values.returndata;
            self.stack.push(U256::from(1))?;  // Success
        }
        CallResult::Failed { return_values } => {
            self.returndata = return_values.returndata;
            self.stack.push(U256::from(0))?;  // Failure
        }
        CallResult::PreparationStepFailed => {
            self.returndata = &[];
            self.stack.push(U256::from(0))?;  // Failure
        }
    }

    // 3. Reclaim returned resources
    self.gas.resources.add(&returned_resources);

    // 4. Continue execution
    self.run(system, tracer)
}
```

---

## Future Execution Environments

The ExecutionEnvironment abstraction is designed to support multiple VMs beyond EVM. Here are placeholders for future implementations:

### EraVM (Planned)

**EraVM** is a zkSync-specific virtual machine optimized for zero-knowledge proofs.

**Characteristics**:
- Custom instruction set designed for ZK proving efficiency
- Native support for elliptic curve operations
- Optimized for RISC-V proving backend
- Backward compatible with EVM (can call EVM contracts)

**Implementation status**: Not yet implemented

**API**: Will implement `ExecutionEnvironment` trait
```rust
pub struct EraVMInterpreter<'a, S: SystemTypes> {
    // EraVM-specific state
    // ...
}

impl<'ee, S: SystemTypes> ExecutionEnvironment<'ee, S> for EraVMInterpreter<'_, S> {
    // Implement trait methods
    // ...
}
```

**How to distinguish**: Contracts will have a version byte in their account data
- Version 0: EVM bytecode
- Version 1: EraVM bytecode (future)

### WebAssembly (Planned)

**Wasm** execution environment would enable running WebAssembly bytecode on zkSync.

**Characteristics**:
- Industry-standard bytecode format
- Portable across platforms
- Extensive tooling support (compilers for C, Rust, Go, etc.)
- Sandboxed execution model

**Implementation status**: Not yet implemented

**API**: Will implement `ExecutionEnvironment` trait
```rust
pub struct WasmInterpreter<'a, S: SystemTypes> {
    // Wasm-specific state
    // ...
}

impl<'ee, S: SystemTypes> ExecutionEnvironment<'ee, S> for WasmInterpreter<'_, S> {
    // Implement trait methods
    // ...
}
```

**Challenges**:
- Mapping Wasm memory model to EVM storage model
- Handling Wasm imports (equivalent to EVM external calls)
- Gas metering for Wasm instructions

### How to Implement a New Execution Environment

To add a new EE to zkSync OS:

1. **Define EE state struct**:
```rust
pub struct MyEE<'a, S: SystemTypes> {
    // Your execution state
    program_counter: usize,
    registers: Vec<U256>,
    // ...
}
```

2. **Implement `ExecutionEnvironment` trait**:
```rust
impl<'ee, S: SystemTypes> ExecutionEnvironment<'ee, S> for MyEE<'_, S> {
    const NEEDS_SCRATCH_SPACE: bool = true;  // Or false
    const EE_VERSION_BYTE: u8 = 2;  // Unique version byte

    fn new(system: &mut System<S>) -> Result<Self, Self::SubsystemError> {
        // Initialize empty state
        Ok(Self {
            program_counter: 0,
            registers: Vec::new(),
            // ...
        })
    }

    fn before_executing_frame(...) -> Result<bool, Self::SubsystemError> {
        // Pre-execution setup (nonce, value transfer)
        // ...
        Ok(true)
    }

    fn start_executing_frame(...) -> Result<ExecutionEnvironmentPreemptionPoint, Self::SubsystemError> {
        // Main execution loop
        // ...
        // Return CallRequest or End
    }

    fn continue_after_preemption(...) -> Result<ExecutionEnvironmentPreemptionPoint, Self::SubsystemError> {
        // Resume after call
        // ...
    }

    fn calculate_resources_passed_in_external_call(...) -> Result<S::Resources, Self::SubsystemError> {
        // Decide how much gas to pass to callee
        // ...
    }
}
```

3. **Register with bootloader**:
```rust
// In bootloader code
let ee = match account_version_byte {
    0 => ExecutionEnvironmentType::EVM,
    1 => ExecutionEnvironmentType::EraVM,
    2 => ExecutionEnvironmentType::MyEE,
    _ => return Err(UnsupportedEEVersion),
};
```

4. **Test**:
- Unit tests for opcode execution
- Integration tests with bootloader
- Cross-EE call tests (EVM → MyEE, MyEE → EVM)

---

## Execution Flow Diagram

This diagram shows the complete execution flow from transaction submission to completion, focusing on the interaction between the bootloader, EE, and system layer.

```
┌────────────────────────────────────────────────────────────────────┐
│                       Transaction Submitted                         │
│                                                                     │
│  Input: Transaction {                                               │
│    from: 0xABCD...,                                                │
│    to: 0x1234...,                                                  │
│    value: 1 ETH,                                                   │
│    data: 0x...,                                                    │
│    gas: 100000,                                                    │
│  }                                                                  │
└────────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────────┐
│                  Bootloader Receives Transaction                    │
│                                                                     │
│  1. Validate transaction (signature, nonce, gas limit)             │
│  2. Query sender account properties                                │
│  3. Deduct gas cost from sender balance                            │
└────────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────────┐
│          Bootloader Creates ExecutionEnvironmentLaunchParams        │
│                                                                     │
│  external_call: ExternalCallRequest {                              │
│    available_resources: Resources { ergs: 25600000, native: ... }, │
│    ergs_to_pass: 25600000,  // 100000 gas * 256                   │
│    caller: 0xABCD...,       // Transaction sender                  │
│    callee: 0x1234...,       // Contract address                    │
│    callers_caller: 0xABCD...,                                      │
│    modifier: NoModifier,                                           │
│    input: 0x...,            // Transaction data                    │
│    nominal_token_value: 1 ETH,                                     │
│  }                                                                  │
│  environment_parameters: EnvironmentParameters {                   │
│    callstack_depth: 0,                                             │
│    callee_account_properties: {                                    │
│      bytecode: [0x60, 0x80, ...],                                 │
│      bytecode_hash: 0x...,                                        │
│      balance: 10 ETH,                                             │
│      ...                                                           │
│    },                                                              │
│  }                                                                  │
└────────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────────┐
│       Bootloader Calls EE.before_executing_frame()                 │
│                                                                     │
│  - Increment sender nonce: 5 → 6                                  │
│  - Transfer value: sender.balance -= 1 ETH, callee.balance += 1 ETH│
│  - Returns Ok(true) → Continue to execution                        │
└────────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────────┐
│       Bootloader Calls EE.start_executing_frame(params)            │
└────────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────────┐
│                  EVM Interpreter Initializes                        │
│                                                                     │
│  Interpreter {                                                      │
│    instruction_pointer: 0,                                         │
│    gas: Resources { ergs: 25600000, ... },                        │
│    stack: [],                                                      │
│    heap: [],                                                       │
│    bytecode: [0x60, 0x80, 0x60, 0x40, ...],                       │
│    caller: 0xABCD...,                                              │
│    address: 0x1234...,                                             │
│    calldata: 0x...,                                                │
│    call_value: 1 ETH,                                              │
│    is_static: false,                                               │
│    ...                                                             │
│  }                                                                  │
└────────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────────┐
│                    EVM Opcode Execution Loop                        │
│                                                                     │
│  loop {                                                             │
│    ┌────────────────────────────────────────────────────┐         │
│    │ 1. Fetch opcode at bytecode[instruction_pointer]  │         │
│    └────────────────────────────────────────────────────┘         │
│                        ↓                                            │
│    ┌────────────────────────────────────────────────────┐         │
│    │ 2. Calculate gas cost (ergs + native)             │         │
│    └────────────────────────────────────────────────────┘         │
│                        ↓                                            │
│    ┌────────────────────────────────────────────────────┐         │
│    │ 3. Charge gas: gas.resources.charge(cost)?         │         │
│    │    If insufficient → OutOfGas error                │         │
│    └────────────────────────────────────────────────────┘         │
│                        ↓                                            │
│    ┌────────────────────────────────────────────────────┐         │
│    │ 4. Execute opcode:                                 │         │
│    │                                                     │         │
│    │  PUSH1 0x42:                                       │         │
│    │    stack.push(0x42)                                │         │
│    │    instruction_pointer += 2                        │         │
│    │                                                     │         │
│    │  ADD:                                              │         │
│    │    b = stack.pop()                                 │         │
│    │    a = stack.pop()                                 │         │
│    │    stack.push(a + b)                               │         │
│    │    instruction_pointer += 1                        │         │
│    │                                                     │         │
│    │  SLOAD:                                            │         │
│    │    key = stack.pop()                               │         │
│    │    value = system.io.storage_read(address, key)    │         │
│    │    stack.push(value)                               │         │
│    │    instruction_pointer += 1                        │         │
│    │                                                     │         │
│    │  LOG2:                                             │         │
│    │    offset, length, topic1, topic2 = stack.pop(4)  │         │
│    │    data = heap[offset..offset+length]              │         │
│    │    system.io.emit_event(address, [topic1,topic2], data)│     │
│    │    instruction_pointer += 1                        │         │
│    │                                                     │         │
│    │  CALL:                                             │         │
│    │    Create ExternalCallRequest                      │         │
│    │    pending_os_request = Some(Call)                 │         │
│    │    return CallRequest preemption ─────────────┐   │         │
│    │                                                │   │         │
│    │  RETURN:                                      │   │         │
│    │    offset, length = stack.pop(2)              │   │         │
│    │    returndata = heap[offset..offset+length]    │   │         │
│    │    return End(Successful) preemption ─────────┤   │         │
│    │                                                │   │         │
│    │  REVERT:                                      │   │         │
│    │    offset, length = stack.pop(2)              │   │         │
│    │    returndata = heap[offset..offset+length]    │   │         │
│    │    return End(Failed) preemption ──────────────┤   │         │
│    └────────────────────────────────────────────────┘   │         │
│  }                                                      │         │
└─────────────────────────────────────────────────────────┼─────────┘
                                                          │
                                        ┌─────────────────┴─────────┐
                                        │                           │
                          ┌─────────────▼──────────┐  ┌────────────▼────────┐
                          │ CallRequest Preemption │  │ End Preemption      │
                          └─────────────┬──────────┘  └────────────┬────────┘
                                        │                           │
                                        │                           │
┌───────────────────────────────────────▼───────────┐              │
│      Bootloader Handles CallRequest                │              │
│                                                    │              │
│  1. Validate:                                     │              │
│     - callstack_depth < 1024                      │              │
│     - static context rules                        │              │
│     - value transfer allowed                      │              │
│                                                    │              │
│  2. Query callee properties:                      │              │
│     callee_props = system.io.read_account_properties(callee)     │
│                                                    │              │
│  3. Calculate resources:                          │              │
│     callee_gas = EE.calculate_resources_passed_in_external_call()│
│     (63/64 rule for EVM)                          │              │
│                                                    │              │
│  4. Create callee params                          │              │
│                                                    │              │
│  5. Recursively execute callee:                   │              │
│     callee_result = EE.start_executing_frame(callee_params)      │
│     ↓                                              │              │
│     [Callee executes... may have nested calls...] │              │
│     ↓                                              │              │
│     callee_result = End(CompletedExecution)       │              │
│                                                    │              │
│  6. Construct CallResult:                         │              │
│     match callee_result.result {                  │              │
│       Success → CallResult::Successful,           │              │
│       Revert → CallResult::Failed,                │              │
│     }                                              │              │
│                                                    │              │
│  7. Resume caller:                                │              │
│     EE.continue_after_preemption(caller, unused_gas, call_result)│
└───────────────────────────────────────┬───────────┘              │
                                        │                          │
                                        ↓                          │
┌───────────────────────────────────────────────────┐              │
│      Caller EE Resumes After Call                 │              │
│                                                    │              │
│  - Clear pending_os_request                       │              │
│  - Update returndata                              │              │
│  - Push success (1) or failure (0) to stack       │              │
│  - Reclaim unused gas                             │              │
│  - Continue opcode loop                           │              │
│  - May hit more CALL opcodes (repeat process)     │              │
│  - Eventually returns End preemption ──────────────┼──────────────┘
└───────────────────────────────────────────────────┘              │
                                                                   │
┌──────────────────────────────────────────────────────────────────▼──┐
│               Bootloader Receives Final End Preemption              │
│                                                                     │
│  CompletedExecution {                                               │
│    resources_returned: Resources { ergs: 12800000, ... }, // Unused│
│    result: CallResult::Successful {                                │
│      return_values: ReturnValues {                                 │
│        returndata: [0x00, 0x00, ..., 0x01],  // Return data        │
│      },                                                             │
│    },                                                               │
│  }                                                                  │
└────────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────────┐
│                   Bootloader Finalizes Transaction                  │
│                                                                     │
│  1. Calculate gas used:                                            │
│     gas_used = 100000 - (12800000 / 256) = 50000                  │
│                                                                     │
│  2. Refund unused gas to sender:                                   │
│     refund = 50000 * gas_price                                     │
│     sender.balance += refund                                       │
│                                                                     │
│  3. Finalize transaction:                                          │
│     system.io.finish_tx()                                          │
│     - Apply SELFDESTRUCT                                           │
│     - Clear transient storage                                      │
│     - Commit storage changes                                       │
│                                                                     │
│  4. Collect outputs:                                               │
│     - events_storage → Transaction receipt logs                    │
│     - logs_storage → L2→L1 messages                                │
│     - storage changes → State diffs                                │
└────────────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────────────┐
│                     Transaction Complete                            │
│                                                                     │
│  TxResult {                                                         │
│    success: true,                                                  │
│    gas_used: 50000,                                                │
│    returndata: [0x00, 0x00, ..., 0x01],                           │
│    logs: [                                                         │
│      { address: 0x1234..., topics: [...], data: [...] },          │
│      ...                                                           │
│    ],                                                              │
│    storage_writes: [                                               │
│      { address: 0x1234..., key: 0xABC..., value: 0xDEF... },     │
│      ...                                                           │
│    ],                                                              │
│  }                                                                  │
└────────────────────────────────────────────────────────────────────┘
```

---

## Summary

This document covered:

1. **ExecutionEnvironment abstraction**: Enables multiple VMs (EVM, future EraVM, Wasm)
2. **Trait lifecycle**: `new()`, `before_executing_frame()`, `start_executing_frame()`, `continue_after_preemption()`, `calculate_resources_passed_in_external_call()`
3. **Preemption model**: Cooperative multitasking via `ExecutionEnvironmentPreemptionPoint` (CallRequest/End)
4. **EVM Interpreter**: Complete implementation with 140+ opcodes, double gas accounting, stack/memory management
5. **Execution flow**: End-to-end transaction processing with preemption handling

**Key Takeaways**:
- EEs handle *how* to execute bytecode, bootloader handles *what* to execute
- Preemption enables clean separation of concerns and proper resource accounting
- EVM interpreter is fully Ethereum-compatible while tracking RISC-V costs
- New EEs can be added by implementing the `ExecutionEnvironment` trait

**Cross-references**:
- [KeyTypes.md](./KeyTypes.md): Detailed type definitions (Interpreter, System, Resources, etc.)
- [DataFlow.md](./DataFlow.md): Complete data flow through execution pipeline
- [SystemLayer.md](./SystemLayer.md): I/O subsystem, resource accounting, metadata
- [Storage.md](./Storage.md): Storage models and SLOAD/SSTORE implementation
- [API.md](./API.md): Entry points that initiate execution

**Next steps**: Read [DataFlow.md](./DataFlow.md) to understand how execution fits into the broader system.
