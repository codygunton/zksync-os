# Hash Functions in ZKsync OS

## Overview

ZKsync OS uses a dual-hash architecture:
- BLAKE2s: Internal commitments (ZK-optimized, delegated to specialized circuit)
- Keccak-256: Ethereum compatibility only (not delegated, runs as regular RISC-V instructions)

Keccak is implemented in Rust (`crypto/src/sha3/mod.rs` wrapping the `sha3` crate, plus a custom implementation in `supporting_crates/keccak/src/lib.rs`) and compiled to RISC-V like everything else. The difference is that BLAKE2s gets special treatment: its round function is delegated to a specialized ZK circuit via CSR 0x7c7, while Keccak runs as ordinary RISC-V instructions that must each be proven individually.

## Delegation Mechanism

Hash operations are triggered via RISC-V CSR (Control/Status Register) instructions:

| CSR   | Hash       | Delegation                | Call Count |
|-------|------------|---------------------------|------------|
| 0x7c7 | BLAKE2s    | Yes (specialized circuit) | ~640       |
| N/A   | Keccak-256 | No (native RISC-V)        | ~81        |

BLAKE2s is delegated because it's used heavily for internal state and is more ZK-friendly.
Keccak is computed instruction-by-instruction since it's only needed for Ethereum compatibility.

## BLAKE2s Usage

### 1. Flat Storage Tree
File: `basic_system/src/system_implementation/flat_storage_model/simple_growable_storage.rs:113-140`

The main state storage structure. Uses a linked-list backed array with 64-level Merkle proofs.

```rust
pub struct Blake2sStorageHasher { _marker: PhantomData<()> }

impl FlatStorageHasher for Blake2sStorageHasher {
    fn hash_leaf<const N: usize>(&mut self, leaf: &FlatStorageLeaf<N>) -> Bytes32;
    fn hash_node(&mut self, left: &Bytes32, right: &Bytes32) -> Bytes32;
}
```

### 2. Storage Key Derivation
File: `zk_ee/src/common_structs/warm_storage_key.rs:49-73`

Derives flat storage keys from (address, slot) pairs:

```rust
pub fn derive_flat_storage_key(address: &B160, key: &Bytes32) -> Bytes32 {
    let mut hasher = Blake2s256::new();
    // Pad address to 32 bytes
    let mut extended_address = Bytes32::ZERO;
    extended_address.as_u8_array_mut()[12..].copy_from_slice(&address.to_be_bytes());
    hasher.update(extended_address.as_u8_array_ref());
    hasher.update(key.as_u8_array_ref());
    Bytes32::from_array(hasher.finalize())
}
```

### 3. Chain State Commitment
File: `basic_system/src/system_implementation/system/public_input.rs:23-46`

Commits to state between blocks:

```rust
pub struct ChainStateCommitment {
    pub state_root: Bytes32,
    pub next_free_slot: u64,
    pub block_number: u64,
    pub last_256_block_hashes_blake: Bytes32,  // BLAKE2s chain of block hashes
    pub last_block_timestamp: u64,
}

impl ChainStateCommitment {
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(self.state_root.as_u8_ref());
        hasher.update(&self.next_free_slot.to_be_bytes());
        hasher.update(&self.block_number.to_be_bytes());
        hasher.update(self.last_256_block_hashes_blake.as_u8_ref());
        hasher.update(&self.last_block_timestamp.to_be_bytes());
        hasher.finalize()
    }
}
```

### 4. Blocks Output (Aggregation)
File: `basic_system/src/system_implementation/system/public_input.rs:59-94`

```rust
pub struct BlocksOutput {
    pub chain_id: U256,
    pub first_block_timestamp: u64,
    pub last_block_timestamp: u64,
    pub pubdata_hash: Bytes32,              // BLAKE2s
    pub priority_ops_hashes_hash: Bytes32,  // BLAKE2s
    pub l2_to_l1_logs_hashes_hash: Bytes32, // BLAKE2s
    pub upgrade_tx_hash: Bytes32,
}
```

---

## Keccak-256 Usage

### 1. Ethereum MPT (Merkle Patricia Trie)
File: `basic_system/src/system_implementation/ethereum_storage_model/mpt/trie.rs:124`

Used for Ethereum state compatibility:

```rust
fn compute_key(&self, hasher: &mut crypto::sha3::Keccak256, ...) {
    // Ethereum MPT requires Keccak-256 for node hashing
}
```

### 2. EVM KECCAK256 Opcode (SHA3)
File: `evm_interpreter/src/instructions/system.rs:37`

```rust
S::SystemFunctions::keccak256(&input, &mut dst, self.gas.resources_mut(), allocator)
```

### 3. CREATE/CREATE2 Address Derivation
File: `evm_interpreter/src/interpreter.rs:466,511`

```rust
// CREATE2: keccak256(0xff ++ sender ++ salt ++ keccak256(init_code))
let new_address = Keccak256::digest(&create2_buffer);
```

### 4. Bytecode Hash
File: `evm_interpreter/src/utils.rs:20-21`

```rust
use crypto::sha3::{Digest, Keccak256};
let hash = Keccak256::digest(bytecode);
```

