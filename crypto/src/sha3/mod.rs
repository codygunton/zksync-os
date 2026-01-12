// Naive (software) implementation for non-RISC-V targets
#[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
mod naive;
#[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
pub use self::naive::Keccak256;

// Delegated module: RISC-V targets or testing
// - RV32: uses keccak_special5 precompile or software simulator
// - RV64: uses zisk_keccak precompile (if feature enabled) or software simulator
#[cfg(any(
    test,
    target_arch = "riscv32",
    target_arch = "riscv64",
    feature = "testing",
    feature = "sha3_tests"
))]
pub mod delegated;

// Export Keccak256 from delegated for RISC-V targets
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
pub use self::delegated::Keccak256;

#[cfg(test)]
mod test;
