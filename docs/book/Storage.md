# Storage

## Introduction

Storage management in zkSync OS provides a sophisticated abstraction layer over state storage, enabling efficient access to contract storage slots and account data while maintaining compatibility with different storage models. The system is designed to support both flat storage (optimized for zkSync) and Ethereum-compatible Merkle-Patricia Trie structures.

**Key Capabilities**:
- Multiple storage models with unified interface
- Warm/cold access tracking for gas accounting (EIP-2929)
- Transient storage support (EIP-1153)
- Oracle-based external data access
- Snapshot and rollback mechanisms for revert handling
- Pubdata calculation for L1 submission
- Merkle proof generation for state commitments

**Storage Architecture**:
The storage system consists of several layers:
1. **IOSubsystem trait**: High-level interface for storage operations
2. **StorageModel trait**: Abstraction over different storage implementations
3. **Storage caches**: In-memory caches tracking warm slots and state changes
4. **Oracle integration**: External queries for cold storage access
5. **State management**: Snapshots, rollback, and finalization

**Why Multiple Models?**
zkSync OS supports multiple storage models to balance:
- **Performance**: Flat storage is optimized for RISC-V proving
- **Compatibility**: Ethereum MPT model maintains full Ethereum compatibility
- **Flexibility**: New models can be added for different use cases

---

## Table of Contents

