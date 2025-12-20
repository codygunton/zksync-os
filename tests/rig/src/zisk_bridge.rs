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
//! The protocol works as:
//! 1. Write query ID (high nibble 0x4)
//! 2. Read length (single u32, number of u32s to follow)
//! 3. Read data (pairs of u32 combined into u64)
//!
//! This bridge tracks state to handle the length specially.

use oracle_provider::{DummyMemorySource, ZkEENonDeterminismSource};
use risc_v_simulator::abstractions::non_determinism::NonDeterminismCSRSource;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Cached check for ZISK_QUIET env var (suppresses per-operation logging)
fn is_quiet() -> bool {
    static QUIET: OnceLock<bool> = OnceLock::new();
    *QUIET.get_or_init(|| std::env::var("ZISK_QUIET").is_ok())
}

/// Counter for oracle operations (for debugging)
static ORACLE_OP_COUNT: AtomicU64 = AtomicU64::new(0);

/// Counter for query operations (for debugging query boundaries)
static QUERY_OP_COUNT: AtomicU64 = AtomicU64::new(0);

/// Environment variable to enable verbose bridge logging
fn verbose_bridge() -> bool {
    static VERBOSE: OnceLock<bool> = OnceLock::new();
    *VERBOSE.get_or_init(|| std::env::var("VERBOSE_BRIDGE").is_ok())
}

/// Bridge between ZkEENonDeterminismSource and Zisk's oracle callback interface.
///
/// This struct wraps the oracle and captures all reads into a witness vector.
/// It tracks protocol state to correctly convert between 32-bit and 64-bit:
/// - Length indicator: returned as-is (single u32 zero-extended to u64)
/// - Data: pairs of u32 combined into u64
///
/// # Safety
/// This bridge MUST only be used from a single thread. The `Send` impl is only
/// safe because witness generation runs on a single thread.
pub struct ZiskOracleBridge {
    oracle: UnsafeCell<ZkEENonDeterminismSource<DummyMemorySource>>,
    witness: Arc<Mutex<Vec<u32>>>,
    /// How many u32 values remain to read for current query response.
    /// 0 means next read is a length indicator.
    remaining_u32s: AtomicU32,
    /// Flag to ignore the next write(0) after response is fully consumed.
    /// This handles the spurious CSRRW write that follows the last data read.
    ignore_next_zero_write: AtomicU32,
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
            remaining_u32s: AtomicU32::new(0),
            ignore_next_zero_write: AtomicU32::new(0),
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

    /// Read a u64 value, handling the protocol correctly.
    ///
    /// - If remaining_u32s == 0, this is a length read: read one u32, set remaining, return as u64
    /// - Otherwise, read two u32s and combine into u64, decrement remaining by 2
    ///
    /// # Safety
    /// Must only be called from a single thread.
    fn read_u64(&self) -> u64 {
        let remaining_before = self.remaining_u32s.load(Ordering::SeqCst);

        if remaining_before == 0 {
            // This is the length indicator - a single u32 value
            // Return the raw u32 count - the guest code divides by 2 itself!
            // See proof_running_system/src/io_oracle/mod.rs line 110
            let length = self.read() as u64;
            // Track the raw u32 count internally for our read counting
            self.remaining_u32s.store(length as u32, Ordering::SeqCst);
            if verbose_bridge() {
                let query_num = QUERY_OP_COUNT.load(Ordering::Relaxed);
                eprintln!(
                    "[bridge] query={} read_u64 LENGTH: {} (will track {} u32 reads)",
                    query_num, length, length
                );
            }
            length
        } else {
            // The guest code does two CSR reads to get (low, high) u32 values.
            // See proof_running_system/src/io_oracle/mod.rs lines 35-37
            // So we just return ONE u32 (as u64) per read - the guest combines them.
            let value = self.read() as u64;
            let new_remaining = remaining_before.saturating_sub(1);
            self.remaining_u32s.store(new_remaining, Ordering::SeqCst);
            // If we just consumed the last data, set flag to ignore the next spurious write(0)
            if new_remaining == 0 {
                self.ignore_next_zero_write.store(1, Ordering::SeqCst);
            }
            if verbose_bridge() && remaining_before <= 8 {
                // Log last few data reads to see values at end of response
                let query_num = QUERY_OP_COUNT.load(Ordering::Relaxed);
                eprintln!(
                    "[bridge] query={} read_u64 DATA: remaining {} -> {}, value=0x{:08x}",
                    query_num, remaining_before, new_remaining, value
                );
            }
            value
        }
    }

    /// Write a u64 value, splitting into two u32s if needed.
    ///
    /// Also resets the remaining counter since a write starts a new query.
    ///
    /// Note: We only write the high 32 bits if non-zero, because:
    /// 1. Query IDs fit in 32 bits (high=0)
    /// 2. The oracle has spurious write(0) filtering that would incorrectly
    ///    ignore a legitimate write(0) for the high bits
    ///
    /// # Safety
    /// Must only be called from a single thread.
    fn write_u64(&self, val: u64) {
        // Debug: log first 100 writes to see if UART marker appears
        static BRIDGE_WRITE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = BRIDGE_WRITE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count < 100 || val == 0xFFFFFFFF {
            eprintln!("[BRIDGE DEBUG] write_u64 #{}: 0x{:016x} (low32=0x{:08x})", count, val, val as u32);
        }

        // The CSRRW instruction writes x0=0 when reading, causing spurious writes.
        // We need to ignore write(0) when:
        // 1. We're in the middle of reading a response (remaining > 0)
        // 2. We just finished reading all data (ignore_next_zero_write flag set)
        let remaining = self.remaining_u32s.load(Ordering::SeqCst);
        let ignore_flag = self.ignore_next_zero_write.swap(0, Ordering::SeqCst);

        if val == 0 && (remaining > 0 || ignore_flag != 0) {
            if verbose_bridge() {
                eprintln!(
                    "[bridge] write_u64 IGNORED spurious write(0): remaining={} ignore_flag={}",
                    remaining, ignore_flag
                );
            }
            return;
        }

        // Track if this looks like a new query (high nibble 0x4 = basic oracle)
        let is_query_id = (val & 0xF0000000) == 0x40000000;
        if is_query_id {
            let query_num = QUERY_OP_COUNT.fetch_add(1, Ordering::Relaxed);
            if verbose_bridge() {
                eprintln!(
                    "[bridge] query={} NEW QUERY ID: 0x{:08x}",
                    query_num + 1, val
                );
            }
        }

        // A real write (query) resets the state - next read will be a length
        self.remaining_u32s.store(0, Ordering::SeqCst);

        // The 64-bit guest splits each u64 into two separate CSR writes.
        // Each write_u64 call here contains just one u32 value (zero-extended).
        // We simply forward the low 32 bits to the oracle.
        let value = val as u32;
        self.write(value);
    }

    /// Convert this bridge into a Zisk OracleCallback.
    ///
    /// Consumes self and returns a callback that can be used with the Zisk emulator.
    ///
    /// The oracle protocol uses 32-bit values, but Zisk is 64-bit. This callback:
    /// - For the length indicator (first read after query): returns single u32 / 2 as u64
    /// - For data reads: combines pairs of u32 into u64
    #[cfg(feature = "zisk-witness")]
    pub fn into_callback(self) -> ziskemu::OracleCallback {
        use ziskemu::OracleOp;

        let bridge = Arc::new(self);

        Arc::new(Mutex::new(move |op: OracleOp| -> u64 {
            match op {
                OracleOp::Read => bridge.read_u64(),
                OracleOp::Write(val) => {
                    bridge.write_u64(val);
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
