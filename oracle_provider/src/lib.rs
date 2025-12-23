#![allow(clippy::bool_comparison)]
#![allow(clippy::precedence)]
#![allow(clippy::len_zero)]

// Hook zk_ee IOOracle to be NonDeterminismCSRSource

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use zk_ee::oracle::query_ids::{DISCONNECT_ORACLE_QUERY_ID, UART_QUERY_ID};

/// Thread-local storage for the guest's current program counter.
/// Simulators should set this before making oracle calls so we can track
/// which code location triggers each query.
static CURRENT_GUEST_PC: AtomicU64 = AtomicU64::new(0);

/// Set the current guest PC (called by simulator before oracle operations)
pub fn set_guest_pc(pc: u64) {
    CURRENT_GUEST_PC.store(pc, Ordering::Relaxed);
}

/// Get the current guest PC (called internally when logging queries)
fn get_guest_pc() -> u64 {
    CURRENT_GUEST_PC.load(Ordering::Relaxed)
}

/// Query ID names for logging
fn query_id_name(id: u32) -> String {
    match id {
        UART_QUERY_ID => "UART".to_string(),
        DISCONNECT_ORACLE_QUERY_ID => "DISCONNECT".to_string(),
        0x40030000 => "INITIAL_STORAGE_SLOT".to_string(),
        0x40030001 => "STORAGE_READ".to_string(),
        0x4002f000 => "FLAT_STORAGE_PREIMAGE".to_string(),
        0x40040001 => "MERKLE_PATH".to_string(),
        0x40040002 => "MERKLE_PATH_PUBDATA".to_string(),
        0x4004f001 => "FLAT_PREVIOUS_INDEX".to_string(),
        0x4004f002 => "FLAT_EXACT_INDEX".to_string(),
        0x40070000 => "ZK_PROOF_DATA_INIT".to_string(),
        0x40070001 => "ZK_PROOF_DATA_INIT_OLD".to_string(),
        0x40070002 => "PROOF_FOR_INDEX".to_string(),
        0x40050001 => "TX_DATA".to_string(),
        0x40050002 => "TX_SIZE".to_string(),
        0x40060000 => "NEXT_TX_SIZE".to_string(),
        0x40060001 => "BLOCK_METADATA".to_string(),
        0x40060002 => "BLOCK_HASHES".to_string(),
        0x40060003 => "TX_CONTENT".to_string(),
        id => format!("0x{:08x}", id),
    }
}

/// Environment variable to enable verbose query logging
fn verbose_query_logging() -> bool {
    std::env::var("VERBOSE_ORACLE").is_ok()
}

/// Decode a B160 address from usize values (3 usizes = 24 bytes, first 20 are address)
fn decode_address_from_usizes(data: &[usize]) -> Option<String> {
    if data.len() < 3 {
        return None;
    }
    // B160 is stored as 3 u64 limbs in little-endian order (lowest limb first)
    // We need to reconstruct the big-endian address representation
    let mut bytes = [0u8; 24];
    // Store in reverse order to get big-endian representation
    bytes[16..24].copy_from_slice(&data[0].to_be_bytes());
    bytes[8..16].copy_from_slice(&data[1].to_be_bytes());
    bytes[0..8].copy_from_slice(&data[2].to_be_bytes());
    // Take last 20 bytes for the address (skip first 4 bytes of padding)
    Some(format!("0x{}", hex::encode(&bytes[4..24])))
}

/// Decode a Bytes32 (storage key or hash) from usize values (4 usizes = 32 bytes)
fn decode_bytes32_from_usizes(data: &[usize]) -> Option<String> {
    if data.len() < 4 {
        return None;
    }
    // Bytes32 is stored as 4 u64 limbs in little-endian order (lowest limb first)
    // We need to reconstruct the big-endian representation
    let mut bytes = [0u8; 32];
    bytes[24..32].copy_from_slice(&data[0].to_be_bytes());
    bytes[16..24].copy_from_slice(&data[1].to_be_bytes());
    bytes[8..16].copy_from_slice(&data[2].to_be_bytes());
    bytes[0..8].copy_from_slice(&data[3].to_be_bytes());
    Some(format!("0x{}", hex::encode(bytes)))
}