1. [Storage Models](#storage-models)
2. [Storage Access Patterns](#storage-access-patterns)
3. [Storage Key Types](#storage-key-types)
4. [State Management](#state-management)
5. [Oracle Integration](#oracle-integration)
6. [Storage Output](#storage-output)

---

## Storage Models

zkSync OS supports multiple storage models through the `StorageModel` trait abstraction. Each model implements the same interface but uses different underlying data structures and commitment schemes.

### StorageModel Trait

**Location**: `storage_models/src/common_structs/traits/storage_model.rs:22-56`

**Purpose**: Defines the interface that all storage models must implement, enabling the system to switch between different storage backends without changing execution logic.

**Key Methods**:

```rust
pub trait StorageModel: Sized + SnapshottableIo {
    type IOTypes: SystemIOTypesConfig;
    type Resources: Resources;
    type StorageCommitment;

    fn storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &Address,
        key: &StorageKey,
        oracle: &mut impl IOOracle,
    ) -> Result<StorageValue, SystemError>;

    fn storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &Address,
        key: &StorageKey,
        new_value: &StorageValue,
        oracle: &mut impl IOOracle,
    ) -> Result<StorageKey, SystemError>;

    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &Address,
        key: &StorageKey,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError>;

    fn read_account_properties(...) -> Result<AccountData, SystemError>;

    // ... additional methods for balance, nonce, bytecode, etc.
}
```

All storage operations flow through this trait, enabling:
- Consistent gas charging across models
- Unified snapshot/rollback behavior
- Oracle integration for external data access
- Type-safe address and key types

---

### Flat Storage Model

**Location**: `basic_system/src/system_implementation/flat_storage_model/`

**Purpose**: The flat storage model is the primary storage implementation for zkSync OS. It uses a flat key-value mapping with Blake2s hashing to derive storage keys, optimized for RISC-V proving and efficient state commitments.

**Key Components**:

1. **Storage Cache** (`storage_cache.rs`):
   - In-memory cache tracking accessed storage slots
   - Backed by `HistoryMap` for snapshot/rollback support
   - Stores `WarmStorageValue` for each `WarmStorageKey`
   - Tracks warmness per transaction for gas accounting

2. **Flat Key Derivation** (`zk_ee/src/common_structs/warm_storage_key.rs:49-60`):
   ```rust
   pub fn derive_flat_storage_key(address: &B160, key: &Bytes32) -> Bytes32 {
       use crypto::blake2s::Blake2s256;
       let mut hasher = Blake2s256::new();
       let mut extended_address = Bytes32::ZERO;
       extended_address.as_u8_array_mut()[12..]
           .copy_from_slice(&address.to_be_bytes());
       hasher.update(extended_address.as_u8_array_ref());
       hasher.update(key.as_u8_array_ref());
       let hash = hasher.finalize();
       Bytes32::from_array(hash)
   }
   ```
   This converts `(address, key)` pairs into flat storage keys for the Merkle tree.

3. **Account Cache**:
   - Caches account properties (balance, nonce, bytecode hash)
   - Reduces oracle queries for frequently accessed accounts
   - Tracks changes for account diffs

4. **Preimage Cache**:
   - Stores bytecode and other preimages
   - Enables efficient bytecode retrieval
   - Tracks published preimages for L1 submission

**Structure**:
```rust
pub struct FlatTreeWithAccountsUnderHashesStorageModel<
    A: Allocator + Clone + Default,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
    SF: StackFactory<M>,
    const M: usize,
    const PROOF_ENV: bool,
> {
    // Storage slot cache
    storage: GenericPubdataAwarePlainStorage<WarmStorageKey, Bytes32, A, SF, M, R, P>,

    // Account data cache
    account_cache: AccountCache<...>,

    // Preimage storage
    preimage_cache: PreimageCache<...>,

    // Current transaction ID
    current_tx_id: TransactionId,
}
```

**Flat Storage Advantages**:
- **Performance**: Simple Blake2s hash, no trie traversal
- **Proof efficiency**: Fewer operations in RISC-V proving
- **Deterministic**: Flat key derivation is deterministic and reproducible
- **Compact**: Efficient merkle tree with 256 leaves per node

**When to Use**:
- Forward execution (sequencer)
- Proof generation (RISC-V)
- Default for all zkSync OS execution

**Merkle Tree Operations**:
The flat storage model maintains a merkle tree for state commitments:
- **Tree index queries**: Find position of key in sorted tree
- **Merkle proofs**: Generate proofs for state changes
- **Root calculation**: Compute state root hash

---

### Ethereum Storage Model (MPT)

**Location**: `basic_system/src/system_implementation/ethereum_storage_model/`

**Purpose**: Provides full Ethereum compatibility using Merkle-Patricia Trie (MPT) structure. This model replicates Ethereum's exact storage semantics including RLP encoding and trie node structure.

**Key Components**:

1. **MPT Structure**:
   - Branch nodes (16 children)
   - Extension nodes (compressed path)
   - Leaf nodes (key-value pairs)
   - RLP encoding for all nodes

2. **Node Parsing**:
   - Deserializes Ethereum trie nodes
   - Validates node structure
   - Computes node hashes

3. **Trie Updates**:
   - Insert/update operations maintain trie structure
   - Node splitting and merging
   - Path compression

**Ethereum Model Advantages**:
- **Full compatibility**: Exact Ethereum state structure
- **Interoperability**: Can import/export Ethereum state
- **Verification**: Compatible with Ethereum light clients

**When to Use**:
- Ethereum compatibility mode
- Migration from Ethereum
- Verification against Ethereum state

**Limitations**:
- More complex than flat storage
- Slower proof generation (more RISC-V operations)
- Larger state representation

---

### Storage Model Comparison

| Feature | Flat Storage Model | Ethereum MPT Model |
|---------|-------------------|-------------------|
| **Key derivation** | Blake2s hash | Keccak256 of RLP path |
| **Structure** | Flat key-value | Merkle-Patricia Trie |
| **Commitment** | Merkle tree (256 leaves/node) | MPT root hash |
| **Proof size** | Compact | Larger (full trie path) |
| **RISC-V performance** | Fast (simple hashing) | Slower (trie traversal) |
| **Ethereum compatibility** | zkSync semantics | Full Ethereum semantics |
| **State import** | zkSync snapshots | Ethereum state exports |
| **Recommended for** | zkSync OS default | Ethereum compatibility mode |

**Selecting a Model**:
The storage model is typically selected at system initialization:
```rust
// Flat storage (default)
let storage = FlatTreeWithAccountsUnderHashesStorageModel::new(...);

// Ethereum MPT (compatibility mode)
let storage = EthereumStorageModel::new(...);
```

Both models implement the `StorageModel` trait, so the rest of the system remains unchanged.

---

## Storage Access Patterns

Storage access in zkSync OS follows a sophisticated pattern that balances gas accounting, caching, and oracle queries. Understanding these patterns is crucial for optimizing contract execution.

### Read Path (SLOAD)

The storage read path handles both warm (cached) and cold (oracle-queried) accesses:

```
SLOAD opcode execution
    ↓
Interpreter calls: system.io.storage_read<false>(EVM, resources, address, key)
    ↓
IOSubsystem::storage_read() delegates to StorageModel::storage_read()
    ↓
Create WarmStorageKey { address, key }
    ↓
Look up in storage cache
    ↓
┌─────────────────────┬─────────────────────┐
│   Cache Hit (warm)  │  Cache Miss (cold)  │
└─────────────────────┴─────────────────────┘
         ↓                        ↓
Charge 100 gas         Query oracle for initial value
(WARM_SLOAD_COST)              ↓
         ↓             Charge 2100 gas (COLD_SLOAD_COST)
Get cached                     ↓
WarmStorageValue       Charge native cost (RISC-V cycles)
         ↓                     ↓
Return current_value   Create WarmStorageValue {
         ↓                 initial_value: oracle_result,
         ↓                 current_value: oracle_result,
         ↓                 value_at_the_start_of_tx: oracle_result,
         ↓                 last_accessed_at_tx_number: Some(current_tx),
         ↓                 changes_stack_depth: 0,
         ↓                 pubdata_diff_bytes: 0,
         ↓                 initial_value_used: true,
         ↓                 is_new_storage_slot: value == 0,
         ↓             }
         ↓                     ↓
         ↓             Insert into cache
         ↓                     ↓
         ↓             Return oracle_result
         └─────────────────────┘
                    ↓
         Return value to interpreter
                    ↓
         Push value to stack
```

**Warm vs. Cold Access** (EIP-2929):
- **Warm**: Slot accessed earlier in this transaction (100 gas)
- **Cold**: First access in this transaction (2100 gas = 2000 cold access + 100 warm)
- Warmness is per-transaction, reset at transaction start
- Access lists (EIP-2930) can pre-warm slots

**Storage Access Policy** (`storage_cache.rs:44-77`):
```rust
pub trait StorageAccessPolicy<R: Resources, V>: 'static + Sized {
    /// Charge for a warm read (already in cache)
    fn charge_warm_storage_read(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        is_access_list: bool,
    ) -> Result<(), SystemError>;

    /// Charge the extra cost of reading a key not present in the cache
    fn charge_cold_storage_read_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        is_new_slot: bool,
    ) -> Result<(), SystemError>;

    /// Charge the additional cost of performing a write
    fn charge_storage_write_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        initial_value: &V,
        current_value: &V,
        new_value: &V,
        resources: &mut R,
        is_warm_write: bool,
        is_new_slot: bool,
    ) -> Result<(), SystemError>;
}
```

This policy abstracts gas charging logic, enabling different policies for different execution environments.

---

### Write Path (SSTORE)

The storage write path is more complex due to gas refunds and pubdata calculation:

```
SSTORE opcode execution
    ↓
Interpreter calls: system.io.storage_write<false>(EVM, resources, address, key, value)
    ↓
IOSubsystem::storage_write() delegates to StorageModel::storage_write()
    ↓
Create WarmStorageKey { address, key }
    ↓
Look up in storage cache (perform read if cold)
    ↓
Get WarmStorageValue {
    initial_value,           // Value at start of block
    current_value,           // Current value before this write
    value_at_the_start_of_tx // Value at start of current tx
}
    ↓
Calculate SSTORE gas cost (EIP-2200/EIP-1283):
    ↓
    If slot is cold:
        cost += COLD_SLOAD_COST (2100 gas)
        Mark as warm
    ↓
    If new_value == value_at_the_start_of_tx:
        // Restoring original value
        cost += WARM_SLOAD_COST (100 gas)
        If value_at_the_start_of_tx != 0:
            refund += SSTORE_RESET_REFUND (2900 gas)
    Else:
        If current_value != new_value:
            If value_at_the_start_of_tx != current_value:
                // Modifying already modified slot
                cost += WARM_SLOAD_COST (100 gas)
            Else:
                // First modification this tx
                If value_at_the_start_of_tx == 0:
                    cost += SSTORE_SET_GAS (20000 gas)  // New slot
                Else:
                    cost += SSTORE_RESET_GAS (2900 gas) // Modify existing

                If new_value == 0:
                    refund += SSTORE_CLEARS_SCHEDULE (15000 gas)
    ↓
Charge gas: resources.charge(cost)?
    ↓
Calculate pubdata diff:
    pubdata_diff_bytes = calculate_pubdata_diff(initial_value, new_value)
    ↓
    If initial_value == 0 && new_value != 0:
        32 bytes (new slot created)
    Else if initial_value != 0 && new_value == 0:
        0 bytes (slot cleared, already had pubdata)
    Else if initial_value != new_value:
        32 bytes (slot modified)
    Else:
        0 bytes (no change from initial)
    ↓
Update WarmStorageValue {
    current_value: new_value,
    changes_stack_depth: current_frame_depth,
    pubdata_diff_bytes: calculated_diff,
    last_accessed_at_tx_number: Some(current_tx),
}
    ↓
Record change in journal for potential rollback
    ↓
Return old value to interpreter
```

**SSTORE Gas Costs**:
- **COLD_SLOAD_COST**: 2100 gas (first access)
- **WARM_SLOAD_COST**: 100 gas (warm access)
- **SSTORE_SET_GAS**: 20000 gas (new slot, was zero)
- **SSTORE_RESET_GAS**: 2900 gas (modify existing slot)
- **SSTORE_CLEARS_SCHEDULE**: 15000 gas refund (clear slot to zero)
- **SSTORE_RESET_REFUND**: 2900 gas refund (restore original value)

**Gas Refunds**:
Refunds are accumulated during transaction execution but capped at:
```
max_refund = gas_used / 5  // EIP-3529 (London)
```
Refunds reduce the total gas charged for the transaction.

---

### Transient Storage (EIP-1153)

Transient storage provides transaction-scoped storage that is cleared at the end of each transaction. It's cheaper than persistent storage and useful for reentrancy locks and temporary data.

**Location**: `storage_models/src/common_structs/generic_transient_storage.rs`

**Structure**:
```rust
pub struct GenericTransientStorage<
    K: KeyLikeWithBounds,
    V: Clone,
    SF: StackFactory<M>,
    const M: usize,
    A: Allocator + Clone = Global,
> {
    cache: HistoryMap<K, V, A>,
    current_tx_number: u32,
    phantom: PhantomData<SF>,
    alloc: A,
}
```

**Key Features**:
1. **Transaction-scoped**: Cleared at end of each transaction
2. **Always warm**: No cold access penalty (100 gas per TLOAD/TSTORE)
3. **Rollback support**: Changes revert on frame failure
4. **No pubdata**: Not published to L1 (never leaves L2)

**Operations**:
```rust
// TLOAD - read transient storage (100 gas)
system.io.storage_read<true>(EVM, resources, address, key)?

// TSTORE - write transient storage (100 gas)
system.io.storage_write<true>(EVM, resources, address, key, value)?
```

**Usage Pattern**:
```solidity
// Reentrancy lock using transient storage
contract ReentrancyGuard {
    uint256 constant LOCK_SLOT = 0;

    modifier nonReentrant() {
        require(tload(LOCK_SLOT) == 0, "Reentrant call");
        tstore(LOCK_SLOT, 1);
        _;
        tstore(LOCK_SLOT, 0);
    }

    function withdraw() external nonReentrant {
        // Safe from reentrancy
        ...
    }
}
```

**Transient Storage Lifecycle**:
```
Transaction starts
    ↓
transient_storage = empty HistoryMap
    ↓
Execution:
    TSTORE slot1 = 0xAAA
    TSTORE slot2 = 0xBBB
    TLOAD slot1 → 0xAAA
    ↓
    External call (frame depth increases):
        TSTORE slot1 = 0xCCC  // Override
        TLOAD slot1 → 0xCCC
        ↓
        REVERT (rollback to frame snapshot)
        ↓
    TLOAD slot1 → 0xAAA  // Restored
    ↓
Transaction ends
    ↓
transient_storage.begin_new_tx()  // Clear all data
    ↓
Next transaction: transient_storage = empty
```

**Cost Comparison**:
| Operation | Persistent Storage (SSTORE) | Transient Storage (TSTORE) |
|-----------|----------------------------|---------------------------|
| Cold write | 20000-22100 gas | 100 gas (always warm) |
| Warm write | 100-2900 gas | 100 gas |
| Cold read | 2100 gas | 100 gas (always warm) |
| Warm read | 100 gas | 100 gas |
| Pubdata | ~8-32 bytes per slot | 0 bytes |
| Persistence | Permanent | Transaction-scoped |

---

### Journaling and Rollback

All storage operations are journaled to enable rollback on REVERT or failed external calls:

```
Frame starts
    ↓
snapshot_id = storage.start_frame()
    ↓
Store snapshot of:
    - storage cache state
    - transient storage state
    - account cache state
    - logs and events counters
    ↓
Execution:
    SSTORE slot1 = 0xAAA
    SSTORE slot2 = 0xBBB
    LOG event
    ↓
    External call (nested frame):
        SSTORE slot1 = 0xCCC
        SSTORE slot3 = 0xDDD
        ↓
        REVERT (call failed)
        ↓
    storage.finish_frame(Some(snapshot_id))  // Rollback nested frame
        ↓
        Restore slot1 = 0xAAA (undo override)
        Delete slot3 (never existed in parent)
        Remove logs/events from nested frame
    ↓
Frame continues:
    SLOAD slot1 → 0xAAA  // Restored value
    SLOAD slot3 → 0 (default, was deleted)
    ↓
Frame completes successfully
    ↓
storage.finish_frame(None)  // Commit changes to parent frame
```

**Snapshot Mechanism**:
Each storage operation records its changes with the current `changes_stack_depth`:
```rust
pub struct WarmStorageValue {
    // ... other fields
    pub changes_stack_depth: usize,  // Frame depth of last change
}
```

On rollback:
1. Iterate all cached slots
2. If `changes_stack_depth >= rollback_frame_depth`:
   - Restore value from history
   - Update `changes_stack_depth` to parent frame
3. Remove any slots that were first created in the reverted frame

**History Map**:
The underlying `HistoryMap` structure maintains a linked list of value changes:
```
slot1:
    current_value: 0xCCC (frame depth 2)
    ↓
    previous_value: 0xAAA (frame depth 1)
    ↓
    initial_value: 0x000 (frame depth 0)
```

---

## Storage Key Types

Storage keys and values are tracked using specialized types that encode warmness, lifecycle, and pubdata information. These types are critical for gas accounting and state diff generation.

### WarmStorageKey

**Location**: `zk_ee/src/common_structs/warm_storage_key.rs:5-8`

**Purpose**: Uniquely identifies a storage location by contract address and storage key.

**Definition**:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WarmStorageKey {
    pub address: B160,  // 160-bit Ethereum address
    pub key: Bytes32,   // 256-bit storage key
}
```

**Properties**:
- **Ord/PartialOrd**: Implements ordering (address first, then key) for BTree usage
- **Hash**: Can be used as hash map key
- **Default**: Creates zero address and zero key

**Usage**: See [KeyTypes.md](./KeyTypes.md) for detailed information about `WarmStorageKey`.

---

### WarmStorageValue

**Location**: `zk_ee/src/common_structs/warm_storage_value.rs:10-20`

**Purpose**: Tracks the complete lifecycle of a storage slot including initial value, current value, warmness, and pubdata implications.

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
    pub is_new_storage_slot: bool,               // Is this a new slot?
}
```

**Field Purposes**: See [KeyTypes.md](./KeyTypes.md) for detailed breakdown of each field.

**Warmth Tracking**:
```rust
impl StorageElementMetadata {
    pub fn considered_warm(&self, current_tx_id: TransactionId) -> bool {
        self.last_accessed_at_tx_number == Some(current_tx_id)
    }
}
```

A slot is warm if `last_accessed_at_tx_number` equals the current transaction number.

---

### Pubdata Calculation

Pubdata represents state changes that must be published to L1. Storage operations contribute to pubdata based on the change from initial value:

**Calculation Logic**:
```rust
fn calculate_pubdata_diff(initial: Bytes32, new: Bytes32) -> u8 {
    if initial == Bytes32::ZERO && new != Bytes32::ZERO {
        32  // New slot created (publish full value)
    } else if initial != Bytes32::ZERO && new == Bytes32::ZERO {
        0   // Slot cleared (initial already on L1, no new pubdata)
    } else if initial != new {
        32  // Slot modified (publish new value)
    } else {
        0   // No change from initial (no pubdata)
    }
}
```

**Key Points**:
- Only changes from **initial_value** (start of block) contribute to pubdata
- Intermediate changes within the block don't add pubdata
- Clearing a slot (to zero) adds no pubdata (the fact it was non-zero is already on L1)
- Each changed slot contributes at most 32 bytes

**Pubdata Cost**:
Pubdata has its own gas cost:
```rust
let pubdata_cost = pubdata_bytes * pubdata_price_per_byte;
resources.charge(pubdata_cost)?;
```

This cost is in addition to storage operation costs and ensures L1 calldata costs are covered.

---

## State Management

State management in zkSync OS handles snapshots, rollback, finalization, and proof generation. These mechanisms ensure atomicity of transactions and enable revert handling.

### Snapshots

Snapshots capture the current state before executing a frame, enabling rollback if the frame fails.

**Snapshot Creation**:
```rust
// Before executing frame
let snapshot = system.io.start_io_frame()?;

// Execute frame
let result = execute_frame(...);

// Commit or rollback
match result {
    Ok(_) => system.io.finish_io_frame(None)?,        // Commit
    Err(_) => system.io.finish_io_frame(Some(&snapshot))?, // Rollback
}
```

**Snapshot Contents**:
```rust
pub struct StateSnapshot {
    pub cache: CacheSnapshotId,               // Storage cache snapshot
    pub evm_refunds_counter: HistoryCounterSnapshotId, // Gas refund counter
}
```

The snapshot records:
1. **Storage cache state**: Current values of all accessed slots
2. **Transient storage state**: Transient slots in current tx
3. **Account cache state**: Balance, nonce, bytecode hash changes
4. **Logs/events counters**: Number of logs and events emitted
5. **Refund counter**: Gas refunds accumulated

**HistoryMap Snapshots**:
The underlying `HistoryMap` maintains a version counter:
```rust
struct HistoryMap<K, V, A> {
    map: BTreeMap<K, VersionedValue<V>, A>,
    version_counter: u64,
}

struct VersionedValue<V> {
    values: Vec<(u64, V)>,  // List of (version, value) pairs
}
```

Creating a snapshot records the current `version_counter`. Rollback restores all values to their state at that version.

---

### Rollback

Rollback undoes all state changes made since a snapshot was created.

**Rollback Process**:
```rust
pub fn finish_io_frame(&mut self, rollback_handle: Option<&StateSnapshot>) -> Result<(), InternalError> {
    if let Some(snapshot) = rollback_handle {
        // Rollback storage cache
        self.storage.rollback_to_snapshot(&snapshot.cache)?;

        // Rollback transient storage
        self.transient_storage.rollback_to_snapshot(&snapshot.cache)?;

        // Rollback account cache
        self.account_cache.rollback_to_snapshot(&snapshot.cache)?;

        // Rollback logs
        self.logs_storage.finish_frame(Some(snapshot.logs_snapshot))?;

        // Rollback events
        self.events_storage.finish_frame(Some(snapshot.events_snapshot))?;

        // Rollback refund counter
        self.evm_refunds_counter.rollback_to_snapshot(&snapshot.evm_refunds_counter)?;
    } else {
        // Commit: just increment version counter, keep changes
        self.storage.commit_frame();
    }
    Ok(())
}
```

**HistoryMap Rollback**:
For each key in the map:
1. Find all value versions > snapshot version
2. Remove those versions
3. Current value becomes the highest version ≤ snapshot version

**Example**:
```
Before rollback (snapshot at version 10):
slot1:
    version 0: 0x000
    version 10: 0xAAA
    version 15: 0xBBB
    version 20: 0xCCC (current)

After rollback to version 10:
slot1:
    version 0: 0x000
    version 10: 0xAAA (current)
```

---

### Finalization

Finalization occurs at the end of each transaction, committing changes and preparing for the next transaction.

**Transaction Finalization**:
```rust
pub fn finish_tx(&mut self) -> Result<(), InternalError> {
    // Apply SELFDESTRUCT (mark_for_deconstruction)
    for address in self.deconstructed_accounts {
        self.clear_account(address)?;
        self.transfer_balance(address, beneficiary)?;
    }

    // Clear transient storage for next tx
    self.transient_storage.begin_new_tx();

    // Increment transaction counter
    self.tx_number += 1;

    // Reset warmness tracking
    self.current_tx_id = TransactionId(self.tx_number);

    Ok(())
}
```

**Block Finalization**:
At the end of the block, all data is aggregated and returned:
```rust
pub fn finish(self) -> Result<BlockOutputData, InternalError> {
    // Compute storage diffs
    let storage_writes = self.storage.compute_diffs();

    // Compute account diffs
    let account_diffs = self.account_cache.compute_diffs();

    // Collect logs
    let logs = self.logs_storage.collect_logs();

    // Collect events
    let events = self.events_storage.collect_events();

    // Published preimages
    let preimages = self.preimage_cache.collect_preimages();

    // Calculate logs merkle root
    let logs_root = self.logs_storage.tree_root();

    Ok(BlockOutputData {
        storage_writes,
        account_diffs,
        logs,
        events,
        preimages,
        logs_root,
    })
}
```

---

### Merkle Proofs

Merkle proofs enable verification of state changes without requiring the full state.

**Proof Generation**:
The flat storage model maintains a merkle tree over all storage keys:
```
Tree structure (example with 4 leaves):
           root
          /    \
     node1      node2
    /    \     /    \
  key1  key2 key3  key4
```

Each leaf contains `hash(key, value)` where key is the flat storage key.

**Proof Queries** (handled by `ReadTreeResponder`):

1. **Tree Index Query** (`PROOF_FOR_INDEX_QUERY_ID`):
   - Given a flat storage key, return its position in the sorted tree
   - Used to locate the leaf for proof generation

2. **Merkle Proof Query**:
   - Given a tree index, return the merkle proof (sibling hashes)
   - Proof enables verification: `root = hash(hash(...hash(leaf, sibling1), sibling2...), siblingN)`

3. **Previous Index Query** (`PreviousIndexQuery`):
   - Given a key, return the tree index of the previous key (or None)
   - Used for non-membership proofs

**Proof Verification** (on L1):
```solidity
function verifyStorageProof(
    bytes32 root,
    bytes32 key,
    bytes32 value,
    bytes32[] memory proof
) public pure returns (bool) {
    bytes32 leaf = keccak256(abi.encode(key, value));
    bytes32 computed = leaf;
    for (uint i = 0; i < proof.length; i++) {
        computed = keccak256(abi.encode(computed, proof[i]));
    }
    return computed == root;
}
```

---

### BlockOutput Format

The final output of block execution includes all state changes and metadata:

**Structure**:
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

    /// Pubdata for L1 submission
    pub pubdata: Vec<u8>,

    /// Native resources consumed
    pub computational_native_used: u64,
}
```

**StorageWrite Format**:
```rust
pub struct StorageWrite {
    pub address: B160,
    pub key: Bytes32,
    pub initial_value: Bytes32,  // Value at start of block
    pub final_value: Bytes32,    // Value at end of block
}
```

**AccountDiff Format**:
```rust
pub struct AccountDiff {
    pub address: B160,
    pub nonce_before: u64,
    pub nonce_after: u64,
    pub balance_before: U256,
    pub balance_after: U256,
    pub bytecode_hash_before: Option<Bytes32>,
    pub bytecode_hash_after: Option<Bytes32>,
}
```

See [DataFlow.md](./DataFlow.md) for complete details on block output generation.

---

## Oracle Integration

Oracle integration provides the bridge between execution and external data sources. The oracle pattern enables deterministic execution in both forward running (native) and proof generation (RISC-V) modes.

### Oracle Architecture

**Oracle Trait** (`zk_ee/src/oracle/mod.rs`):
```rust
pub trait IOOracle {
    fn query<Q: SimpleOracleQuery>(
        &mut self,
        query: Q::Input,
    ) -> Result<Q::Output, OracleError>;
}
```

The oracle abstracts all external data access:
- Storage reads
- Account data (balance, nonce, bytecode)
- Block metadata
- Transaction data

**Oracle Implementations**:
1. **Forward Mode**: Oracle backed by database or in-memory state
2. **Proof Mode**: Oracle backed by CSR (Control & Status Register) communication with host
3. **Testing**: Mock oracle with predefined responses

---

### ReadTreeResponder

**Location**: `forward_system/src/run/query_processors/read_tree.rs`

**Purpose**: Handles queries related to the storage tree structure, including storage reads, tree index lookups, and Merkle proof generation.

**Supported Queries**:
```rust
const SUPPORTED_QUERY_IDS: &[u32] = &[
    InitialStorageSlotQuery::QUERY_ID,  // Read storage slot value
    PreviousIndexQuery::QUERY_ID,       // Get previous tree index
    ExactIndexQuery::QUERY_ID,          // Get exact tree index
    PROOF_FOR_INDEX_QUERY_ID,           // Generate Merkle proof
];
```

**Query Processing**:
1. **InitialStorageSlotQuery**:
   - Input: `StorageAddress { address, key }`
   - Output: `InitialStorageSlotData { initial_value, is_new_storage_slot }`
   - Process:
     ```rust
     let flat_key = derive_flat_storage_key(&address, &key);
     let value = tree.read(flat_key);
     let is_new = value.is_none();
     InitialStorageSlotData {
         initial_value: value.unwrap_or(Bytes32::ZERO),
         is_new_storage_slot: is_new,
     }
     ```

2. **PreviousIndexQuery**:
   - Input: `flat_key: Bytes32`
   - Output: `Option<u64>` (tree index)
   - Process: Binary search in sorted tree to find previous key

3. **ExactIndexQuery**:
   - Input: `flat_key: Bytes32`
   - Output: `u64` (tree index, panics if not found)
   - Process: Lookup exact position in tree

4. **Merkle Proof Query**:
   - Input: `tree_index: u64`
   - Output: `Vec<Bytes32>` (sibling hashes for merkle path)
   - Process: Traverse tree from leaf to root, collecting siblings

**Tree Interface** (`ReadStorageTree` trait):
```rust
pub trait ReadStorageTree {
    fn read(&self, key: Bytes32) -> Option<Bytes32>;
    fn tree_index(&self, key: Bytes32) -> Option<u64>;
    fn prev_tree_index(&self, key: Bytes32) -> Option<u64>;
    fn merkle_proof(&self, index: u64) -> Vec<Bytes32>;
}
```

---

### ReadStorageResponder

**Location**: `forward_system/src/run/query_processors/read_storage.rs`

**Purpose**: Simpler oracle responder that only handles storage reads without tree operations. Useful for simulations (eth_call, eth_estimateGas) where Merkle proofs aren't needed.

**Supported Queries**:
```rust
const SUPPORTED_QUERY_IDS: &[u32] = &[
    InitialStorageSlotQuery::QUERY_ID,
];
```

**Query Processing**:
```rust
fn process_buffered_query(&mut self, query_id: u32, query: Vec<usize>)
    -> Box<dyn ExactSizeIterator<Item = usize>>
{
    let StorageAddress { address, key } = deserialize_query(query);
    let flat_key = derive_flat_storage_key(&address, &key);

    let slot_data = if let Some(value) = self.storage.read(flat_key) {
        InitialStorageSlotData {
            initial_value: value,
            is_new_storage_slot: false,
        }
    } else {
        InitialStorageSlotData {
            initial_value: Bytes32::ZERO,
            is_new_storage_slot: true,
        }
    };

    Box::new(slot_data.iter())  // Serialize response
}
```

**Storage Interface** (`ReadStorage` trait):
```rust
pub trait ReadStorage {
    fn read(&self, key: Bytes32) -> Option<Bytes32>;
}
```

**When to Use**:
- **ReadTreeResponder**: Full block execution, proof generation (needs Merkle proofs)
- **ReadStorageResponder**: Simulations, estimations (no proofs needed)

---

### Query Protocol

The query protocol defines how execution requests data from the oracle:

**Query Structure**:
```rust
pub struct OracleQuery {
    pub query_id: u32,      // Identifies query type
    pub params: Vec<usize>, // Serialized parameters
}

pub struct OracleResponse {
    pub data: Vec<usize>,   // Serialized response
}
```

**Query Flow**:
```
1. Execution needs storage value:
   SLOAD address:0x123, key:0xABC
       ↓
2. Storage model checks cache:
   Not in cache → cold access
       ↓
3. Create oracle query:
   query_id = InitialStorageSlotQuery::QUERY_ID (0x01)
   params = serialize(StorageAddress { address: 0x123, key: 0xABC })
       ↓
4. Send query to oracle:
   oracle.query(query_id, params)
       ↓
5. Oracle processes:
   Forward mode:  Query database/tree
   Proof mode:    Write to CSR, read response from CSR
   Test mode:     Return mock data
       ↓
6. Oracle returns response:
   data = serialize(InitialStorageSlotData { initial_value: 0xDEF, is_new: false })
       ↓
7. Storage model deserializes:
   slot_data = deserialize(data)
       ↓
8. Create WarmStorageValue and cache:
   cache.insert(WarmStorageKey { address: 0x123, key: 0xABC },
                WarmStorageValue { initial_value: 0xDEF, current_value: 0xDEF, ... })
       ↓
9. Return value to execution:
   SLOAD returns 0xDEF
```

**Serialization**:
All oracle data is serialized as `Vec<usize>` for efficient RISC-V communication:
```rust
pub trait UsizeSerializable {
    fn iter(&self) -> impl ExactSizeIterator<Item = usize>;
}

pub trait UsizeDeserializable: Sized {
    fn from_iter(iter: &mut impl Iterator<Item = usize>) -> Result<Self, DeserializationError>;
}
```

See [DataFlow.md](./DataFlow.md) for complete oracle communication flow.

---

## Storage Output

Storage output represents the final state changes from block execution, formatted for L1 submission and state verification.

### StorageWrite Structure

**Definition**:
```rust
pub struct StorageWrite {
    pub address: B160,           // Contract address
    pub key: Bytes32,            // Storage key
    pub initial_value: Bytes32,  // Value at start of block
    pub final_value: Bytes32,    // Value at end of block
}
```

**Generation**:
At block finalization, iterate all cached storage slots:
```rust
fn compute_storage_writes(&self) -> Vec<StorageWrite> {
    let mut writes = Vec::new();

    for (warm_key, warm_value) in self.storage_cache.iter() {
        // Only include slots that changed
        if warm_value.initial_value != warm_value.current_value {
            writes.push(StorageWrite {
                address: warm_key.address,
                key: warm_key.key,
                initial_value: warm_value.initial_value,
                final_value: warm_value.current_value,
            });
        }
    }

    writes.sort();  // Deterministic ordering
    writes
}
```

**Properties**:
- Only changed slots are included (initial_value ≠ final_value)
- Sorted by (address, key) for deterministic ordering
- No temporary changes within the block (only initial → final)

---

### Account Diffs

**Definition**:
```rust
pub struct AccountDiff {
    pub address: B160,
    pub nonce_before: u64,
    pub nonce_after: u64,
    pub balance_before: U256,
    pub balance_after: U256,
    pub bytecode_hash_before: Option<Bytes32>,
    pub bytecode_hash_after: Option<Bytes32>,
}
```

**Generation**:
```rust
fn compute_account_diffs(&self) -> Vec<AccountDiff> {
    let mut diffs = Vec::new();

    for (address, account_data) in self.account_cache.iter() {
        // Only include accounts that changed
        if account_data.has_changes() {
            diffs.push(AccountDiff {
                address,
                nonce_before: account_data.initial_nonce,
                nonce_after: account_data.current_nonce,
                balance_before: account_data.initial_balance,
                balance_after: account_data.current_balance,
                bytecode_hash_before: account_data.initial_bytecode_hash,
                bytecode_hash_after: account_data.current_bytecode_hash,
            });
        }
    }

    diffs.sort_by_key(|d| d.address);
    diffs
}
```

**Changes Tracked**:
- **Nonce**: Incremented by transactions and contract creation
- **Balance**: Modified by value transfers, SELFDESTRUCT, block rewards
- **Bytecode hash**: Set by CREATE/CREATE2, cleared by SELFDESTRUCT

---

### Preimage Publication

Preimages are data that must be published to L1 for state verification:

**Types of Preimages**:
1. **Bytecode**: Contract bytecode from CREATE/CREATE2
2. **Account data**: Initial account properties
3. **Storage data**: Initial storage values (for ZK proofs)

**Format**:
```rust
pub type PreimagePublication = Vec<(Hash, Vec<u8>)>;
// Each entry: (hash_of_preimage, preimage_bytes)
```

**Bytecode Publication**:
```rust
fn publish_bytecode(&mut self, bytecode: &[u8]) -> Bytes32 {
    let hash = keccak256(bytecode);
    self.published_preimages.push((hash, bytecode.to_vec()));
    hash
}
```

When a contract is created:
1. Bytecode is deployed to storage
2. Bytecode hash is stored in account
3. Full bytecode is added to published_preimages
4. On L1, bytecode can be verified: `keccak256(bytecode) == stored_hash`

**Why Publish?**
- Enables L1 to reconstruct state
- Allows verification of ZK proofs
- Supports L2 → L1 data availability
- Required for fraud proofs (if applicable)

---

## Storage Access Flow Diagrams

### Diagram 1: Storage Read Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    SLOAD Opcode Execution                        │
│  Interpreter: stack.pop() → storage_key                          │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│           system.io.storage_read<false>(...)                     │
│  Parameters:                                                     │
│    - ee_type: EVM                                                │
│    - resources: &mut gas                                         │
│    - address: interpreter.address                                │
│    - key: storage_key                                            │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│              IOSubsystem → StorageModel                          │
│  Delegate to storage model implementation                        │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│       Create WarmStorageKey { address, key }                     │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│              Check storage_cache[warm_key]                       │
└─────────────────────────────────────────────────────────────────┘
          ↓                                          ↓
┌────────────────────┐                    ┌─────────────────────┐
│   CACHE HIT (warm) │                    │  CACHE MISS (cold)  │
└────────────────────┘                    └─────────────────────┘
          ↓                                          ↓
┌────────────────────┐                    ┌─────────────────────┐
│Charge 100 gas      │                    │ Query oracle:       │
│(WARM_SLOAD_COST)   │                    │                     │
└────────────────────┘                    │ query_id =          │
          ↓                                │  InitialStorage-    │
┌────────────────────┐                    │  SlotQuery          │
│Get cached          │                    │                     │
│WarmStorageValue    │                    │ params = { addr,    │
│                    │                    │           key }     │
└────────────────────┘                    └─────────────────────┘
          ↓                                          ↓
┌────────────────────┐                    ┌─────────────────────┐
│warm_value.         │                    │Oracle returns:      │
│  last_accessed =   │                    │  { initial_value,   │
│  Some(current_tx)  │                    │    is_new_slot }    │
│                    │                    │                     │
└────────────────────┘                    └─────────────────────┘
          ↓                                          ↓
┌────────────────────┐                    ┌─────────────────────┐
│Return              │                    │Charge 2100 gas      │
│warm_value.         │                    │(COLD_SLOAD_COST)    │
│  current_value     │                    │                     │
└────────────────────┘                    └─────────────────────┘
          ↓                                          ↓
          │                                ┌─────────────────────┐
          │                                │Charge native cost   │
          │                                │(RISC-V cycles)      │
          │                                └─────────────────────┘
          │                                          ↓
          │                                ┌─────────────────────┐
          │                                │Create               │
          │                                │WarmStorageValue:    │
          │                                │  initial_value: X   │
          │                                │  current_value: X   │
          │                                │  value_at_start: X  │
          │                                │  last_accessed:     │
          │                                │    Some(current_tx) │
          │                                │  is_new: true/false │
          │                                └─────────────────────┘
          │                                          ↓
          │                                ┌─────────────────────┐
          │                                │Insert into cache:   │
          │                                │cache[warm_key] =    │
          │                                │  warm_value         │
          │                                └─────────────────────┘
          │                                          ↓
          │                                ┌─────────────────────┐
          │                                │Return initial_value │
          │                                └─────────────────────┘
          │                                          ↓
          └──────────────────┬───────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                  Return value to Interpreter                     │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│             stack.push(value)  // Push to EVM stack              │
└─────────────────────────────────────────────────────────────────┘
```

---

### Diagram 2: Write and Rollback Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                 Frame Start (External Call)                      │
│  snapshot = system.io.start_io_frame()                           │
│    - Record current cache state                                  │
│    - Record version counter                                      │
│    - Record logs/events count                                    │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                  SSTORE address:A, key:K, value:V                │
│  1. Ensure slot is warm (SLOAD if needed)                        │
│  2. Get current WarmStorageValue                                 │
│  3. Calculate SSTORE gas cost (EIP-2200)                         │
│  4. Charge gas                                                   │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│              Update WarmStorageValue in Cache                    │
│                                                                  │
│  warm_value.current_value = V                                   │
│  warm_value.changes_stack_depth = current_frame_depth           │
│  warm_value.pubdata_diff_bytes =                                │
│      calculate_pubdata(initial_value, V)                        │
│                                                                  │
│  Record in history:                                              │
│    version: current_version_counter                              │
│    previous_value: old_current_value                             │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│              Additional Operations in Frame                      │
│  SSTORE address:A, key:K2, value:V2                              │
│  LOG2 ...                                                        │
│  CALL to another contract (nested frame)                         │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                     Frame Completes                              │
└─────────────────────────────────────────────────────────────────┘
          ↓                                          ↓
┌────────────────────┐                    ┌─────────────────────┐
│  SUCCESS           │                    │  REVERT/FAILURE     │
└────────────────────┘                    └─────────────────────┘
          ↓                                          ↓
┌────────────────────┐                    ┌─────────────────────┐
│system.io.          │                    │system.io.           │
│  finish_io_frame(  │                    │  finish_io_frame(   │
│    None            │                    │    Some(&snapshot)  │
│  )                 │                    │  )                  │
│                    │                    │                     │
│Commit changes:     │                    │Rollback:            │
│- Keep all writes   │                    │                     │
│- Keep logs/events  │                    │For each cached slot:│
│- Increment version │                    │  If changes_stack_  │
│  counter           │                    │    depth >=         │
└────────────────────┘                    │    snapshot_depth:  │
          ↓                                │                     │
┌────────────────────┐                    │  Restore value from │
│Changes visible     │                    │    history at       │
│to parent frame     │                    │    snapshot version │
└────────────────────┘                    │                     │
                                          │Remove logs/events   │
                                          │  after snapshot     │
                                          │                     │
                                          │Restore version      │
                                          │  counter to         │
                                          │  snapshot version   │
                                          └─────────────────────┘
                                                     ↓
                                          ┌─────────────────────┐
                                          │Changes undone,      │
                                          │state restored to    │
                                          │snapshot point       │
                                          └─────────────────────┘
```

---

### Diagram 3: Flat Storage Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                  Flat Storage Architecture                       │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  Input: (address, key) pair from SLOAD/SSTORE                   │
│    address: 0x1234...  (20 bytes)                                │
│    key:     0xABCD...  (32 bytes)                                │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│          Flat Key Derivation (Blake2s)                           │
│                                                                  │
│  flat_key = Blake2s(                                             │
│      0x000000000000000000000000 ++ address ++ key                │
│  )                                                               │
│                                                                  │
│  Result: 32-byte flat storage key                                │
│    flat_key: 0x7F3A...  (32 bytes)                               │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│              Merkle Tree Leaf Position                           │
│                                                                  │
│  Flat keys are sorted: K₁ < K₂ < K₃ < ... < Kₙ                  │
│                                                                  │
│  Binary search finds position for flat_key                       │
│    - If found: tree_index = position                             │
│    - If not found: tree_index = insertion position               │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                    Merkle Tree Structure                         │
│                   (example with 8 leaves)                        │
│                                                                  │
│                          Root                                    │
│                         /    \                                   │
│                    N₁           N₂                               │
│                   /  \         /  \                              │
│                N₃    N₄     N₅    N₆                             │
│               / \    / \    / \    / \                           │
│             L₁  L₂  L₃ L₄  L₅ L₆  L₇ L₈                          │
│                                                                  │
│  Each leaf Lᵢ = Hash(flat_keyᵢ, valueᵢ)                         │
│  Each node Nⱼ = Hash(left_child, right_child)                   │
│  Root = final hash at top                                        │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                  Storage Cache Structure                         │
│                                                                  │
│  BTreeMap<WarmStorageKey, WarmStorageValue>                     │
│                                                                  │
│  Entry 1:                                                        │
│    key: { address: 0x1234, key: 0xABCD }                         │
│    value: {                                                      │
│      initial_value: 0x0000...                                    │
│      current_value: 0x5555...                                    │
│      value_at_start_of_tx: 0x0000...                             │
│      changes_stack_depth: 2                                      │
│      last_accessed_at_tx: Some(5)                                │
│      pubdata_diff_bytes: 32                                      │
│      is_new_storage_slot: true                                   │
│    }                                                             │
│                                                                  │
│  Entry 2:                                                        │
│    key: { address: 0x5678, key: 0xDEF0 }                         │
│    value: { ... }                                                │
│                                                                  │
│  ... (cached entries for warm slots)                             │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│              Oracle Query for Cold Access                        │
│                                                                  │
│  If cache miss:                                                  │
│    1. Query oracle with InitialStorageSlotQuery                  │
│    2. Oracle derives flat_key = Blake2s(address ++ key)          │
│    3. Oracle looks up in tree/database                           │
│    4. Oracle returns { initial_value, is_new_slot }              │
│    5. Cache the value with metadata                              │
└─────────────────────────────────────────────────────────────────┘
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                 State Commitment Flow                            │
│                                                                  │
│  1. All changed slots tracked in cache                           │
│  2. At finalization:                                             │
│     - Compute storage diffs (initial → final)                    │
│     - Update merkle tree leaves for changed slots                │
│     - Recompute merkle root from bottom up                       │
│  3. Merkle root becomes state root                               │
│  4. Merkle proofs enable verification:                           │
│     - Given (flat_key, value, proof)                             │
│     - Verify: compute_root(flat_key, value, proof) == state_root│
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                  Flat Storage Benefits                           │
│                                                                  │
│  ✓ Simple key derivation: Blake2s(address ++ key)               │
│  ✓ Deterministic ordering: flat keys are sortable               │
│  ✓ Efficient proofs: binary merkle tree                         │
│  ✓ Fast RISC-V proving: minimal hashing operations              │
│  ✓ Compact representation: no trie overhead                     │
└─────────────────────────────────────────────────────────────────┘
```

---

## Cross-References

**For deeper understanding of storage in context, see:**

- **[KeyTypes.md](./KeyTypes.md)**: Detailed documentation of `WarmStorageKey`, `WarmStorageValue`, and related types
- **[SystemLayer.md](./SystemLayer.md)**: I/O subsystem details, resource accounting, and system primitives
- **[DataFlow.md](./DataFlow.md)**: Complete data flow showing how storage operations fit into execution pipeline
- **[ExecutionEnvironments.md](./ExecutionEnvironments.md)**: How execution environments interact with storage (SLOAD/SSTORE opcodes)
- **[API.md](./API.md)**: BlockOutput structure and how storage writes are returned to callers

---

## Summary

This document covered zkSync OS storage system:

**Storage Models** (2 implementations):
- Flat Storage Model: Optimized for zkSync with Blake2s key derivation
- Ethereum MPT Model: Full Ethereum compatibility with Merkle-Patricia Trie

**Storage Access**:
- Read path with warm/cold access distinction (EIP-2929)
- Write path with EIP-2200 gas calculation and pubdata tracking
- Transient storage (EIP-1153) for transaction-scoped data

**Storage Key Types**:
- `WarmStorageKey`: Identifies storage location (address + key)
- `WarmStorageValue`: Tracks slot lifecycle, warmness, and pubdata

**State Management**:
- Snapshots for rollback on revert
- Journaling of all state changes
- Finalization at transaction and block boundaries
- Merkle proof generation for state verification

**Oracle Integration**:
- `ReadTreeResponder`: Full tree operations with Merkle proofs
- `ReadStorageResponder`: Simple storage reads for simulations
- Query protocol for deterministic external data access

**Storage Output**:
- `StorageWrite`: Changed slots (initial → final value)
- `AccountDiff`: Account state changes
- Preimage publication for L1 verification

The storage system provides a robust abstraction enabling efficient state management while maintaining Ethereum compatibility and supporting zero-knowledge proof generation.
