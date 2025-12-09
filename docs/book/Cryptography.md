# zkSync OS Cryptography

## Table of Contents

- [Introduction](#introduction)
- [Cryptographic Primitives](#cryptographic-primitives)
- [EVM Precompiles](#evm-precompiles)
- [Callable Oracles](#callable-oracles)
- [Integration with Execution](#integration-with-execution)
- [Proof System Crypto](#proof-system-crypto)
- [Build Profiles and Precompiles](#build-profiles-and-precompiles)

---

## Introduction

The zkSync OS cryptographic system provides a comprehensive suite of cryptographic primitives, EVM-compatible precompiles, and expensive cryptographic operations delegated to the host environment. This layered approach enables efficient verification within the zero-knowledge proof system while maintaining full Ethereum compatibility.

### Cryptographic System Overview

The zkSync OS cryptography stack is organized into three primary layers:

1. **Cryptographic Primitives** (`crypto/` crate): Core cryptographic functions including hash algorithms, digital signatures, and elliptic curve operations
2. **EVM Precompiles** (`system_hooks/` crate): Ethereum-compatible precompiled contracts at addresses 0x01-0x09 and extended precompiles
3. **Callable Oracles** (`callable_oracles/` crate): Operations too expensive for RISC-V execution, delegated to the host environment

### Role in zkSync OS

Cryptography serves multiple critical roles:

- **Transaction Verification**: Signature validation using secp256k1 (ecrecover) and P256
- **State Management**: Hash functions for Merkle trees and storage proofs (Keccak256, SHA256, Blake2s)
- **Precompile Support**: EVM-compatible precompiles for cryptographic operations callable from smart contracts
- **Data Availability**: KZG commitments and proofs for EIP-4844 blob transactions
- **Proof Generation**: Elliptic curve pairings (BN254) for zero-knowledge proof verification

The system is designed for dual execution modes:
- **Forward mode**: Native execution using optimized implementations
- **Proof mode**: Operations delegated to callable oracles for efficiency in RISC-V environment

---

## Cryptographic Primitives

The `crypto/` crate (`/crypto/src/lib.rs`) provides low-level cryptographic implementations optimized for both native execution and zero-knowledge proving. All primitives support deterministic execution required for proof generation.

### Hash Functions

zkSync OS implements multiple hash functions to support various cryptographic use cases:

#### Blake2 / Blake2s

**Location**: `/crypto/src/blake2s/mod.rs`

**Purpose**: Fast, cryptographically secure hash function used for:
- Internal system hashing
- KZG commitment challenge point generation
- Data availability commitments

**Implementations**:
- **Naive**: Pure Rust implementation for portability
- **Delegated Extended**: Optimized version using host delegation in proof mode

**Algorithm**: Blake2s produces 256-bit (32-byte) hash outputs with high performance characteristics.

**Usage Example**:
```rust
use crypto::MiniDigest;
use crypto::blake2s::Blake2s256;

let mut hasher = Blake2s256::new();
hasher.update(b"data to hash");
let hash = hasher.finalize();  // 32-byte output
```

**Performance Notes**: Blake2s is preferred over SHA256 for internal operations due to superior performance on modern CPUs, particularly in native forward execution mode.

#### SHA256

**Location**: `/crypto/src/sha256/mod.rs`

**Purpose**: Ethereum-compatible SHA256 implementation for:
- EVM precompile at address 0x02
- Transaction hashing
- Legacy Ethereum compatibility

**Algorithm**: SHA-2 family producing 256-bit outputs

**EVM Integration**: Directly exposed as precompile for smart contract use

#### SHA3 / Keccak256 / Keccak512

**Location**: `/crypto/src/sha3/mod.rs`

**Purpose**: Primary hash function for Ethereum compatibility:
- Keccak256: EVM opcode (SHA3), address derivation, Merkle trees
- Keccak512: Extended hashing operations

**Key Differences**:
- **Keccak256**: Original Keccak submission (used by Ethereum)
- **SHA3-256**: NIST standardized version (different padding)

**Critical Note**: Ethereum uses Keccak256, NOT NIST SHA3-256. The EVM SHA3 opcode actually computes Keccak256.

**Implementation**: Uses optimized Keccak-f[1600] permutation function with proper padding for Ethereum compatibility.

#### RIPEMD160

**Location**: `/crypto/src/ripemd160/mod.rs`

**Purpose**:
- Bitcoin address compatibility
- EVM precompile at address 0x03
- Legacy cryptographic operations

**Algorithm**: RIPE Message Digest producing 160-bit (20-byte) outputs

**Use Cases**: Primarily for Bitcoin-style address derivation and legacy compatibility with contracts expecting RIPEMD160 hashes.

### Digital Signatures

zkSync OS supports multiple elliptic curve digital signature schemes for transaction verification and cryptographic operations.

#### secp256k1 (ECDSA)

**Location**: `/crypto/src/secp256k1/mod.rs`

**Curve**: secp256k1 (Bitcoin/Ethereum standard curve)
- Field prime: `p = 2^256 - 2^32 - 977`
- Order: ~2^256 (256-bit security level)

**Purpose**:
- Ethereum transaction signature verification (ecrecover)
- Standard ECDSA signature scheme for EVM

**Key Components** (`/crypto/src/secp256k1/`):
- **Field arithmetic**: `field/mod.rs` - Optimized field operations
  - Multiple implementations: field_5x52 (64-bit), field_8x32 (32-bit), field_10x26
  - Modular inversion: Montgomery ladder for constant-time operations
- **Point operations**: `points/mod.rs` - Elliptic curve point arithmetic
  - Affine coordinates: `points/affine.rs`
  - Jacobian coordinates: `points/jacobian.rs` (faster for repeated operations)
  - Storage format: `points/storage.rs`
- **Scalar operations**: `scalars/mod.rs` - Scalar field arithmetic
  - 32-bit scalars: `scalars/scalar32.rs`
  - 64-bit scalars: `scalars/scalar64.rs`
  - Scalar inversion: `scalars/invert.rs`
- **Signature recovery**: `recover.rs` - ecrecover implementation

**ecrecover Implementation**:
```rust
// Pseudo-code structure
pub fn ecrecover(hash: &[u8; 32], v: u8, r: &[u8; 32], s: &[u8; 32]) -> Result<Address, Error> {
    // 1. Validate signature parameters
    // 2. Recover public key from signature
    // 3. Hash public key to derive Ethereum address
}
```

**Performance Optimizations**:
- Static context precomputation (enabled via `secp256k1-static-context` feature)
- GLV decomposition for scalar multiplication (`/crypto/src/glv_decomposition.rs`)
- Endomorphism acceleration using curve-specific properties

**Feature Flag**: `secp256k1-static-context` enables precomputed multiplication tables for 2-3x speedup

#### P256 (ECDSA)

**Location**: `/crypto/src/p256/mod.rs`, `/crypto/src/secp256r1/mod.rs`

**Curve**: secp256r1 / P-256 (NIST standard curve)
- Also known as: prime256v1, secp256r1
- Field prime: `p = 2^256 - 2^224 + 2^192 + 2^96 - 1`

**Purpose**:
- WebAuthn/Passkey signature verification
- Hardware security module (HSM) compatibility
- Government and enterprise cryptographic standards

**Key Components** (`/crypto/src/secp256r1/`):
- **Context**: `context.rs` - Precomputed tables and curve parameters
- **Field arithmetic**: `field/mod.rs`
  - 64-bit field elements: `field/fe64.rs`
  - Delegated operations: `field/fe32_delegation.rs` (for proof mode)
  - U64 arithmetic utilities: `u64_arithmetic.rs`
- **Point operations**: `points/mod.rs`
  - Affine: `points/affine.rs`
  - Jacobian: `points/jacobian.rs`
  - Storage: `points/storage.rs`
- **Scalar operations**: `scalar/mod.rs`
  - 64-bit scalars: `scalar/scalar64.rs`
  - Delegated scalars: `scalar/scalar_delegation.rs`
- **Signature verification**: `verify.rs` - P256 ECDSA verification
- **Windowed NAF**: `wnaf.rs` - Non-adjacent form for efficient scalar multiplication

**Precompile Address**: 0x0100 (extended precompile, not standard Ethereum)

**Feature Flag**: `p256_precompile` enables P256 signature verification precompile

**Build Profile Support**:
- **Production**: P256 enabled by default
- **for_tests**: P256 enabled
- **evm_tester**: P256 enabled

**RIP-7212 Compatibility**: The P256 precompile follows the RIP-7212 specification for secp256r1 curve signature verification, enabling WebAuthn/Passkey integration in smart contracts.

### Elliptic Curve Operations

zkSync OS supports advanced elliptic curve operations for zero-knowledge proof verification and cryptographic protocols.

#### BN254 (BN128)

**Location**: `/crypto/src/bn254/mod.rs`

**Curve**: BN254 (also known as alt-bn128 or BN128)
- Pairing-friendly curve
- 254-bit prime field
- Embedding degree: 12

**Purpose**:
- zkSNARK verification (Groth16, Plonk)
- EVM precompiles for pairing-based cryptography
- Zero-knowledge proof verification

**Components** (`/crypto/src/bn254/`):
- **Curves**: `curves/mod.rs`
  - G1 curve: `curves/g1.rs` (base field points)
  - G2 curve: `curves/g2.rs` (extension field points)
  - Pairing implementation: `curves/pairing_impl.rs` - Optimal Ate pairing
- **Fields**: `fields/mod.rs`
  - Base field Fq: `fields/fq.rs`
  - Scalar field Fr: `fields/fr.rs`
  - Extension field Fq2: `fields/fq2.rs` (quadratic extension)
  - Extension field Fq6: `fields/fq6.rs` (sextic extension)
  - Extension field Fq12: `fields/fq12.rs` (full extension for pairing)

**Operations**:
- **EC Add** (precompile 0x06): Add two G1 points
- **EC Mul** (precompile 0x07): Scalar multiplication on G1
- **Pairing Check** (precompile 0x08): Verify pairing equations for zkSNARK verification

**Pairing Algorithm**: Optimal Ate pairing with final exponentiation, producing values in Fq12

**Performance**: Pairing operations are computationally expensive; typically delegated to callable oracles in proof mode

#### BLS12-381

**Location**: `/crypto/src/bls12_381/mod.rs`

**Curve**: BLS12-381
- Pairing-friendly curve
- 381-bit prime field
- Embedding degree: 12
- High security level (~128-bit)

**Purpose**:
- BLS signatures (aggregate signatures)
- Advanced zkSNARK systems
- Future Ethereum consensus layer compatibility

**Components** (`/crypto/src/bls12_381/`):
- **Constants**: `consts.rs` - Curve parameters and constants
- **Curves**: `curves/mod.rs`
  - G1 curve: `curves/g1.rs`
  - G2 curve: `curves/g2.rs`
  - G1 isogeny mapping: `curves/g1_swu_iso.rs` (hash-to-curve)
  - G2 isogeny mapping: `curves/g2_swu_iso.rs`
  - Pairing: `curves/pairing_impl.rs`
  - Utilities: `curves/util.rs`
- **Fields**: `fields/mod.rs`
  - Base field Fq: `fields/fq.rs`
  - Scalar field Fr: `fields/fr.rs`
  - Quadratic extension Fq2: `fields/fq2.rs`
  - Sextic extension Fq6: `fields/fq6.rs`
  - Full extension Fq12: `fields/fq12.rs`

**Hash-to-Curve**: Implements Shallue-van de Woestijne-Ulas (SWU) algorithm with isogeny mapping for secure hash-to-curve operations

**Status**: BLS12-381 operations are implemented but not currently exposed as EVM precompiles. Used internally for advanced cryptographic protocols.

### Bigint Operations and Delegation

**Location**: `/crypto/src/bigint_delegation/mod.rs`, `/crypto/src/ark_ff_delegation/mod.rs`

**Purpose**: Efficient arbitrary-precision integer arithmetic for cryptographic operations

**Delegation Interface** (`/crypto/src/raw_delegation_interface.rs`):
```rust
pub enum BigIntOps {
    Add = 0,
    Sub = 1,
    SubAndNegate = 2,
    MulLow = 3,      // Low 256 bits of multiplication
    MulHigh = 4,     // High 256 bits of multiplication
    Eq = 5,          // Equality comparison
    MemCpy = 7,      // Memory copy
}
```

**Delegation Mechanism**:
- **Native mode**: Direct computation using optimized algorithms
- **Proof mode**: Delegated to host via oracle queries for efficiency

**Feature Flags**:
- `bigint_ops`: Enable bigint delegation in RISC-V
- `proving`: Enable cryptographic delegations for proof generation

**arkworks Integration**: The `ark_ff_delegation` module provides delegated implementations of arkworks field arithmetic traits (`Fp`, `BigInteger`) for seamless integration with zkSNARK libraries.

---

## EVM Precompiles

EVM precompiles are special contracts at low addresses (< 0x10000) that provide cryptographic operations callable via the CALL opcode. zkSync OS implements all standard Ethereum precompiles plus extended precompiles for additional functionality.

### Standard Ethereum Precompiles

The following table lists all standard Ethereum precompiles supported by zkSync OS:

| Address | Name | Purpose | Input | Output | Gas Cost |
|---------|------|---------|-------|--------|----------|
| 0x0001 | ecrecover | ECDSA public key recovery | hash(32), v(32), r(32), s(32) | address(20) padded to 32 | 3000 |
| 0x0002 | SHA256 | SHA-256 hash function | data(variable) | hash(32) | 60 + 12/word |
| 0x0003 | RIPEMD160 | RIPEMD-160 hash function | data(variable) | hash(20) padded to 32 | 600 + 120/word |
| 0x0004 | identity | Identity/datacopy | data(variable) | data(same) | 15 + 3/word |
| 0x0005 | modexp | Modular exponentiation | B,E,M(variable) | result(variable) | dynamic |
| 0x0006 | ecadd | BN254 point addition | x1,y1,x2,y2(128) | x,y(64) | 150 |
| 0x0007 | ecmul | BN254 scalar multiplication | x,y,scalar(96) | x,y(64) | 6000 |
| 0x0008 | ecpairing | BN254 pairing check | pairs(variable) | success(32) | 45000 + 34000/pair |
| 0x0009 | blake2f | Blake2 compression function | rounds(4), h(64), m(128), t(16), f(1) | h(64) | rounds |

**Implementation Location**: `/system_hooks/src/precompiles.rs`, `/system_hooks/src/lib.rs`

**Address Constants**: `/system_hooks/src/addresses_constants.rs`

#### Precompile Details

##### 0x01: ecrecover (EC Recovery)

**Purpose**: Recover Ethereum address from ECDSA signature

**Implementation**: Uses secp256k1 signature recovery algorithm

**Input Format** (128 bytes total):
- Bytes 0-31: Message hash
- Bytes 32-63: Recovery ID (v) - only byte 63 used, must be 27 or 28
- Bytes 64-95: Signature component r
- Bytes 96-127: Signature component s

**Output**: 32 bytes (12 zero bytes + 20-byte address)

**Error Handling**: Returns empty output on invalid signature (doesn't revert)

**Gas Cost**: Fixed 3000 gas

**Native Cost**: Depends on signature validation operations

**Code Location**: `/system_hooks/src/lib.rs:206-208` (hook registration)

##### 0x02: SHA256

**Purpose**: Compute SHA-256 hash

**Implementation**: SHA-2 family cryptographic hash

**Input**: Arbitrary length byte array

**Output**: 32-byte hash

**Gas Cost**: 60 gas base + 12 gas per word (32 bytes)

**Native Cost**: `SHA256_BASE_NATIVE_COST + SHA256_BYTE_NATIVE_COST * input_len`

**Code Location**: `/system_hooks/src/lib.rs:209-211`

##### 0x03: RIPEMD160

**Purpose**: Compute RIPEMD-160 hash

**Implementation**: RIPEMD-160 algorithm producing 20-byte hashes

**Input**: Arbitrary length byte array

**Output**: 32 bytes (12 zero bytes + 20-byte hash)

**Gas Cost**: 600 gas base + 120 gas per word

**Native Cost**: Similar to SHA256 with different constants

**Code Location**: `/system_hooks/src/lib.rs:212-214`

##### 0x04: identity (Datacopy)

**Purpose**: Copy input data to output (identity function)

**Implementation**: Simple memory copy operation

**Input**: Arbitrary length byte array

**Output**: Exact copy of input

**Gas Cost**: 15 gas base + 3 gas per word

**Native Cost**:
- Base: 20 native units
- Per byte: 10 native units

**Use Cases**:
- Memory copying
- Data validation
- Gas cost calculation testing

**Code Location**: `/system_hooks/src/precompiles.rs:111-136` (custom implementation)

##### 0x05: modexp (Modular Exponentiation)

**Purpose**: Compute modular exponentiation: `(base^exponent) mod modulus`

**Implementation**: Montgomery multiplication with sliding window exponentiation

**Input Format** (variable length):
- Bytes 0-31: Length of base (Bsize)
- Bytes 32-63: Length of exponent (Esize)
- Bytes 64-95: Length of modulus (Msize)
- Bytes 96+: Base (Bsize bytes)
- Bytes 96+Bsize: Exponent (Esize bytes)
- Bytes 96+Bsize+Esize: Modulus (Msize bytes)

**Output**: Result of modular exponentiation (Msize bytes)

**Gas Cost**: Complex calculation based on input sizes and complexity
- Minimum: 200 gas
- Scales with multiplication complexity

**Arithmetic Oracle Integration**: Uses `MODEXP_ADVICE_QUERY_ID` for division advice in proof mode

**Code Location**: `/system_hooks/src/lib.rs:216-218`

##### 0x06: ecadd (BN254 EC Addition)

**Purpose**: Add two points on the BN254 elliptic curve

**Implementation**: BN254 G1 point addition using optimized field arithmetic

**Input Format** (128 bytes):
- Bytes 0-31: x1 coordinate (big-endian)
- Bytes 32-63: y1 coordinate
- Bytes 64-95: x2 coordinate
- Bytes 96-127: y2 coordinate

**Output**: 64 bytes (x, y coordinates of sum)

**Special Cases**:
- Point at infinity represented as (0, 0)
- Invalid points return failure

**Gas Cost**: Fixed 150 gas

**Code Location**: `/system_hooks/src/lib.rs:219-221`

##### 0x07: ecmul (BN254 Scalar Multiplication)

**Purpose**: Multiply BN254 G1 point by scalar

**Implementation**: Scalar multiplication using windowed NAF or double-and-add

**Input Format** (96 bytes):
- Bytes 0-31: x coordinate
- Bytes 32-63: y coordinate
- Bytes 64-95: scalar (big-endian)

**Output**: 64 bytes (x, y coordinates of result)

**Gas Cost**: Fixed 6000 gas

**Code Location**: `/system_hooks/src/lib.rs:222-224`

##### 0x08: ecpairing (BN254 Pairing Check)

**Purpose**: Verify BN254 pairing equations for zkSNARK verification

**Implementation**: Optimal Ate pairing with final exponentiation

**Input Format** (variable length, multiple of 192 bytes):
- Each 192-byte chunk represents a pair:
  - Bytes 0-63: G1 point (x, y)
  - Bytes 64-191: G2 point (x, y) in Fq2

**Output**: 32 bytes
- Returns 1 (success) if pairing check passes
- Returns 0 (failure) if pairing check fails

**Verification**: Checks if `e(P1, Q1) * e(P2, Q2) * ... * e(Pn, Qn) = 1`

**Gas Cost**: 45000 base + 34000 per pair

**Use Cases**: Groth16 zkSNARK verification, Plonk verification

**Code Location**: `/system_hooks/src/lib.rs:225-227`

##### 0x09: blake2f

**Purpose**: Blake2b F compression function

**Status**: Mock implementation (not fully supported in production)

**Input Format** (213 bytes):
- Bytes 0-3: Number of rounds (big-endian u32)
- Bytes 4-67: State vector h (8 x 64-bit)
- Bytes 68-195: Message block m (16 x 64-bit)
- Bytes 196-211: Offset counters t (2 x 64-bit)
- Byte 212: Final block indicator f

**Output**: 64 bytes (updated state vector)

**Gas Cost**: Rounds (1 gas per round)

**Feature Flag**: `mock-unsupported-precompiles` provides placeholder implementation

**Code Location**: `/system_hooks/src/mock_precompiles.rs`, `/system_hooks/src/lib.rs:230-232`

### Extended Precompiles

zkSync OS extends the standard EVM precompile set with additional cryptographic operations:

#### 0x000a: Point Evaluation (EIP-4844)

**Purpose**: Verify KZG commitment opening for EIP-4844 blob transactions

**EIP Reference**: EIP-4844 (Proto-Danksharding)

**Implementation**: KZG polynomial commitment verification

**Input Format** (192 bytes):
- Bytes 0-31: Versioned hash
- Bytes 32-63: Evaluation point z
- Bytes 64-95: Expected value y
- Bytes 96-143: KZG commitment C (48 bytes, BLS12-381 G1 point)
- Bytes 144-191: KZG proof π (48 bytes, BLS12-381 G1 point)

**Output**: 64 bytes
- Bytes 0-31: FIELD_ELEMENTS_PER_BLOB (4096)
- Bytes 32-63: BLS_MODULUS

**Verification**: Checks that polynomial P committed to by C evaluates to y at point z:
- Verifies: `e(C - [y], [1]) = e(π, [s] - [z])`
- Where [s] is the secret setup point

**Gas Cost**: Fixed 50000 gas

**Feature Flag**: `point_eval_precompile` enables this precompile

**Build Profile Support**:
- **for_tests**: Enabled
- **evm_tester**: Enabled
- **production**: Disabled (placeholder mock)

**Address Constant**: `POINT_EVAL_HOOK_ADDRESS_LOW = 0x000a`

**Code Location**:
- Registration: `/system_hooks/src/lib.rs:239-242`
- Mock: `/system_hooks/src/mock_precompiles.rs:30-34`

**Integration with DA**: Used by blob transaction processing to verify blob commitments match the data posted to L1.

#### 0x0100: P256 Verify (RIP-7212)

**Purpose**: Verify secp256r1 (P-256) ECDSA signatures for WebAuthn/Passkey support

**RIP Reference**: RIP-7212 (secp256r1 Precompile)

**Implementation**: P-256 curve signature verification

**Input Format** (160 bytes):
- Bytes 0-31: Message hash
- Bytes 32-63: Signature r component
- Bytes 64-95: Signature s component
- Bytes 96-127: Public key x coordinate
- Bytes 128-159: Public key y coordinate

**Output**: 32 bytes
- Returns 1 (success) if signature is valid
- Returns 0 (failure) if signature is invalid

**Verification Algorithm**:
1. Validate signature components (r, s in valid range)
2. Validate public key is on curve
3. Compute signature verification equation using P-256 parameters
4. Return success/failure

**Gas Cost**: Fixed 3450 gas

**Feature Flag**: `p256_precompile` enables this precompile

**Build Profile Support**:
- **production**: Enabled
- **for_tests**: Enabled
- **evm_tester**: Enabled

**Address Constant**: `P256_VERIFY_PREHASH_HOOK_ADDRESS_LOW = 0x0100`

**Code Location**: `/system_hooks/src/lib.rs:244-249`

**Use Cases**:
- WebAuthn signature verification
- Passkey authentication in smart contracts
- Hardware security module (HSM) integration
- Government and enterprise cryptographic standards

---

## Callable Oracles

Callable oracles provide a mechanism for delegating expensive cryptographic operations to the host environment during proof generation. This is essential for maintaining performance in the RISC-V proving system where complex operations would be prohibitively expensive.

### Purpose

In RISC-V proof mode, certain cryptographic operations are too expensive to execute natively:
- Large integer arithmetic (modular exponentiation)
- Polynomial commitment operations (KZG proofs)
- Field arithmetic for zkSNARK-friendly fields
- Prime number generation (hash-to-prime)

Instead of executing these operations in RISC-V, zkSync OS delegates them to **callable oracles** that run on the host (x86/ARM) and provide results back to the RISC-V execution via the oracle query protocol.

**Key Properties**:
- Deterministic: Same inputs always produce same outputs
- Verifiable: Results can be verified efficiently within RISC-V
- Transparent: All oracle queries recorded in witness for proof verification

### Oracle Query Protocol

Callable oracle queries follow the standard zkSync OS oracle protocol:

1. **Query Initiation**: RISC-V code writes query ID and parameters to CSR registers
2. **Host Processing**: Host environment intercepts CSR writes and processes query
3. **Result Return**: Host writes result back via CSR reads
4. **Witness Recording**: All CSR operations recorded for proof generation

**Query ID Allocation**: Callable oracles use query ID ranges 0x8000-0x8FFF and 0x0100+

### Arithmetic Query

**Location**: `/callable_oracles/src/arithmetic/mod.rs`

**Query ID**: `MODEXP_ADVICE_QUERY_ID` (referenced in `/basic_system/src/system_functions/modexp.rs`)

**Purpose**: Provide division advice for modular exponentiation

**Operation**: Given operands `n` and `d`, compute:
- Quotient: `q = n / d`
- Remainder: `r = n % d`

**Input Parameters** (via `ModExpAdviceParams` struct):
```rust
struct ModExpAdviceParams {
    a_ptr: u32,        // Pointer to dividend
    a_len: u32,        // Length of dividend in words
    b_ptr: u32,        // Pointer to divisor (0 for modexp advice)
    b_len: u32,        // Length of divisor (0 for modexp advice)
    modulus_ptr: u32,  // Pointer to modulus
    modulus_len: u32,  // Length of modulus in words
}
```

**Output Format**:
- Header: 64-bit value encoding quotient and remainder lengths
  - Low 32 bits: Quotient length in u32 words
  - High 32 bits: Remainder length in u32 words
- Data: Quotient (little-endian u64 array) followed by remainder

**Algorithm**:
1. Read dividend `n` and modulus `d` from memory
2. Compute division using `ruint::algorithms::div(&mut n, &mut d)`
3. Strip leading zeros from results
4. Encode as u64 array (note: usize = u64 on host)

**Use Case**: The modexp precompile (0x05) uses this oracle for division operations within Montgomery multiplication, avoiding expensive division in RISC-V.

**Performance**: Division delegated to host runs orders of magnitude faster than RISC-V implementation.

**Code Reference**: `/callable_oracles/src/arithmetic/mod.rs:10-94`

### Blob KZG Commitment Query

**Location**: `/callable_oracles/src/blob_kzg_commitment/mod.rs`

**Query ID**: `BLOB_COMMITMENT_AND_PROOF_QUERY_ID = 0x0103` (defined in `/basic_system/src/system_implementation/system/da_commitment_generator/blob_commitment_generator/commitment_and_proof_advice.rs:6`)

**Purpose**: Generate KZG commitment and proof for blob data (EIP-4844 data availability)

**Operation**: Given blob data, compute:
1. Encode data as blob (chunk by 31 bytes → BLS12-381 field elements)
2. Compute KZG commitment to polynomial
3. Generate KZG opening proof at challenge point

**Input Parameters**:
- `data_ptr: u32` - Pointer to blob data in RISC-V memory
- `data_len: u32` - Length of blob data in bytes

**Blob Encoding**:
```
For each 31-byte chunk of data:
  - Create 32-byte field element (1 zero byte + 31 data bytes)
  - Interpret as big-endian BLS12-381 scalar
Result: 4096 field elements (blob)
```

**KZG Commitment**: Polynomial commitment computed using trusted setup parameters
- Uses `c-kzg` library with Ethereum KZG settings (8 elements)
- Commitment: 48-byte BLS12-381 G1 point

**Challenge Point Derivation**:
```rust
challenge = Blake2s(versioned_hash || data)
// Truncate to 128 bits for security (Schwartz-Zippel lemma)
challenge[0..16] = 0
```

**KZG Proof**: Opening proof π demonstrating P(challenge) = y
- Proof: 48-byte BLS12-381 G1 point

**Output Format** (`KZGCommitmentAndProof` struct):
```rust
struct KZGCommitmentAndProof {
    commitment: [u8; 48],  // BLS12-381 G1 point
    proof: [u8; 48],       // BLS12-381 G1 point
}
```

**Serialization**: Serialized via `UsizeSerializable` trait into usize iterator for CSR streaming

**Use Cases**:
- Generate DA commitments for blob transactions
- Prove data availability without posting full data on-chain
- Support EIP-4844 rollup data availability

**Verification**: The point evaluation precompile (0x000a) verifies these commitments on-chain

**Performance**: KZG operations on BLS12-381 are extremely expensive; delegation to host essential for practical performance

**Code Reference**: `/callable_oracles/src/blob_kzg_commitment/mod.rs:17-101`

### Hash-to-Prime Query

**Location**: `/callable_oracles/src/hash_to_prime/mod.rs`

**Query ID**: `HASH_TO_PRIME_ORACLE_ID = 0x100`

**Purpose**: Generate cryptographically secure prime numbers from hash inputs

**Operation**: Given entropy, generate provably prime number using Miller-Rabin primality testing

**Constants**:
- `ENTROPY_BITS: u32 = 256` - Input entropy size
- `BIGINT_BITS: u32 = 322` - Output prime size
- `GENERATION_STEPS: [(u32, u32); 5]` - Multi-step prime search
  - Steps: (21,11), (20,11), (49,12), (108,13), (63,14)
  - Format: (bits of entropy, number of candidates to test)
- `MAX_ENTROPY_BYTES: u32` - Maximum entropy buffer size

**Algorithm**:
1. Generate candidate using entropy
2. Test primality using Miller-Rabin
3. If not prime, iterate with different entropy
4. Multi-step process with increasing candidate spaces

**Components**:
- **Common**: `/callable_oracles/src/hash_to_prime/common.rs` - Shared definitions
- **Compute**: `/callable_oracles/src/hash_to_prime/compute.rs` - Prime generation
- **Evaluate**: `/callable_oracles/src/hash_to_prime/evaluate.rs` - Oracle evaluation
- **Verify**: `/callable_oracles/src/hash_to_prime/verify.rs` - Primality verification

**Types**:
```rust
pub type S1BigInt = Uint<64, 1>;    // 64-bit
pub type S2BigInt = Uint<128, 2>;   // 128-bit
pub type S3BigInt = Uint<256, 4>;   // 256-bit
pub type S4BigInt = Uint<384, 6>;   // 384-bit
```

**Use Cases**:
- RSA-style protocols requiring proven primes
- Cryptographic protocols needing random primes
- Zero-knowledge proof systems with prime-order groups

**Performance**: Prime generation is expensive; delegation allows using optimized GMP-based algorithms

**Code Reference**: `/callable_oracles/src/hash_to_prime/mod.rs`

### Query ID Reference Table

The following table lists all callable oracle query IDs used in zkSync OS:

| Query ID | Constant Name | Processor | Purpose | Input | Output |
|----------|--------------|-----------|---------|-------|--------|
| `MODEXP_ADVICE_QUERY_ID` | ArithmeticQuery | ArithmeticQuery | Division advice for modexp | ModExpAdviceParams | Quotient + remainder |
| 0x0103 | `BLOB_COMMITMENT_AND_PROOF_QUERY_ID` | BlobCommitmentAndProofQuery | KZG commitment for blobs | data_ptr, data_len | KZGCommitmentAndProof |
| 0x0100 | `HASH_TO_PRIME_ORACLE_ID` | HashToPrimeQuery | Generate provable prime | entropy | prime number |

**Query ID Ranges**:
- 0x0000-0x00FF: Standard system queries (storage, preimages, metadata)
- 0x0100-0x01FF: Crypto-related callable oracles
- 0x8000-0x8FFF: Reserved for additional arithmetic oracles

**Feature-Gated Oracles**: Some oracles are only available in specific build profiles (e.g., `point_eval_precompile` feature enables KZG oracle).

---

## Integration with Execution

Cryptographic operations integrate deeply with the zkSync OS execution environment, accessible via EVM CALL opcodes and internal system functions.

### How CALL to Precompile Address Works

When EVM bytecode executes a CALL to a low address (< 0x10000), the system intercepts the call and routes it to a precompile implementation:

**Execution Flow**:

```
1. EVM Interpreter executes CALL opcode
         ↓
2. Bootloader detects low address (<0x10000)
         ↓
3. HooksStorage.try_intercept(address_low, request, ...)
         ↓
4. Look up hook in BTreeMap<u16, SystemHook>
         ↓
5. Execute hook function with:
   - ExternalCallRequest (calldata, gas, modifier)
   - System (IO, metadata, allocator)
   - Return memory buffer
         ↓
6. Hook implementation:
   - Charge resources (EVM gas + native)
   - Invoke system function / crypto primitive
   - Write output to return buffer
   - Return CompletedExecution result
         ↓
7. Bootloader receives result:
   - Success: returndata available
   - Revert: empty returndata, gas burned
         ↓
8. EVM Interpreter resumes with result
```

**Code Reference**: `/system_hooks/src/lib.rs:174-188` (`HooksStorage::try_intercept`)

**Hook Registration**: Precompiles are registered during system initialization:
```rust
impl HooksStorage<S, A> {
    pub fn add_precompiles(&mut self) {
        // Standard Ethereum precompiles
        self.add_precompile::<Secp256k1ECRecover, ...>(0x0001);
        self.add_precompile::<Sha256, ...>(0x0002);
        // ... more precompiles

        // Extended precompiles (feature-gated)
        #[cfg(feature = "point_eval_precompile")]
        self.add_precompile::<PointEvaluation, ...>(0x000a);

        #[cfg(feature = "p256_precompile")]
        self.add_precompile::<P256Verify, ...>(0x0100);
    }
}
```

**Code Reference**: `/system_hooks/src/lib.rs:205-250`

### Gas Costs

Precompile gas costs follow Ethereum specifications with some modifications:

**Gas Accounting**:
```rust
// Example: Identity precompile
const ID_STATIC_COST_ERGS: Ergs = Ergs(15 * ERGS_PER_GAS);  // 15 gas
const ID_WORD_COST_ERGS: Ergs = Ergs(3 * ERGS_PER_GAS);     // 3 gas per 32-byte word

fn calculate_gas(input_len: usize) -> Ergs {
    ID_STATIC_COST_ERGS + ID_WORD_COST_ERGS * ((input_len + 31) / 32)
}
```

**Ergs**: zkSync uses "ergs" as the gas unit (1 gas = 1000 ergs in EVM context). The constant `ERGS_PER_GAS` defines the conversion.

**Gas Charging**:
1. Calculate gas cost based on operation and input size
2. Convert to ergs: `gas * ERGS_PER_GAS`
3. Charge via `resources.charge()` method
4. On insufficient gas, revert and burn all remaining gas (EVM precompile behavior)

**Gas Cost Table** (from precompile section above):
- ecrecover: 3000 gas
- SHA256: 60 + 12/word
- RIPEMD160: 600 + 120/word
- identity: 15 + 3/word
- modexp: dynamic (complexity-based)
- BN254 add: 150
- BN254 mul: 6000
- BN254 pairing: 45000 + 34000/pair
- blake2f: 1/round
- point_eval: 50000
- P256 verify: 3450

**Code Reference**:
- Gas constants: `/evm_interpreter/src/gas/` directory
- Gas charging: `/system_hooks/src/precompiles.rs:123-130`

### Native Resource Costs

In addition to EVM gas, zkSync OS tracks **native resources** representing actual computational cost in RISC-V execution:

**Double Accounting Model**:
```rust
struct Resources {
    ergs: Ergs,              // EVM gas (for compatibility)
    native: NativeResources, // Actual RISC-V cycle cost
}

// Charging both
resources.charge(&Resources::from_ergs_and_native(
    ergs_cost,
    native_cost,
))?;
```

**Native Cost Calculation**:
```rust
// Example: Identity precompile
const ID_BASE_NATIVE_COST: u64 = 20;
const ID_BYTE_NATIVE_COST: u64 = 10;

let native_cost = ID_BASE_NATIVE_COST + ID_BYTE_NATIVE_COST * input_len;
```

**Why Double Accounting?**:
- **EVM gas**: Maintains Ethereum compatibility, smart contracts see correct gas usage
- **Native resources**: Reflects actual proving cost in RISC-V, prevents DoS via cheap-gas-expensive-native operations

**Native Cost Categories**:
- **Computational**: RISC-V cycles consumed
- **Memory**: Memory allocations and expansions
- **Storage**: Storage access costs

**Native Resource Limits**: In proof mode, native resources may hit limits before EVM gas runs out, causing the transaction to fail with "out of native resources" error.

**Code Reference**:
- Resources trait: `/zk_ee/src/system/resources.rs`
- Native cost constants: `/evm_interpreter/src/native_resource_constants/`

### Flow Diagram

Complete precompile execution flow:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Smart Contract Bytecode                       │
│                                                                  │
│    PUSH1 0x20        // return size                             │
│    PUSH1 0x00        // return offset                           │
│    PUSH1 0x80        // input size                              │
│    PUSH1 0x00        // input offset                            │
│    PUSH1 0x00        // value                                   │
│    PUSH2 0x0001      // address (ecrecover)                     │
│    PUSH2 0x0BB8      // gas (3000)                              │
│    CALL               // execute precompile                      │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                      EVM Interpreter                             │
│                                                                  │
│  1. Parse CALL parameters                                       │
│  2. Check if address < 0x10000                                  │
│  3. Create ExternalCallRequest:                                 │
│     - callee_address: 0x0001                                    │
│     - input: memory[0x00:0x80] (hash, v, r, s)                 │
│     - available_resources: 3000 gas                             │
│     - modifier: Normal / Delegate / Static                      │
│  4. Trigger preemption → return to bootloader                   │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Bootloader                                │
│                                                                  │
│  1. Receive ExternalCallRequest                                 │
│  2. Extract address_low = 0x0001                                │
│  3. Call hooks_storage.try_intercept(0x0001, request, ...)      │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                      HooksStorage                                │
│                                                                  │
│  1. Look up 0x0001 in BTreeMap                                  │
│  2. Found: SystemHook(secp256k1_ecrecover_hook)                 │
│  3. Call hook function:                                         │
│     hook(request, caller_ee, system, return_memory)             │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│              pure_system_function_hook_impl                      │
│                     (Generic Wrapper)                            │
│                                                                  │
│  1. Extract calldata from request                               │
│  2. Calculate gas cost: 3000 gas                                │
│  3. Calculate native cost: ECRECOVER_NATIVE_COST                │
│  4. Charge resources:                                            │
│     resources.charge(ergs=3000*1000, native=X)?                 │
│  5. Create SliceVec output buffer                               │
│  6. Call system function:                                        │
│     F::invoke(oracle, logger, input, output, resources, alloc)  │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│              Secp256k1ECRecover System Function                  │
│                                                                  │
│  1. Parse input: hash, v, r, s                                  │
│  2. Validate signature parameters                               │
│  3. Call crypto::secp256k1::recover():                          │
│     - Compute public key from signature                         │
│     - Hash public key with Keccak256                            │
│     - Extract Ethereum address (last 20 bytes)                  │
│  4. Write address to output buffer (32 bytes, left-padded)      │
│  5. Return Ok(())                                               │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│          pure_system_function_hook_impl (continued)              │
│                                                                  │
│  Result = Ok(())                                                │
│  1. Destruct SliceVec → (returndata, remaining_memory)          │
│  2. Create CompletedExecution:                                  │
│     - resources_returned: remaining gas                         │
│     - result: CallResult::Successful                            │
│     - return_values: ReturnValues { returndata, ... }           │
│  3. Return to bootloader                                        │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Bootloader                                │
│                                                                  │
│  1. Receive CompletedExecution                                  │
│  2. Copy returndata to EVM memory                               │
│  3. Prepare to resume EVM interpreter:                          │
│     - Success: Push 1 to stack                                  │
│     - Update gas: remaining_gas                                 │
│  4. Continue execution after CALL                               │
└────────────────┬────────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                    EVM Interpreter (Resumed)                     │
│                                                                  │
│  1. Read return value from stack: 1 (success)                   │
│  2. Continue executing bytecode                                 │
│  3. Access returndata at memory[0x00:0x20] = address            │
└─────────────────────────────────────────────────────────────────┘
```

**Key Insights**:
- Precompile execution uses the preemption model (doesn't recurse into interpreter)
- All parameters flow through `ExternalCallRequest` struct
- System functions are generic implementations wrapped by hooks
- Resources (gas + native) charged before execution
- Errors burn remaining gas (EVM behavior)

**Code References**:
- CALL opcode: `/evm_interpreter/src/opcodes/call.rs`
- Preemption: `/zk_ee/src/system/execution_environment/mod.rs`
- Hooks: `/system_hooks/src/lib.rs`
- ecrecover: `/crypto/src/secp256k1/recover.rs`

---

## Proof System Crypto

Cryptography plays a crucial role in zkSync OS's proof generation and verification pipeline.

### Role in Witness Generation

The witness (execution trace) must include all non-deterministic cryptographic operations:

**Cryptographic Operations Recorded in Witness**:
1. **Oracle Queries**: All callable oracle queries (KZG, modexp advice, hash-to-prime)
2. **Random Number Generation**: Any RNG used in proof system (must be deterministic)
3. **Hash Operations**: All hash function invocations (for Merkle proofs, commitments)
4. **Signature Verifications**: Public key recoveries and signature checks

**Witness Format**:
```
Vec<u32> containing:
  [query_id, param_count, param1_lo, param1_hi, ...,
   result_count, result1_lo, result1_hi, ...]
```

**Recording Mechanism** (`/oracle_provider/src/lib.rs:294-334`):
```rust
pub struct ReadWitnessSource<M: MemorySource> {
    original_source: ZkEENonDeterminismSource<M>,
    read_items: Rc<RefCell<Vec<u32>>>,  // Witness accumulator
}

impl<M: MemorySource> NonDeterminismCSRSource<M> for ReadWitnessSource<M> {
    fn read(&mut self) -> u32 {
        let item = self.original_source.read();
        self.read_items.borrow_mut().push(item);  // Record in witness
        item
    }
}
```

**Witness Generation Flow**:
```
1. Initialize ReadWitnessSource wrapping oracle processors
         ↓
2. Execute RISC-V binary with witness source
         ↓
3. All CSR reads intercepted and recorded
         ↓
4. Cryptographic operations → callable oracles → CSR reads
         ↓
5. Witness vector grows with each oracle query
         ↓
6. Final witness: Vec<u32> ready for prover
```

**Determinism Requirement**: All cryptographic operations must be deterministic:
- Same inputs → same witness → same proof
- No sources of randomness except oracle responses
- Replay during proving produces identical execution

**Code Reference**: `/oracle_provider/src/lib.rs`, `/zksync_os_runner/src/lib.rs`

### Verification

**Proof Verification Stages**:

1. **L1 Verification Contract**: On-chain zkSNARK verification
   - Verifies BN254 pairing equations
   - Checks public inputs (state root, block hash)
   - Uses ecpairing precompile (0x08) for verification

2. **State Commitment Verification**: Merkle proofs verify state transitions
   - Keccak256 hashes for Merkle tree nodes
   - SHA256 for Bitcoin-compatible proofs

3. **Signature Verification**: Transaction authenticity
   - ecrecover (0x01) for Ethereum transactions
   - P256 verify (0x0100) for WebAuthn transactions

**On-Chain Verification**:
```solidity
// Pseudo-code for L1 verification contract
function verifyProof(
    uint256[] calldata proof,
    uint256[] calldata publicInputs
) external view returns (bool) {
    // Prepare pairing check inputs
    (G1Point[] memory g1s, G2Point[] memory g2s) = prepareInputs(proof);

    // Call BN254 pairing precompile at 0x08
    bytes memory input = encodePairingInput(g1s, g2s);
    (bool success, bytes memory output) = address(0x08).staticcall(input);

    require(success, "Pairing failed");
    return abi.decode(output, (bool));
}
```

**Code Reference**: Verification contracts in zkSync Era L1 contracts (external to this repository)

### KZG Commitments for Data Availability

**EIP-4844 Integration**:

zkSync OS supports EIP-4844 blob transactions for scalable data availability:

**Commitment Generation** (via callable oracle):
```
1. Transaction data → Blob (4096 field elements)
         ↓
2. Polynomial interpolation P(X) from blob
         ↓
3. KZG commitment C = [P(τ)]₁ (using trusted setup)
         ↓
4. Versioned hash = Keccak256(commitment)[0] || Blake2s(commitment)[1:32]
         ↓
5. Challenge z = Blake2s(versioned_hash || data)[0:128 bits]
         ↓
6. KZG proof π at point z: proves P(z) = y
         ↓
7. Return (C, π) to RISC-V
```

**On-Chain Verification** (point evaluation precompile 0x000a):
```
Verify: e(C - [y], [1]) = e(π, [τ] - [z])

Where:
  - C: commitment point
  - y: claimed evaluation
  - π: opening proof
  - [τ]: trusted setup point
  - [z]: challenge point
```

**Blob Format**:
- Size: 4096 field elements × 32 bytes = 128 KB
- Encoding: 31 bytes of data per field element (1 byte always 0)
- Maximum data per blob: ~127 KB

**DA Commitment Pipeline**:
```
Transaction data
      ↓
Blob encoding (31-byte chunks)
      ↓
BlobKZGCommitmentQuery (callable oracle)
      ↓
KZG commitment + proof
      ↓
Point evaluation precompile verification
      ↓
Verified commitment published to L1
```

**Feature Flag**: KZG operations require `point_eval_precompile` feature enabled

**Code References**:
- KZG oracle: `/callable_oracles/src/blob_kzg_commitment/mod.rs`
- Point eval precompile: `/system_hooks/src/lib.rs:239-242`
- DA commitment generator: `/basic_system/src/system_implementation/system/da_commitment_generator/`

---

## Build Profiles and Precompiles

zkSync OS uses Cargo feature flags to control which cryptographic precompiles are enabled in different build configurations.

### Build Profiles Overview

**Profile Definitions** (`/proof_running_system/Cargo.toml:48-61`):

```toml
[features]
# Production: Single-block batches with P256 support
production = [
    "basic_bootloader/eip-7702",
    "system_hooks/p256_precompile"
]

# Testing: All precompiles enabled
for_tests = [
    "production",
    "state-diffs-pi",
    "cycle_marker",
    "system_hooks/point_eval_precompile"
]

# Ethereum runner: Mainnet replay
eth_runner = [
    "basic_bootloader/disable_system_contracts",
    "zk_ee/prevrandao",
    "state-diffs-pi",
    "unlimited_native",
    "basic_bootloader/burn_base_fee"
]

# EVM tester: Maximum compatibility
evm_tester = [
    "basic_bootloader/resources_for_tester",
    "basic_bootloader/disable_system_contracts",
    "zk_ee/prevrandao",
    "state-diffs-pi",
    "basic_bootloader/eip-7702",
    "system_hooks/p256_precompile",
    "system_hooks/point_eval_precompile",
    "basic_bootloader/burn_base_fee",
    "system_hooks/mock-unsupported-precompiles",
    "unlimited_native"
]
```

### Production Profile

**Features**: `proving`, `production`

**Enabled Precompiles**:
- ✅ Standard Ethereum precompiles (0x01-0x08)
  - ecrecover (0x01)
  - SHA256 (0x02)
  - RIPEMD160 (0x03)
  - identity (0x04)
  - modexp (0x05)
  - BN254 add (0x06)
  - BN254 mul (0x07)
  - BN254 pairing (0x08)
- ✅ P256 verify (0x0100) - WebAuthn support
- ❌ Blake2f (0x09) - Mock only
- ❌ Point evaluation (0x000a) - Disabled

**Build Command**:
```bash
cd zksync_os
./dump_bin.sh --type production
# Output: singleblock_batch.bin
```

**Use Case**: Production zkSync Era deployment
- Single-block batches
- P256 for WebAuthn/Passkey support
- Optimized for proof generation

**Binary Output**: `/zksync_os/singleblock_batch.bin`

### for_tests Profile

**Features**: `proving`, `for_tests`

**Enabled Precompiles**:
- ✅ All production precompiles
- ✅ Point evaluation (0x000a) - KZG verification
- ✅ Cycle markers for benchmarking

**Build Command**:
```bash
cd zksync_os
./dump_bin.sh --type for-tests
# Output: for_tests.bin
```

**Use Case**: Development and testing
- Full precompile support for comprehensive testing
- KZG operations for EIP-4844 testing
- Performance profiling via cycle markers

**Binary Output**: `/zksync_os/for_tests.bin`

**Additional Features**:
- `state-diffs-pi`: Include state diffs in public input
- `cycle_marker`: Enable performance profiling

### eth_runner Profile

**Features**: `proving`, `eth_runner`

**Enabled Precompiles**:
- ✅ Standard Ethereum precompiles (0x01-0x08)
- ❌ P256 - Not needed for Ethereum mainnet replay
- ❌ Point evaluation - Pre-EIP-4844 blocks

**Build Command**:
```bash
cd zksync_os
./dump_bin.sh --type eth-runner
# Output: evm_replay.bin
```

**Use Case**: Ethereum mainnet block replay
- Replays historical Ethereum blocks
- No zkSync-specific precompiles
- Validates EVM compatibility

**Special Features**:
- `disable_system_contracts`: No zkSync system contracts
- `prevrandao`: Support for PREVRANDAO opcode (post-Merge)
- `burn_base_fee`: Proper base fee handling
- `unlimited_native`: No native resource limits for testing

**Binary Output**: `/zksync_os/evm_replay.bin`

### evm_tester Profile

**Features**: `proving`, `evm_tester`

**Enabled Precompiles**:
- ✅ All standard precompiles
- ✅ P256 verify
- ✅ Point evaluation
- ✅ Mock implementations for unsupported precompiles

**Build Command**:
```bash
cd zksync_os
./dump_bin.sh --type evm-tester
# Output: evm_tester.bin
```

**Use Case**: EVM test suite execution (e.g., Ethereum tests)
- Maximum compatibility
- All precompiles available
- Mock implementations for edge cases

**Special Features**:
- `mock-unsupported-precompiles`: Mock for Blake2f, etc.
- `resources_for_tester`: Adjusted resource limits
- All compatibility features enabled

**Binary Output**: `/zksync_os/evm_tester.bin`

### Additional Profiles

#### multiblock-batch

**Features**: `proving`, `production`, `multiblock-batch`

**Purpose**: Process multiple blocks in a single proof

**Build Command**:
```bash
cd zksync_os
./dump_bin.sh --type multiblock-batch
# Output: multiblock_batch.bin
```

**Use Case**: Batch multiple blocks for amortized proof cost

#### benchmarking

**Features**: `proving`, `eth_runner`, `benchmarking`

**Purpose**: Performance profiling with detailed cycle counts

**Build Command**:
```bash
cd zksync_os
./dump_bin.sh --type benchmarking
# Output: evm_replay.bin (with profiling)
```

**Use Case**: Analyze performance bottlenecks in RISC-V execution

### How to Enable/Disable Precompiles

**Method 1: Cargo Features** (for library integration):
```toml
[dependencies]
proof_running_system = { path = "../proof_running_system", features = ["p256_precompile"] }
system_hooks = { path = "../system_hooks", features = ["point_eval_precompile"] }
```

**Method 2: Build Script** (for RISC-V binaries):
```bash
# Edit dump_bin.sh to add/remove features
./dump_bin.sh --type custom-profile
```

**Method 3: Cargo Command Line**:
```bash
cd zksync_os
cargo build --release --features "proving,production,p256_precompile"
```

**Conditional Compilation** (`/system_hooks/src/lib.rs`):
```rust
impl HooksStorage {
    pub fn add_precompiles(&mut self) {
        // Always enabled
        self.add_precompile::<Secp256k1ECRecover, _>(0x0001);

        // Conditionally enabled
        #[cfg(feature = "p256_precompile")]
        self.add_precompile::<P256Verify, _>(0x0100);

        #[cfg(feature = "point_eval_precompile")]
        self.add_precompile::<PointEvaluation, _>(0x000a);

        // Mock if unsupported precompiles feature enabled
        #[cfg(feature = "mock-unsupported-precompiles")]
        self.add_precompile::<Blake2f, _>(0x0009);
    }
}
```

### Precompile Feature Flags Summary

| Feature Flag | Precompiles Affected | Enabled In |
|--------------|---------------------|------------|
| `p256_precompile` | P256 verify (0x0100) | production, for_tests, evm_tester |
| `point_eval_precompile` | Point evaluation (0x000a) | for_tests, evm_tester |
| `mock-unsupported-precompiles` | Blake2f (0x09), mocked precompiles | evm_tester |

**Default (no features)**: Only standard Ethereum precompiles 0x01-0x08 enabled

**Code References**:
- Feature definitions: `/proof_running_system/Cargo.toml:48-61`
- Build script: `/zksync_os/dump_bin.sh`
- Conditional compilation: `/system_hooks/src/lib.rs:205-250`

---

## Summary

The zkSync OS cryptographic system provides:

1. **Comprehensive Primitives**: Hash functions (Blake2, SHA256, Keccak256, RIPEMD160), digital signatures (secp256k1, P256), elliptic curves (BN254, BLS12-381)

2. **EVM Compatibility**: Full suite of Ethereum precompiles (0x01-0x09) plus extended precompiles for modern cryptographic standards (P256, KZG)

3. **Performance Optimization**: Callable oracles delegate expensive operations (modexp, KZG) to host environment, maintaining efficiency in RISC-V proof generation

4. **Flexible Configuration**: Build profiles enable/disable precompiles based on deployment requirements (production, testing, Ethereum replay)

5. **Proof System Integration**: All cryptographic operations recorded in witness for deterministic proof generation and verification

This layered architecture enables zkSync OS to achieve both high performance and full verifiability while maintaining Ethereum compatibility and supporting modern cryptographic standards like WebAuthn and EIP-4844 blob transactions.

**Key Takeaways**:
- Standard EVM precompiles: Always available for Ethereum compatibility
- P256 precompile: Enabled in production for WebAuthn/Passkey support
- KZG operations: Available in testing profiles for EIP-4844 development
- Callable oracles: Essential for efficient proof generation
- Double resource accounting: Prevents DoS via gas/native cost discrepancies

For detailed execution flow, see [DataFlow.md](./DataFlow.md). For system integration, see [SystemLayer.md](./SystemLayer.md).
