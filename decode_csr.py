#!/usr/bin/env python3
"""Decode RISC-V CSR instructions from zksync-os binary."""

# Register ABI names
REG_NAMES = {
    0: "zero", 1: "ra", 2: "sp", 3: "gp", 4: "tp",
    5: "t0", 6: "t1", 7: "t2", 8: "s0/fp", 9: "s1",
    10: "a0", 11: "a1", 12: "a2", 13: "a3", 14: "a4",
    15: "a5", 16: "a6", 17: "a7", 18: "s2", 19: "s3",
    20: "s4", 21: "s5", 22: "s6", 23: "s7", 24: "s8",
    25: "s9", 26: "s10", 27: "s11", 28: "t3", 29: "t4",
    30: "t5", 31: "t6"
}

# CSR funct3 opcodes
FUNCT3_NAMES = {
    1: "CSRRW",   # Read/Write
    2: "CSRRS",   # Read/Set
    3: "CSRRC",   # Read/Clear
    5: "CSRRWI",  # Read/Write Immediate
    6: "CSRRSI",  # Read/Set Immediate
    7: "CSRRCI"   # Read/Clear Immediate
}

# Known custom CSRs in zksync-os
CSR_NAMES = {
    0x7c0: "ORACLE_IO",      # Oracle read/write for witness data
    0x7c7: "BLAKE2_DELEG",   # BLAKE2s round delegation
    0x7ca: "BIGINT_DELEG",   # U256 bigint delegation
    0xc00: "cycle",          # Standard: cycle counter
}

def decode_csr_instruction(insn_hex):
    """Decode a CSR instruction and return human-readable string."""
    insn = int(insn_hex, 16)

    opcode = insn & 0x7f
    rd = (insn >> 7) & 0x1f
    funct3 = (insn >> 12) & 0x7
    rs1 = (insn >> 15) & 0x1f
    csr = (insn >> 20) & 0xfff

    if opcode != 0x73:
        return f"{insn_hex}: NOT A SYSTEM INSTRUCTION (opcode=0x{opcode:02x})"

    funct3_name = FUNCT3_NAMES.get(funct3, f"UNKNOWN({funct3})")
    csr_name = CSR_NAMES.get(csr, f"0x{csr:03x}")
    rd_name = REG_NAMES.get(rd, f"x{rd}")
    rs1_name = REG_NAMES.get(rs1, f"x{rs1}")

    # Determine operation semantics
    if funct3 in [1, 2, 3]:  # Register-based CSR ops
        if funct3 == 1:  # CSRRW
            if rd == 0:
                # rd=x0 means we don't care about old value -> pure write
                operation = f"CSR[{csr_name}] := {rs1_name}"
                if rs1 == 0:
                    operation = f"CSR[{csr_name}] := 0  (clear/trigger)"
            elif rs1 == 0:
                # rs1=x0 means we write 0 -> pure read
                operation = f"{rd_name} := CSR[{csr_name}]  (read)"
            else:
                operation = f"{rd_name} := CSR[{csr_name}]; CSR[{csr_name}] := {rs1_name}"
        elif funct3 == 2:  # CSRRS
            if rs1 == 0:
                operation = f"{rd_name} := CSR[{csr_name}]  (read-only)"
            else:
                operation = f"{rd_name} := CSR[{csr_name}]; CSR[{csr_name}] |= {rs1_name}"
        elif funct3 == 3:  # CSRRC
            if rs1 == 0:
                operation = f"{rd_name} := CSR[{csr_name}]  (read-only)"
            else:
                operation = f"{rd_name} := CSR[{csr_name}]; CSR[{csr_name}] &= ~{rs1_name}"
    else:
        operation = f"immediate-based CSR op"

    # Add semantic meaning for known CSRs
    semantic = ""
    if csr == 0x7c0:
        if rd == 0 and rs1 != 0:
            semantic = " → ORACLE WRITE (send to prover)"
        elif rd != 0 and rs1 == 0:
            semantic = " → ORACLE READ (get from prover)"
        elif rd == 0 and rs1 == 0:
            semantic = " → ORACLE NOP/SYNC"
    elif csr == 0x7c7:
        semantic = " → BLAKE2s ROUND DELEGATION"
    elif csr == 0x7ca:
        semantic = " → BIGINT DELEGATION (U256 op)"
    elif csr == 0xc00:
        semantic = " → READ CYCLE COUNTER"

    asm = f"{funct3_name.lower()} {rd_name}, {csr_name}, {rs1_name}"

    return f"{insn_hex}: {asm:40} | {operation}{semantic}"


def main():
    instructions = [
        "0x7c0010f3", "0x7c0012f3", "0x7c001373", "0x7c0013f3",
        "0x7c0014f3", "0x7c001573", "0x7c0015f3", "0x7c001673",
        "0x7c0016f3", "0x7c001773", "0x7c0017f3", "0x7c001873",
        "0x7c0018f3", "0x7c001973", "0x7c0019f3", "0x7c001a73",
        "0x7c001af3", "0x7c001b73", "0x7c001bf3", "0x7c001c73",
        "0x7c001cf3", "0x7c001d73", "0x7c001df3", "0x7c001e73",
        "0x7c001ef3", "0x7c001f73", "0x7c001ff3", "0x7c049073",
        "0x7c051073", "0x7c059073", "0x7c061073", "0x7c069073",
        "0x7c071073", "0x7c079073", "0x7c081073", "0x7c091073",
        "0x7c0a1073", "0x7c0a9073", "0x7c0d9073", "0x7c701073",
        "0x7ca01073",
    ]

    print("=" * 100)
    print("RISC-V CSR Instruction Decoder for zksync-os")
    print("=" * 100)
    print()

    # Group by CSR
    by_csr = {}
    for insn in instructions:
        csr = (int(insn, 16) >> 20) & 0xfff
        by_csr.setdefault(csr, []).append(insn)

    for csr in sorted(by_csr.keys()):
        csr_name = CSR_NAMES.get(csr, f"0x{csr:03x}")
        print(f"\n### CSR {csr_name} (0x{csr:03x}) ###\n")
        for insn in by_csr[csr]:
            print(decode_csr_instruction(insn))

    print("\n" + "=" * 100)
    print("SUMMARY")
    print("=" * 100)
    print(f"""
CSR 0x7c0 (ORACLE_IO):
  - Used for oracle/witness communication with the ZK prover
  - Write: Send data to prover (e.g., query parameters)
  - Read: Receive data from prover (e.g., storage values, witness)

CSR 0x7c7 (BLAKE2_DELEG):
  - Triggers delegation of BLAKE2s round computation
  - Used for hashing (e.g., transaction hashes, state roots)

CSR 0x7ca (BIGINT_DELEG):
  - Triggers delegation of U256 arithmetic to specialized circuits
  - More efficient than computing 256-bit ops instruction-by-instruction
  - Operations: ADD, SUB, MUL_LOW, MUL_HIGH, EQ

Register Convention (a0-a7 = x10-x17):
  - a0 (x10): Often pointer to first operand / result
  - a1 (x11): Often pointer to second operand
  - a2 (x12): Often operation mask / flags
""")


if __name__ == "__main__":
    main()
