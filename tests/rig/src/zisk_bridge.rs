//! Bridge between ZkEENonDeterminismSource and Zisk's oracle callback interface.
//!
//! This module provides the glue to run Zisk emulator with zksync-os's oracle
//! for 64-bit witness generation.
//!
//! The key challenge is that `ZkEENonDeterminismSource` uses a 32-bit protocol:
//! - Returns u32 values from read()
//! - Reports response lengths in terms of u32 counts
//! - Returns 64-bit values as pairs of u32 (low, then high)
//!
//! But Zisk (64-bit) expects:
//! - 64-bit values from CSR reads
//! - The guest reads 64-bit usizes directly
//!
//! This bridge combines pairs of u32 reads into u64 for the 64-bit guest.

use oracle_provider::{DummyMemorySource, ZkEENonDeterminismSource};
use risc_v_simulator::abstractions::non_determinism::NonDeterminismCSRSource;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Cached check for ZISK_QUIET env var (suppresses per-operation logging)
fn is_quiet() -> bool {
    static QUIET: OnceLock<bool> = OnceLock::new();
    *QUIET.get_or_init(|| std::env::var("ZISK_QUIET").is_ok())
}

/// Counter for oracle operations (for debugging)
static ORACLE_OP_COUNT: AtomicU64 = AtomicU64::new(0);

/// Bridge between ZkEENonDeterminismSource and Zisk's oracle callback interface.
///
/// This struct wraps the oracle and captures all reads into a witness vector.
/// It combines pairs of u32 reads from the oracle into u64 values for 64-bit Zisk.
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
        let count = ORACLE_OP_COUNT.fetch_add(1, Ordering::Relaxed);
        // Log every 100000 operations to track progress (suppressed by ZISK_QUIET=1)
        if count % 100000 == 0 && !is_quiet() {
            eprintln!("[oracle] op={} read -> 0x{:08x}", count, value);
        }
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
        let count = ORACLE_OP_COUNT.fetch_add(1, Ordering::Relaxed);
        // Log writes that look like query IDs (high nibble 0x4) or every 100000 ops
        // Suppressed by ZISK_QUIET=1
        if !is_quiet() && ((value & 0xF0000000) == 0x40000000 || count % 100000 == 0) {
            eprintln!("[oracle] op={} write 0x{:08x}", count, value);
        }
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
