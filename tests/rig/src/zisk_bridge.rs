//! Bridge between ZkEENonDeterminismSource and Zisk's oracle callback interface.
//!
//! This module provides the glue to run Zisk emulator with zksync-os's oracle
//! for 64-bit witness generation.

use oracle_provider::{DummyMemorySource, ZkEENonDeterminismSource};
use risc_v_simulator::abstractions::non_determinism::NonDeterminismCSRSource;
use std::cell::UnsafeCell;
use std::sync::{Arc, Mutex};

/// Bridge between ZkEENonDeterminismSource and Zisk's oracle callback interface.
///
/// This struct wraps the oracle and captures all reads into a witness vector.
/// It uses `UnsafeCell` for interior mutability because the oracle is not Send
/// but we know the execution is single-threaded.
///
/// # Safety
/// This bridge MUST only be used from a single thread. The `Send` impl is only
/// safe because witness generation runs on a single thread.
pub struct ZiskOracleBridge {
    oracle: UnsafeCell<ZkEENonDeterminismSource<DummyMemorySource>>,
    witness: Arc<Mutex<Vec<u32>>>,
}

// Safety: ZiskOracleBridge is only used in single-threaded witness generation.
// The oracle is never accessed from multiple threads.
unsafe impl Send for ZiskOracleBridge {}
unsafe impl Sync for ZiskOracleBridge {}

impl ZiskOracleBridge {
    /// Create a new bridge wrapping the given oracle.
    pub fn new(oracle: ZkEENonDeterminismSource<DummyMemorySource>) -> Self {
        Self {
            oracle: UnsafeCell::new(oracle),
            witness: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get a reference to the captured witness vector.
    pub fn get_witness(&self) -> Arc<Mutex<Vec<u32>>> {
        Arc::clone(&self.witness)
    }

    /// Read from the oracle and capture the value.
    ///
    /// Called by Zisk emulator on CSR read.
    ///
    /// # Safety
    /// Must only be called from a single thread.
    pub fn read(&self) -> u32 {
        // Safety: single-threaded access guaranteed by caller
        let oracle = unsafe { &mut *self.oracle.get() };
        let value = oracle.read();
        self.witness.lock().expect("witness lock poisoned").push(value);
        value
    }

    /// Write to the oracle.
    ///
    /// Called by Zisk emulator on CSR write.
    ///
    /// # Safety
    /// Must only be called from a single thread.
    pub fn write(&self, value: u32) {
        // Safety: single-threaded access guaranteed by caller
        let oracle = unsafe { &mut *self.oracle.get() };
        oracle.write_with_memory_access(&DummyMemorySource, value);
    }

    /// Convert this bridge into a Zisk OracleCallback.
    ///
    /// Consumes self and returns a callback that can be used with the Zisk emulator.
    #[cfg(feature = "zisk-witness")]
    pub fn into_callback(self) -> ziskemu::OracleCallback {
        use ziskemu::OracleOp;

        let bridge = Arc::new(self);

        Arc::new(Mutex::new(move |op: OracleOp| -> u64 {
            match op {
                OracleOp::Read => bridge.read() as u64,
                OracleOp::Write(val) => {
                    bridge.write(val as u32);
                    0
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_captures_reads() {
        // Create an empty oracle (will panic on actual use, but we can test the structure)
        let oracle = ZkEENonDeterminismSource::default();
        let bridge = ZiskOracleBridge::new(oracle);

        let witness_ref = bridge.get_witness();

        // Check that witness starts empty
        assert!(witness_ref.lock().unwrap().is_empty());
    }
}