/// Decode a StorageAddress (address + key) from usize values
fn decode_storage_address(data: &[usize]) -> Option<(String, String)> {
    if data.len() < 7 {
        return None;
    }
    let address = decode_address_from_usizes(&data[0..3])?;
    let key = decode_bytes32_from_usizes(&data[3..7])?;
    Some((address, key))
}
use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{internal_error, oracle::IOOracle};

pub use risc_v_simulator::abstractions::memory::MemorySource;
use risc_v_simulator::abstractions::non_determinism::NonDeterminismCSRSource;

pub struct DummyMemorySource;

impl MemorySource for DummyMemorySource {
    fn get(
        &self,
        _phys_address: u64,
        _access_type: risc_v_simulator::abstractions::memory::AccessType,
        _trap: &mut risc_v_simulator::cycle::status_registers::TrapReason,
    ) -> u32 {
        unreachable!()
    }
    fn set(
        &mut self,
        _phys_address: u64,
        _value: u32,
        _access_type: risc_v_simulator::abstractions::memory::AccessType,
        _trap: &mut risc_v_simulator::cycle::status_registers::TrapReason,
    ) {
        unreachable!()
    }
}

///
/// Structure that is responsible for buffering incoming queries till the end,
/// and then dispatching them to various responders. When constructed it checks
/// that responders do not try to serve the same query ID.
pub struct ZkEENonDeterminismSource<M: MemorySource> {
    query_buffer: Option<QueryBuffer>,
    current_query_id: Option<u32>,
    current_iterator: Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>>,
    iterator_len_to_indicate: Option<u32>,
    high_half: Option<u32>,
    is_connected_to_external_oracle: bool,
    /// Flag to ignore the next write(0) from CSRRW side effect after response is complete.
    /// CSRRW always writes, so after reading the last response value, we get a spurious write(0).
    ignore_next_zero_write: bool,
    /// Vector of different processors that are responsible for handling queries.
    processors: Vec<Box<dyn OracleQueryProcessor<M> + 'static>>,
    /// Mapping from query_id to processor that is handling it (represented as index in processors vector above).
    ranges: BTreeMap<u32, usize>,
    /// Count of oracle queries made (total).
    query_count: usize,
    /// Count of queries by query_id for analysis.
    query_counts: BTreeMap<u32, usize>,
    /// Current transaction index (inferred from NEXT_TX_SIZE queries)
    current_tx_index: usize,
    /// Detailed query log for debugging
    query_log: Vec<QueryLogEntry>,
}

/// Log entry for detailed query analysis
#[derive(Clone, Debug)]
pub struct QueryLogEntry {
    pub query_num: usize,
    pub tx_index: usize,
    pub query_id: u32,
    pub query_name: String,
    pub input_len: usize,
    pub response_len: usize,
    /// First few words of input for context
    pub input_preview: Vec<usize>,
    /// First few words of response for context (especially useful for NEXT_TX_SIZE)
    pub response_preview: Vec<usize>,
    /// Guest program counter when query was made (0 if not available)
    pub guest_pc: u64,
}

impl<M: MemorySource> Default for ZkEENonDeterminismSource<M> {
    fn default() -> Self {
        Self {
            query_buffer: None,
            current_query_id: None,
            current_iterator: None,
            iterator_len_to_indicate: None,
            high_half: None,
            is_connected_to_external_oracle: false,
            ignore_next_zero_write: false,
            processors: Vec::new(),
            ranges: BTreeMap::new(),
            query_count: 0,
            query_counts: BTreeMap::new(),
            current_tx_index: 0,
            query_log: Vec::new(),
        }
    }
}