---

## Proving Efficiency

### Two Levels of Hashing

When a contract executes `SLOAD`/`SSTORE`, hashing occurs at two levels:

Level 1: EVM Application Layer (Keccak)

Solidity computes storage slot indices using Keccak. For example, `mapping(address => uint) balances` stores values at `keccak256(addr . slot_number)`. The EVM interpreter implements this via the SHA3 opcode:

- Opcode definition: `evm_interpreter/src/opcodes.rs:41` (`SHA3 = 0x20`)
- Implementation: `evm_interpreter/src/instructions/system.rs:18` (`fn sha3()`)
- Keccak call: `evm_interpreter/src/instructions/system.rs:37`

Level 2: ZKsync OS Storage Layer (BLAKE2s)

Once the EVM computes the 32-byte slot key, ZKsync OS:
1. Derives the flat storage key via `BLAKE2s(address || slot_key)` — see `zk_ee/src/common_structs/warm_storage_key.rs:49`
2. Proves Merkle membership with 64 × BLAKE2s hashes (tree depth = 64) — see `basic_system/src/system_implementation/flat_storage_model/simple_growable_storage.rs:113`

Reference: `docs/system/io/tree.md`

### Cache Efficiency

ZKsync OS maintains three caches (storage, account, preimage) that sit between the EVM and the Merkle tree. These caches accumulate reads and writes during block execution, then apply only the net state changes to the tree at finalization. This means repeated accesses to the same slot don't hit the tree multiple times.
File: `docs/system/io/tree.md`
> "If in a block a given slot is read initially with value `A`, then written with value `B` and then written again with value `C`, only the update `A → C` will be performed on the tree. The rest of the interactions are handled by the caches."

| Operation Pattern            | Without Cache     | With Cache                       |
|------------------------------|-------------------|----------------------------------|
| 1000 reads of same slot      | 1000 × 64 BLAKE2s | 1 × 64 BLAKE2s                   |
| Read → Write → Write → Write | 4 × 64 BLAKE2s    | 2 × 64 BLAKE2s (initial + final) |
| Hot storage loop             | O(n) tree ops     | O(1) tree ops                    |

Additional amortization from `docs/system/io/tree.md`:
> "Hashes in Merkle paths close to the root will most probably be the same for a significant portion of paths, and we don't do duplicate hashing for them either."

### The Combined Effect

1. Replace expensive with cheap: Internal Merkle proofs use BLAKE2s instead of Keccak
2. Minimize the expensive: Keccak only where Ethereum demands it (~81 vs ~640 calls)
3. Cache the cheap: BLAKE2s operations amortized via storage/account/preimage caches
4. Delegate the cheap: BLAKE2s delegated to specialized circuit (CSR 0x7c7)

---

## What ZKsync OS Actually Proves

### EVM Execution Equivalence: ✅ Yes

ZKsync OS proves that EVM bytecode executes with correct semantics:
- Same opcodes, same gas costs, same behavior
- SHA3 opcode returns correct Keccak hash
- CREATE/CREATE2 derive correct addresses
- Contract storage reads/writes are consistent

File: `evm_interpreter/src/` - Full EVM implementation

### Ethereum State Root Compatibility: ❌ No

ZKsync OS does not produce Ethereum-compatible state roots:

| Component   | Ethereum L1              | ZKsync OS                 |
|-------------|--------------------------|---------------------------|
| State tree  | Keccak-based MPT         | BLAKE2s flat tree         |
| State root  | `keccak(MPT_root)`       | `BLAKE2s(flat_tree_root)` |
| Storage key | `keccak(addr \|\| slot)` | `BLAKE2s(addr \|\| slot)` |

File: `basic_system/src/system_implementation/flat_storage_model/` vs `ethereum_storage_model/`

### Implications for `eth_runner`

The `eth_runner` test instance can prove that Ethereum transactions execute correctly, but:
- The resulting state commitment uses ZKsync's BLAKE2s format
- It does not produce the same `stateRoot` as an Ethereum block header
- This is sufficient for L2 rollups but not for replacing Ethereum L1 consensus

File: `tests/instances/eth_runner/`

To prove Ethereum L1 blocks with compatible state roots, you would need to use the Keccak-based MPT (`ethereum_storage_model/`), accepting higher proving costs.

---

## EVM Test Suite: What They Actually Verify

ZKsync OS passes the Ethereum Foundation's execution spec tests, but not by verifying state roots.

### Test Fixtures Downloaded

File: `tests/evm_tester/download_ethereum_fixtures.sh`
```bash
# Downloads official Ethereum execution-spec-tests v5.1.0
DEVELOP_TAR_URL="https://github.com/ethereum/execution-spec-tests/releases/..."
```

### Test Fixture Structure

Each test contains:
```json
{
  "pre": { /* initial account states */ },
  "transaction": { /* tx to execute */ },
  "post": {
    "Cancun": [{
      "hash": "0x1234...",           // ← Ethereum state root (NOT VERIFIED)
      "state": {                      // ← Individual account states (VERIFIED)
        "0xaddr1": { "balance": "0x100", "nonce": "0x1", "storage": {...} }
      }
    }]
  }
}
```

