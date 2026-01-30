//! ZisK Keccak-f1600 precompile implementation.
//!
//! Routes Keccak-f permutations to ZisK's native syscall (CSR 0x800).
//! When the guest writes the address of a 200-byte Keccak state buffer to CSR 0x800,
//! ZisK's execution environment performs the Keccak-f1600 permutation in the host
//! and updates the state in-place. This generates a native ZisK Keccak proof table
//! entry, making it proving-friendly compared to software computation.

use super::AlignedState;

/// Execute Keccak-f1600 via ZisK's native precompile.
///
/// # Safety Considerations
///
/// The `AlignedState` contains 31 u64 words but ZisK's syscall expects 25.
/// This is safe because:
/// - The first 25 words ARE the Keccak state
/// - Words 25-30 are scratch space used only by keccak_special5 (RV32)
/// - The 256-byte alignment exceeds ZisK's 64-bit requirement
#[inline(always)]
pub(crate) fn keccak_f1600(state: &mut AlignedState) {
    // SAFETY: AlignedState.0 has 31 u64 words, first 25 are the Keccak state.
    // The pointer cast is valid because [u64; 31] starts at the same address
    // as its first 25 elements, and ZisK only reads/writes those 25 words.
    unsafe {
        let state_ptr: *mut [u64; 25] = state.0.as_mut_ptr() as *mut [u64; 25];
        syscall_keccak_f(state_ptr);
    }
}

/// Execute Keccak-f1600 permutation via ZisK syscall (CSR 0x800).
///
/// The syscall takes the address of a 1600-bit (200 byte) state and performs
/// the Keccak-f permutation in-place.
#[inline(always)]
unsafe fn syscall_keccak_f(state: *mut [u64; 25]) {
    core::arch::asm!(
        "csrs 0x800, {value}",
        value = in(reg) state,
    );
}