impl<M: MemorySource> ZkEENonDeterminismSource<M> {
    /// Returns the total number of oracle queries made.
    pub fn query_count(&self) -> usize {
        self.query_count
    }

    /// Returns the query counts by query type.
    pub fn get_query_counts(&self) -> &BTreeMap<u32, usize> {
        &self.query_counts
    }

    /// Prints a summary of all oracle queries made.
    pub fn print_query_summary(&self) {
        eprintln!("\n[ORACLE] Query breakdown by type:");
        let mut total = 0usize;
        let mut non_uart = 0usize;
        for (qid, count) in &self.query_counts {
            total += count;
            if *qid != UART_QUERY_ID {
                non_uart += count;
            }
            eprintln!("  {}: {} queries", query_id_name(*qid), count);
        }
        eprintln!("[ORACLE] Total queries: {} (non-UART: {})", total, non_uart);
        eprintln!("[ORACLE] Transactions processed: {}", self.current_tx_index);
    }

    /// Returns the detailed query log
    pub fn get_query_log(&self) -> &[QueryLogEntry] {
        &self.query_log
    }

    /// Prints detailed query log (for debugging)
    pub fn print_detailed_log(&self) {
        eprintln!("\n[ORACLE] Detailed query log ({} queries):", self.query_log.len());
        for entry in &self.query_log {
            eprintln!(
                "  [{}] tx={} {} input_len={} response_len={} input={:x?}",
                entry.query_num,
                entry.tx_index,
                entry.query_name,
                entry.input_len,
                entry.response_len,
                entry.input_preview
            );
        }
    }

    /// Prints queries grouped by transaction
    pub fn print_queries_by_tx(&self) {
        eprintln!("\n[ORACLE] Queries by transaction:");
        let mut current_tx = 0;
        let mut tx_queries: BTreeMap<String, usize> = BTreeMap::new();

        for entry in &self.query_log {
            if entry.tx_index != current_tx {
                // Print summary for previous tx
                if !tx_queries.is_empty() {
                    eprintln!("  TX {}: {:?}", current_tx, tx_queries);
                }
                current_tx = entry.tx_index;
                tx_queries.clear();
            }
            *tx_queries.entry(entry.query_name.clone()).or_insert(0) += 1;
        }
        // Print last tx
        if !tx_queries.is_empty() {
            eprintln!("  TX {}: {:?}", current_tx, tx_queries);
        }
    }

    /// Dumps detailed query log to a file for comparison/diffing
    pub fn dump_query_log_to_file(&self, path: &str) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;

        writeln!(file, "# Oracle Query Log")?;
        writeln!(file, "# Total queries: {}", self.query_count)?;
        writeln!(file, "# Transactions: {}", self.current_tx_index)?;
        writeln!(file, "#")?;
        writeln!(
            file,
            "# Format: query_num,tx_index,query_name,input_len,response_len,input_preview"
        )?;
        writeln!(file, "#")?;

        for entry in &self.query_log {
            writeln!(
                file,
                "{},{},{},{},{},{:x?}",
                entry.query_num,
                entry.tx_index,
                entry.query_name,
                entry.input_len,
                entry.response_len,
                entry.input_preview
            )?;
        }

        // Also write per-tx summary at the end
        writeln!(file, "\n# Per-transaction summary:")?;
        let mut current_tx = 0;
        let mut tx_queries: BTreeMap<String, usize> = BTreeMap::new();

        for entry in &self.query_log {
            if entry.tx_index != current_tx {
                if !tx_queries.is_empty() {
                    writeln!(file, "# TX {}: {:?}", current_tx, tx_queries)?;
                }
                current_tx = entry.tx_index;
                tx_queries.clear();
            }
            *tx_queries.entry(entry.query_name.clone()).or_insert(0) += 1;
        }
        if !tx_queries.is_empty() {
            writeln!(file, "# TX {}: {:?}", current_tx, tx_queries)?;
        }

