  1. RV32IM CPU Execution

  What: Execute RISC-V instructions (base integer + multiply/divide)
  Airbender reference: risc_v_simulator/src/cycle/state.rs and state_new.rs
  - fn cycle(&mut self) - main execution loop
  - Opcode decoding in cycle/opcode_formats/*.rs

  2. Memory Subsystem
zksync-airbender/risc_v_simulator/src/abstractions/memory.rs
  What: ROM (2MB at 0x0) + RAM (rest up to 1GB)
  - trait MemorySource - get/set interface
  Airbender implementations: risc_v_simulator/src/abstractions/memory.rs and prover/src/tracer.rs
  - VectorMemoryImpl - simple implementation

  3. CSR Interception for Oracle (0x7c0)

  What: When CPU hits csrrw to CSR 0x7c0, route to oracle
  Airbender reference: risc_v_simulator/src/cycle/state.rs:1171-1240 and state_new.rs:894-940
  match csr_number {
      NON_DETERMINISM_CSR => {
          // read: call non_determinism_source.read()
          // write: call non_determinism_source.write_with_memory_access()
      }
  }

  4. NonDeterminismCSRSource Trait Implementation

  What: The oracle interface that connects CSR to query handlers
  Airbender reference: risc_v_simulator/src/abstractions/non_determinism.rs:2-12
  pub trait NonDeterminismCSRSource<M: MemorySource> {
      fn read(&mut self) -> u32;
      fn write_with_memory_access(&mut self, memory: &M, value: u32);
  }

  5. Oracle Query Dispatcher

  What: Parse queries from CSR writes, dispatch to handlers, buffer responses
  zksync-os reference (reusable): oracle_provider/src/lib.rs
  - ZkEENonDeterminismSource - implements the dispatcher
  - You can reuse this - just wire it to your CSR handler

  6. Query Handlers

  What: Actually answer queries (storage, tx data, preimages, etc.)
  zksync-os reference (reusable): forward_system/src/run/query_processors/*.rs
  - TxDataResponder - tx queries
  - ReadStorageResponder - storage queries
  - GenericPreimageResponder - preimages
  - etc.

  You can reuse all of these - they implement OracleQueryProcessor trait.

  7. Error Exit Detection (CSR 0xc00)

  What: Write to cycle CSR = abort
  Airbender reference: risc_v_simulator/src/cycle/state.rs - look for 0xc00 handling
  riscv_common reference: riscv_common/src/lib.rs:37-45 (guest side)

  8. Success Exit Detection

  What: Detect infinite loop, read x10-x17 as output
  Airbender reference: riscv_common/src/lib.rs:54-107 shows the guest side
  Your implementation: Track PC, if it loops to itself, read registers 10-17

  ---
  Summary: Build vs Reuse

  | Component               | Build or Reuse | Location                                 |
  |-------------------------|----------------|------------------------------------------|
  | RV32IM CPU              | Build          | -                                        |
  | Memory                  | Build          | -                                        |
  | CSR 0x7c0 interception  | Build          | risc_v_simulator/src/cycle/state.rs:1171 |
  | CSR 0xc00 error exit    | Build          | -                                        |
  | Infinite loop detection | Build          | -                                        |
  | Oracle dispatcher       | Reuse          | oracle_provider/src/lib.rs               |
  | Query handlers          | Reuse          | forward_system/src/run/query_processors/ |
  | Advice handlers         | Reuse          | callable_oracles/src/                    |


  ● If ZisK wants to reuse the existing oracle infrastructure from zksync-os, here are the traits:

  Required Traits (from Airbender's risc_v_simulator)

  1. MemorySource

  Location: risc_v_simulator/src/abstractions/memory.rs
  pub trait MemorySource {
      fn get(&self, phys_address: u64, access_type: AccessType, trap: &mut TrapReason) -> u32;
      fn set(&mut self, phys_address: u64, value: u32, access_type: AccessType, trap: &mut TrapReason);
  }
  Why: Query handlers need to read guest memory (e.g., modexp reads operands from guest RAM)

  2. NonDeterminismCSRSource<M: MemorySource>

  Location: risc_v_simulator/src/abstractions/non_determinism.rs
  pub trait NonDeterminismCSRSource<M: MemorySource> {
      fn read(&mut self) -> u32;
      fn write_with_memory_access(&mut self, memory: &M, value: u32);
  }
  Why: Connects your CSR handler to the oracle dispatcher

  ---
  That's It


  ● If ZisK wants to reuse the existing oracle infrastructure from zksync-os, here are the traits:

  Required Traits (from Airbender's risc_v_simulator)

  1. MemorySource

  Location: risc_v_simulator/src/abstractions/memory.rs
  pub trait MemorySource {
      fn get(&self, phys_address: u64, access_type: AccessType, trap: &mut TrapReason) -> u32;
      fn set(&mut self, phys_address: u64, value: u32, access_type: AccessType, trap: &mut TrapReason);
  }
  Why: Query handlers need to read guest memory (e.g., modexp reads operands from guest RAM)

  2. NonDeterminismCSRSource<M: MemorySource>

  Location: risc_v_simulator/src/abstractions/non_determinism.rs
  pub trait NonDeterminismCSRSource<M: MemorySource> {
      fn read(&mut self) -> u32;
      fn write_with_memory_access(&mut self, memory: &M, value: u32);
  }
  Why: Connects your CSR handler to the oracle dispatcher