File: `tests/evm_tester/src/test/test_structure/post_state.rs:16-23`
```rust
pub struct PostState {
    pub indexes: PostStateIndexes,
    pub hash: B256,                    // Ethereum state root - PARSED BUT IGNORED
    pub logs: B256,
    pub txbytes: Bytes,
    pub expect_exception: Option<String>,
    pub state: Option<HashMap<...>>,   // Individual accounts - ACTUALLY VERIFIED
}
```

### What Gets Verified

File: `tests/evm_tester/src/test/case/mod.rs:576-676`

The test harness checks individual values, not the state root:

```rust
// Balance check (line 593)
if vm.get_balance(address) != expected_balance_value { ... }

// Nonce check (line 608)
if vm.get_nonce(address) != expected_nonce_value { ... }

// Code check (line 621)
if actual_code != filler_struct.code { ... }

// Storage check (line 645)
if unwrapped_actual_value.0 != expected_u256.to_be_bytes() { ... }
```

The `hash` field (Ethereum state root) is never compared.


### Stack Trace: Test Execution to Verification

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ cargo run --bin evm-tester                                                  │
│ tests/evm_tester/src/evm_tester/main.rs:21                                  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ main_inner(arguments)                                                       │
│ tests/evm_tester/src/evm_tester/main.rs:34                                  │
│                                                                             │
│   let evm_tester = EvmTester::new(...);                                     │
│   evm_tester.run_zksync_os(arguments.mutation)?;  ◄── line 79               │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ EvmTester::run_zksync_os()                                                  │
│ tests/evm_tester/src/lib.rs:78                                              │
│                                                                             │
│   let tests = self.all_tests(Environment::ZKsyncOS)?;                       │
│   tests.into_par_iter().map(|mut test| {                                    │
│       test.run_zksync_os(self.summary.clone(), self.proof_run);  ◄── line 86│
│   })                                                                        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Test::run_zksync_os()                                                       │
│ tests/evm_tester/src/test/mod.rs:265                                        │
│                                                                             │
│   for case in self.cases {                                                  │
│       let vm = ZKsyncOS::new();                                             │
│       case.run_zksync_os(summary.clone(), vm, ...);  ◄── line 292           │
│   }                                                                         │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Case::run_zksync_os()                                                       │
│ tests/evm_tester/src/test/case/mod.rs:506                                   │
│                                                                             │
│   // Setup pre-state                                                        │
│   for (address, account_state) in &self.prestate { ... }                    │
│                                                                             │
│   // Execute transactions                                                   │
│   let run_result = Self::run_zksync_os_blocks(...);  ◄── line 574           │
│                                                                             │
│   // ════════════════════════════════════════════════════════════════════   │
│   // VERIFICATION HAPPENS HERE (lines 576-676)                              │
│   // ════════════════════════════════════════════════════════════════════   │
│                                                                             │
│   for (address, filler_struct) in self.expected_state {                     │
│       // ✅ Balance check (line 593)                                        │
│       if vm.get_balance(address) != expected_balance_value { FAIL }         │
│                                                                             │
│       // ✅ Nonce check (line 608)                                          │
│       if vm.get_nonce(address) != expected_nonce_value { FAIL }             │
│                                                                             │
│       // ✅ Code check (line 621)                                           │
│       if actual_code != filler_struct.code { FAIL }                         │
│                                                                             │
│       // ✅ Storage check (line 645)                                        │
│       if storage_value != expected_value { FAIL }                           │
│   }                                                                         │
│                                                                             │
│   // ❌ NO STATE ROOT CHECK                                                 │
│   // PostState.hash is NEVER compared against computed state                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Where the State Root Gets Dropped

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Case::from_ethereum_spec_state_test()                                       │
│ tests/evm_tester/src/test/case/mod.rs:251                                   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ for post_state in post_state_structs.unwrap() {        ◄── line 272         │
│                                                                             │
│     // post_state has BOTH fields:                                          │
│     // - post_state.hash: B256        (Ethereum state root)                 │
│     // - post_state.state: HashMap    (individual account states)           │
│                                                                             │
│     // ❌ ONLY state is extracted, hash is IGNORED                          │
│     let expected_accounts =                                                 │
│         ExpectStructure::get_expected_result(                               │
│             post_state.state.as_ref().unwrap()   ◄── line 279-280           │
│         );                                       // hash not used!          │
│                                                                             │
│     expected_results_states.push((expected_accounts, expect_exception));    │
│ }                                                                           │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Case is constructed with:                                                   │
│                                                                             │
│   Case {                                                                    │
│       label: String,                                                        │
│       prestate: PreState,                                                   │
│       pre_blocks: Vec<PreBlock>,                                            │
│       expected_state: HashMap<Address, AccountFillerStruct>,  ◄── from state│
│       skip_balance_check_for_sender_and_coinbase: bool,                     │
│       // ❌ NO hash field - state root is lost here                         │
│   }                                                                         │
└─────────────────────────────────────────────────────────────────────────────┘