        Ok(())
    }

    #[track_caller]
    pub fn add_external_processor<P: OracleQueryProcessor<M> + 'static>(&mut self, processor: P) {
        let query_ids = processor.supported_query_ids();
        let processor_id = self.processors.len();
        for id in query_ids.into_iter() {
            let existing = self.ranges.insert(id, processor_id);
            assert!(existing.is_none(), "more than one processor for query id 0x{id:08x}");
        }
        self.processors.push(Box::new(processor));
        self.is_connected_to_external_oracle = true;
    }

    fn process_buffered_query(&mut self, memory: &M) {
        assert!(self.current_iterator.is_none());
        assert!(self.current_query_id.is_none());

        let buffer = self.query_buffer.take().expect("must exist");
        let query_id = buffer.query_type;

        // Track query counts
        self.query_count += 1;
        *self.query_counts.entry(query_id).or_insert(0) += 1;

        // Track transaction boundaries (NEXT_TX_SIZE signals start of new tx processing)
        if query_id == 0x40060000 {
            // NEXT_TX_SIZE query indicates we're starting to process a new transaction
            self.current_tx_index += 1;
        }

        if query_id == DISCONNECT_ORACLE_QUERY_ID {
            // Print query summary on disconnect
            self.print_query_summary();
            // Print detailed log if verbose mode
            if verbose_query_logging() {
                self.print_queries_by_tx();
            }
            self.is_connected_to_external_oracle = false;
        } else {
            let input_buffer = buffer.buffer;
            let input_len = input_buffer.len();
            let input_preview: Vec<usize> = input_buffer.iter().take(4).copied().collect();

            // Log detailed query info for storage queries (always, to help debug divergence)
            // INITIAL_STORAGE_SLOT = 0x40030000
            if query_id == 0x40030000 {
                if let Some((address, key)) = decode_storage_address(&input_buffer) {
                    eprintln!(
                        "[oracle] INITIAL_STORAGE_SLOT tx={} address={} key={}",
                        self.current_tx_index, address, key
                    );
                }
            }
            // FLAT_STORAGE_PREIMAGE = 0x4002f000
            if query_id == 0x4002f000 {
                if let Some(hash) = decode_bytes32_from_usizes(&input_buffer) {
                    eprintln!(
                        "[oracle] FLAT_STORAGE_PREIMAGE tx={} hash={}",
                        self.current_tx_index, hash
                    );
                }
            }

            let Some(processor_id) = self.ranges.get(&query_id).copied() else {
                panic!("Can not process query with ID = 0x{query_id:08x}");
            };
            let processor = &mut self.processors[processor_id];
            let new_iterator = processor.process_buffered_query(query_id, input_buffer, memory);

            // Collect iterator to peek at response values, then wrap back as iterator
            let response_vec: Vec<usize> = new_iterator.collect();
            let response_preview: Vec<usize> = response_vec.iter().take(4).copied().collect();
            let result_len = response_vec.len() * 2; // NOTE for mismatch of 32/64-bit archs

            // Log query details
            let query_name = query_id_name(query_id);
            let guest_pc = get_guest_pc();

            // Special logging for NEXT_TX_SIZE to show the transaction size
            if query_id == 0x40060000 && !response_preview.is_empty() {
                let tx_size = response_preview[0] as u32;
                eprintln!(
                    "[oracle] NEXT_TX_SIZE tx={} size={} bytes{}",
                    self.current_tx_index,
                    tx_size,
                    if tx_size == 0 { " (END OF BLOCK)" } else { "" }
                );
            }

            // Log query completion (skip UART queries as they're noisy and content is printed by guest)
            if verbose_query_logging() && query_id != UART_QUERY_ID {
                eprintln!(
                    "[oracle] query {} complete, processing... tx={} input_len={} response_len={} (u64s={}) response={:?}",
                    query_name, self.current_tx_index, input_len, result_len, response_vec.len(), response_preview
                );
            }

            // Store in detailed log
            self.query_log.push(QueryLogEntry {
                query_num: self.query_count,
                tx_index: self.current_tx_index,
                query_id,
                query_name: query_name.clone(),
                input_len,
                response_len: result_len,
                input_preview,
                response_preview,
                guest_pc,
            });

            self.iterator_len_to_indicate = Some(result_len as u32);
            if result_len > 0 {
                self.current_query_id = Some(query_id);
                self.current_iterator = Some(Box::new(response_vec.into_iter()));
            }
        }
    }

    /// Reads the next 32bits.
    /// Our iterators and queues hold usize elements (u64), so we have to do some splitting and caching.
    fn read_impl(&mut self) -> u32 {
        // We mocked reads, so it's filtered out before
        if self.is_connected_to_external_oracle == false {
            return 0;
        }

        if let Some(iterator_len_to_indicate) = self.iterator_len_to_indicate.take() {
            // If there's no iterator (empty response), set flag to ignore the spurious write(0)
            if self.current_iterator.is_none() {
                self.ignore_next_zero_write = true;
            }
            return iterator_len_to_indicate;
        }

        // This is the 32 bits remaining from the previous item - return them now.
        if let Some(high) = self.high_half.take() {
            // If this was the last value (iterator already consumed), set flag to ignore
            // the spurious write(0) from csrrw that follows this read.
            if self.current_iterator.is_none() {
                self.ignore_next_zero_write = true;
            }
            return high;
        }
        // If we didn't have any partial data left, we should fetch another element from the iterator.
        let Some(current_iterator) = self.current_iterator.as_mut() else {
            panic!("trying to read, but data is not prepared");
        };
        let next = current_iterator.next().expect("must contain next element");
        if current_iterator.len() == 0 {
            // we are done - there are no more elements left after this one.
            self.current_query_id = None;
            self.current_iterator = None;
        }
        // Split the 64 bits into 2 pieces - one is put into 'high' field, to be returned later
        // and the other one is returned immediately.
        let high = (next >> 32) as u32;
        let low = next as u32;
        self.high_half = Some(high);

        low
    }

    fn write_impl(&mut self, memory: &M, value: u32) {
        // Debug: log writes only when verbose oracle is enabled
        // if value != 0 && verbose_query_logging() {
        //     static WRITE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        //     let count = WRITE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        //     // eprintln!("[ORACLE DEBUG] write #{}: 0x{:08x}", count, value);
        // }

        // CSRRW instruction always writes to CSR, even when "reading".
        // When the guest does `csrrw rd, 0x7c0, x0` to read, it also writes x0=0.
        // We need to ignore these spurious write(0) operations when we're in the
        // middle of returning a response (indicated by any of these being set).
        if value == 0
            && (self.iterator_len_to_indicate.is_some()
                || self.current_iterator.is_some()
                || self.high_half.is_some()
                || self.ignore_next_zero_write)
        {
            // Ignore spurious write(0) from csrrw read operation
            self.ignore_next_zero_write = false;
            return;
        }
        self.ignore_next_zero_write = false;

        if self.current_query_id.is_some() {
            println!(
                "Current query ID = 0x{:08x} iterator is not consumed in full, but received value 0x{:08x}",
                self.current_query_id.unwrap(),
                value
            );
            self.current_query_id = None;
        }

        // may have something from remains
        if self.current_iterator.is_some() {
            if self.current_iterator.as_ref().unwrap().len() != 0 {
                println!(
                    "Current iterator is not consumed in full, but received value 0x{value:08x}"
                );
            }
            self.current_iterator = None;
        }
        if self.iterator_len_to_indicate.is_some() {
            self.iterator_len_to_indicate = None;
        }
        if self.high_half.is_some() {
            self.high_half = None;
        }

        if let Some(query_buffer) = self.query_buffer.as_mut() {
            let complete = query_buffer.write(value);
            if complete {
                self.process_buffered_query(memory);
            }
        } else {
            if self.is_connected_to_external_oracle == false && value != UART_QUERY_ID {
                // we are not interested in general to start another query
                return;
            }

            let new_buffer = QueryBuffer::empty_for_query_type(value);
            self.query_buffer = Some(new_buffer);
        }
    }
}

impl IOOracle for ZkEENonDeterminismSource<DummyMemorySource> {
    type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
        &'a mut self,
        query_type: u32,
        input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        if query_type == DISCONNECT_ORACLE_QUERY_ID {
            self.is_connected_to_external_oracle = false;
        }
        if self.is_connected_to_external_oracle == false {
            return Ok(Box::new([].into_iter()));
        }
        let Some(processor) = self.ranges.get(&query_type).copied() else {
            return Err(internal_error!("invalid query ID"));
        };
        let processor = &mut self.processors[processor];
        let response = processor.process_buffered_query(
            query_type,
            UsizeSerializable::iter(input).collect::<Vec<usize>>(),
            &DummyMemorySource,
        );

        Ok(response)
    }
}

pub trait OracleQueryProcessor<M: MemorySource> {
    /// List of different query ids that are supported (for example NextTxSize or BlockLevelMetadataIterator).
    fn supported_query_ids(&self) -> Vec<u32>;
    fn supports_query_id(&self, query_id: u32) -> bool {
        self.supported_query_ids().contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static>;
}

struct QueryBuffer {
    query_type: u32,
    remaining_len: Option<usize>,
    write_low: bool,
    buffer: Vec<usize>,
}

impl QueryBuffer {
    fn empty_for_query_type(query_type: u32) -> Self {
        Self { query_type, remaining_len: None, write_low: true, buffer: Vec::new() }
    }

    fn write(&mut self, value: u32) -> bool {
        // NOTE: we have to match between 32 bit inner env and 64 bit outer env
        if let Some(remaining_len) = self.remaining_len.as_mut() {
            // println!("Writing word 0x{:08x} for query ID = 0x{:08x}", value, self.query_type);
            if self.write_low {
                self.buffer.push(value as usize);
                self.write_low = false;
            } else {
                let last = self.buffer.last_mut().unwrap();
                *last |= (value as usize) << 32;
                self.write_low = true;
            }
            *remaining_len -= 1;

            *remaining_len == 0
        } else {
            // println!("Expecting {} words for query ID = 0x{:08x}", value, self.query_type);
            self.remaining_len = Some(value as usize);
            if value == 0 {
                // nothing else to expect
                true
            } else {
                false
            }
        }
    }
}

// Now we hook an access
impl<M: MemorySource> NonDeterminismCSRSource<M> for ZkEENonDeterminismSource<M> {
    #[allow(clippy::let_and_return)]
    fn read(&mut self) -> u32 {
        let value = self.read_impl();
        // println!("`NonDeterminismCSRSource` returned 0x{:08x}", value);
        value
    }

    fn write_with_memory_access(&mut self, memory: &M, value: u32) {
        // println!("`NonDeterminismCSRSource` received 0x{:08x}", value);
        self.write_impl(memory, value);
    }
}

/// Wraps the original source and remembers all the read accesses.
pub struct ReadWitnessSource<M: MemorySource> {
    original_source: ZkEENonDeterminismSource<M>,
    read_items: Rc<RefCell<Vec<u32>>>,
}

impl<M: MemorySource> ReadWitnessSource<M> {
    pub fn new(original_source: ZkEENonDeterminismSource<M>) -> Self {
        Self { original_source, read_items: Rc::new(RefCell::new(vec![])) }
    }

    pub fn get_read_items(&self) -> Rc<RefCell<Vec<u32>>> {
        self.read_items.clone()
    }
}

impl<M: MemorySource> NonDeterminismCSRSource<M> for ReadWitnessSource<M> {
    fn read(&mut self) -> u32 {
        let item = self.original_source.read();
        // On read - remember the items.
        self.read_items.borrow_mut().push(item);
        item
    }

    fn write_with_memory_access(&mut self, memory: &M, value: u32) {
        self.original_source.write_with_memory_access(memory, value);
    }
}
