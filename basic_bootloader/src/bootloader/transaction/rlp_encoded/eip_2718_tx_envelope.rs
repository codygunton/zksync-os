use crate::bootloader::{
    errors::{InvalidTransaction, TxError},
    transaction::rlp_encoded::{
        rlp::{
            apply_list_concatenation_encoding_to_hash,
            minimal_rlp_parser::{Rlp, RlpListDecode},
        },
        transaction_types::EthereumTxType,
    },
};
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

/// CSR-based QuasiUART for RISC-V guests
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
mod uart_log {
    use arrayvec::ArrayString;
    const HELLO_MARKER: u32 = u32::MAX;
    const BUF_SIZE: usize = 512;
    static mut LINE_BUF: ArrayString<BUF_SIZE> = ArrayString::new_const();

    #[inline(always)]
    fn csr_write_word(word: usize) {
        unsafe {
            core::arch::asm!(
                "csrrw x0, 0x7c0, {rd}",
                rd = in(reg) word,
                options(nomem, nostack, preserves_flags)
            )
        }
    }

    fn flush_buffer(s: &str) {
        let len = s.len();
        if len == 0 { return; }
        csr_write_word(HELLO_MARKER as usize);
        csr_write_word(len.next_multiple_of(4) / 4 + 1);
        csr_write_word(len);
        let bytes = s.as_bytes();
        let mut i = 0;
        while i + 4 <= len {
            let word = u32::from_le_bytes([bytes[i], bytes[i+1], bytes[i+2], bytes[i+3]]);
            csr_write_word(word as usize);
            i += 4;
        }
        if i < len {
            let mut buf = [0u8; 4];
            for j in 0..(len - i) { buf[j] = bytes[i + j]; }
            csr_write_word(u32::from_le_bytes(buf) as usize);
        }
    }

    #[inline(never)] pub fn write_str(s: &str) { unsafe { let _ = LINE_BUF.try_push_str(s); } }
    #[inline(never)] pub fn write_u8(val: u8) {
        if val >= 100 { unsafe { let _ = LINE_BUF.try_push((b'0' + val / 100) as char); } }
        if val >= 10 { unsafe { let _ = LINE_BUF.try_push((b'0' + (val / 10) % 10) as char); } }
        unsafe { let _ = LINE_BUF.try_push((b'0' + val % 10) as char); }
    }
    #[inline(never)] pub fn write_usize(mut val: usize) {
        if val == 0 { unsafe { let _ = LINE_BUF.try_push('0'); } return; }
        let mut digits = [0u8; 20]; let mut i = 0;
        while val > 0 { digits[i] = (val % 10) as u8; val /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { let _ = LINE_BUF.try_push((b'0' + digits[i]) as char); } }
    }
    #[inline(never)] pub fn write_hex_byte(byte: u8) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        unsafe { let _ = LINE_BUF.try_push(HEX[(byte >> 4) as usize] as char);
                 let _ = LINE_BUF.try_push(HEX[(byte & 0xf) as usize] as char); }
    }
    #[inline(never)] pub fn write_hex_slice(bytes: &[u8]) {
        write_str("0x"); for b in bytes { write_hex_byte(*b); }
    }
    #[inline(never)] pub fn newline() { unsafe { let _ = LINE_BUF.try_push('\n'); flush_buffer(&LINE_BUF); LINE_BUF.clear(); } }
}

#[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
mod uart_log {
    #[inline(always)] pub fn write_str(_: &str) {}
    #[inline(always)] pub fn write_u8(_: u8) {}
    #[inline(always)] pub fn write_usize(_: usize) {}
    #[inline(always)] pub fn write_hex_byte(_: u8) {}
    #[inline(always)] pub fn write_hex_slice(_: &[u8]) {}
    #[inline(always)] pub fn newline() {}
}

/// Parser for typed EIP-2718 transactions where the payload (P) and signature
/// are encoded as two consecutive list items inside a single outer list:
/// outer = [ payload_list(P), signature_list(yParity, r, s) ]
pub(crate) struct EIP2718PayloadParser<'a, P: RlpListDecode<'a> + EthereumTxType> {
    _marker: core::marker::PhantomData<&'a P>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP2718SignatureData<'a> {
    pub(crate) y_parity: bool,
    pub(crate) r: &'a [u8],
    pub(crate) s: &'a [u8],
}

impl<'a> RlpListDecode<'a> for EIP2718SignatureData<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let y_parity = r.bool()?;
        let r_bytes = r.bytes()?;
        let s = r.bytes()?;
        if r_bytes.len() + s.len() > 64 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let new = Self {
            y_parity,
            r: r_bytes,
            s,
        };
        Ok(new)
    }
}

impl<'a, P: RlpListDecode<'a> + EthereumTxType> EIP2718PayloadParser<'a, P> {
    /// Will try to parse P, and the try to parse signature manually
    /// NOTE: double hashing is inevitable, as signature is verified upon keccak256(0x01 || rlp([chainId, nonce, gasPrice, gasLimit, to, value, data, accessList])),
    /// while for indexing purposes divergence starts at the very start as RLP pre-encodes total length
    pub(crate) fn try_parse_and_hash_for_signature_verification(
        src: &'a [u8],
    ) -> Result<(P, EIP2718SignatureData<'a>, Bytes32), TxError> {
        let mut outer = Rlp::new(src);
        // Strip the list encoding
        let mut inner = outer.list()?;
        // Outer list must be fully consumed
        if !outer.is_empty() {
            return Err(InvalidTransaction::InvalidStructure.into());
        }
        // Take mark to include payload for hashing
        let mark = inner.mark();
        // Parse payload part (transaction fields without signature)
        let payload = P::decode_list_body(&mut inner)?;
        let inner_slice = inner.consumed_since(mark);

        // Parse signature suffix [yParity, r, s] from same parser
        let sig = EIP2718SignatureData::decode_list_body(&mut inner)?;

        if !inner.is_empty() {
            return Err(InvalidTransaction::InvalidStructure.into());
        }

        // Sanity test: hash empty input - should be 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
        {
            let test_hasher = crypto::sha3::Keccak256::new();
            let test_hash: [u8; 32] = test_hasher.finalize().into();
            uart_log::write_str("[eip2718] SANITY empty_hash=0x");
            for b in &test_hash[..8] {
                uart_log::write_hex_byte(*b);
            }
            uart_log::newline();
        }

        // Test: hash 49 bytes in single update vs 3 separate updates
        // Using the known TX#1 input: 02 ef 01 80 84 b2 d0 5e 00 84 d1 2c fa 71 82 52 08 94 14 33 95 f3 b4 c0 d5 5e e6 76 b0 6f c6 c2 dc 7c b9 ef d9 ee 88 01 b1 26 95 9d cf 44 58 80 c0
        {
            let test_input: [u8; 49] = [
                0x02, 0xef, 0x01, 0x80, 0x84, 0xb2, 0xd0, 0x5e, 0x00, 0x84, 0xd1, 0x2c, 0xfa, 0x71, 0x82, 0x52,
                0x08, 0x94, 0x14, 0x33, 0x95, 0xf3, 0xb4, 0xc0, 0xd5, 0x5e, 0xe6, 0x76, 0xb0, 0x6f, 0xc6, 0xc2,
                0xdc, 0x7c, 0xb9, 0xef, 0xd9, 0xee, 0x88, 0x01, 0xb1, 0x26, 0x95, 0x9d, 0xcf, 0x44, 0x58, 0x80,
                0xc0,
            ];

            // Single update
            let mut h1 = crypto::sha3::Keccak256::new();
            h1.update(&test_input);
            let hash1: [u8; 32] = h1.finalize().into();

            // Three updates (1 + 1 + 47)
            let mut h2 = crypto::sha3::Keccak256::new();
            h2.update(&test_input[0..1]);   // 0x02
            h2.update(&test_input[1..2]);   // 0xef
            h2.update(&test_input[2..49]);  // remaining 47 bytes
            let hash2: [u8; 32] = h2.finalize().into();

            uart_log::write_str("[eip2718] TEST single_update=0x");
            for b in &hash1[..8] { uart_log::write_hex_byte(*b); }
            uart_log::newline();

            uart_log::write_str("[eip2718] TEST three_updates=0x");
            for b in &hash2[..8] { uart_log::write_hex_byte(*b); }
            uart_log::newline();
        }

        // Build complete hash input manually to log it
        let len32 = inner_slice.len() as u32;
        let list_hdr = if len32 < 56 { 0xc0u8 + (len32 as u8) } else { 0xf8 };

        // Total input: tx_type (1) + list_hdr (1) + inner_slice (47) = 49 bytes for TX#1
        let total_len = 1 + 1 + inner_slice.len();
        uart_log::write_str("[eip2718] hash_input total_len=");
        uart_log::write_usize(total_len);
        uart_log::newline();

        // Log complete hash input
        uart_log::write_str("[eip2718] hash_input=0x");
        uart_log::write_hex_byte(P::TX_TYPE);
        uart_log::write_hex_byte(list_hdr);
        for i in 0..inner_slice.len() {
            let b = unsafe { core::ptr::read_volatile(&inner_slice[i]) };
            uart_log::write_hex_byte(b);
        }
        uart_log::newline();

        // TRACE: Only do detailed tracing for small transactions (inner_slice <= 128 bytes)
        // to avoid buffer overflows and focus on TX#1 which has the issue
        let do_detailed_trace = inner_slice.len() <= 128;

        let list_byte = if len32 < 56 { 0xc0u8 + (len32 as u8) } else { 0xf8 };

        if do_detailed_trace {
            uart_log::write_str("[eip2718] TRACE: copying inner_slice to local buffer\n");

            // Create local buffer with full hash input
            let mut local_buf = [0u8; 256];
            local_buf[0] = P::TX_TYPE;
            local_buf[1] = list_byte;
            uart_log::write_str("[eip2718] TRACE: tx_type=");
            uart_log::write_hex_byte(P::TX_TYPE);
            uart_log::write_str(" list_byte=");
            uart_log::write_hex_byte(list_byte);
            uart_log::newline();

            // Copy inner_slice byte-by-byte with volatile reads
            for i in 0..inner_slice.len() {
                let b = unsafe { core::ptr::read_volatile(&inner_slice[i]) };
                local_buf[2 + i] = b;
            }
            let local_len = 2 + inner_slice.len();

            uart_log::write_str("[eip2718] TRACE: local_buf[0..local_len]=0x");
            for i in 0..local_len {
                uart_log::write_hex_byte(local_buf[i]);
            }
            uart_log::newline();

            // Hash the local buffer in one shot
            uart_log::write_str("[eip2718] TRACE: hashing local_buf single-shot\n");
            let mut h_local = crypto::sha3::Keccak256::new();
            h_local.update(&local_buf[..local_len]);
            let hash_local: [u8; 32] = h_local.finalize().into();
            uart_log::write_str("[eip2718] TRACE: hash_local=0x");
            for b in &hash_local { uart_log::write_hex_byte(*b); }
            uart_log::newline();

            // Alternative: hash inner_slice byte-by-byte
            uart_log::write_str("[eip2718] TRACE: hashing inner_slice byte-by-byte\n");
            let mut h_bytewise = crypto::sha3::Keccak256::new();
            h_bytewise.update(&[P::TX_TYPE]);
            h_bytewise.update(&[list_byte]);
            for i in 0..inner_slice.len() {
                let b = inner_slice[i];
                h_bytewise.update(&[b]);
            }
            let hash_bytewise: [u8; 32] = h_bytewise.finalize().into();
            uart_log::write_str("[eip2718] TRACE: hash_bytewise=0x");
            for b in &hash_bytewise { uart_log::write_hex_byte(*b); }
            uart_log::newline();

            // Now do the actual hashing with separate updates
            uart_log::write_str("[eip2718] TRACE: hashing with separate updates\n");
            let mut hasher = crypto::sha3::Keccak256::new();

            uart_log::write_str("[eip2718] TRACE: update1 TX_TYPE=");
            uart_log::write_hex_byte(P::TX_TYPE);
            uart_log::newline();
            hasher.update(&[P::TX_TYPE]);

            uart_log::write_str("[eip2718] TRACE: update2 list_encoding len32=");
            uart_log::write_usize(len32 as usize);
            uart_log::newline();
            apply_list_concatenation_encoding_to_hash(len32, &mut hasher);

            // Log inner_slice details before update
            uart_log::write_str("[eip2718] TRACE: update3 inner_slice.len()=");
            uart_log::write_usize(inner_slice.len());
            uart_log::newline();

            // Log first few and last few bytes of inner_slice via direct indexing
            uart_log::write_str("[eip2718] TRACE: inner_slice[0..8]=0x");
            for i in 0..8.min(inner_slice.len()) {
                uart_log::write_hex_byte(inner_slice[i]);
            }
            uart_log::newline();
            if inner_slice.len() > 8 {
                uart_log::write_str("[eip2718] TRACE: inner_slice[last8]=0x");
                let start = inner_slice.len().saturating_sub(8);
                for i in start..inner_slice.len() {
                    uart_log::write_hex_byte(inner_slice[i]);
                }
                uart_log::newline();
            }

            // Now the actual update with inner_slice as a slice
            // First log what inner_slice actually contains when passed as slice
            uart_log::write_str("[eip2718] TRACE: inner_slice before update=0x");
            for i in 0..inner_slice.len() {
                uart_log::write_hex_byte(inner_slice[i]);
            }
            uart_log::newline();

            // Test 1: Update with inner_slice directly as a slice
            uart_log::write_str("[eip2718] TRACE: hasher direct slice update\n");
            let mut hasher_direct = crypto::sha3::Keccak256::new();
            hasher_direct.update(&[P::TX_TYPE]);
            hasher_direct.update(&[list_byte]);
            hasher_direct.update(inner_slice);
            let hash_direct: [u8; 32] = hasher_direct.finalize().into();
            uart_log::write_str("[eip2718] TRACE: hash_direct=0x");
            for b in &hash_direct { uart_log::write_hex_byte(*b); }
            uart_log::newline();

            // Test 2: Single-shot update with volatile-copied full buffer (like hash_local)
            uart_log::write_str("[eip2718] TRACE: hasher volatile single-shot\n");
            let mut full_copy = [0u8; 256];
            unsafe { core::ptr::write_volatile(&mut full_copy[0], P::TX_TYPE) };
            unsafe { core::ptr::write_volatile(&mut full_copy[1], list_byte) };
            for i in 0..inner_slice.len() {
                let b = unsafe { core::ptr::read_volatile(&inner_slice[i]) };
                unsafe { core::ptr::write_volatile(&mut full_copy[2 + i], b) };
            }
            let full_len = 2 + inner_slice.len();
            let mut hasher_full = crypto::sha3::Keccak256::new();
            hasher_full.update(&full_copy[..full_len]);
            let hash_full: [u8; 32] = hasher_full.finalize().into();
            uart_log::write_str("[eip2718] TRACE: hash_full_volatile=0x");
            for b in &hash_full { uart_log::write_hex_byte(*b); }
            uart_log::newline();

            // Test 3: Same as above but three separate updates instead of one
            uart_log::write_str("[eip2718] TRACE: hasher volatile three updates\n");
            let mut hasher_three = crypto::sha3::Keccak256::new();
            hasher_three.update(&full_copy[0..1]);  // just tx_type
            hasher_three.update(&full_copy[1..2]);  // just list_byte
            hasher_three.update(&full_copy[2..full_len]); // the data
            let hash_three: [u8; 32] = hasher_three.finalize().into();
            uart_log::write_str("[eip2718] TRACE: hash_three_volatile=0x");
            for b in &hash_three { uart_log::write_hex_byte(*b); }
            uart_log::newline();

            // Use byte-by-byte for the actual sig_hash (known to work)
            for i in 0..inner_slice.len() {
                hasher.update(&[inner_slice[i]]);
            }
            let sig_hash: Bytes32 = hasher.finalize().into();

            uart_log::write_str("[eip2718] HASH_OUT sig_hash=");
            uart_log::write_hex_slice(&sig_hash.as_u8_array());
            uart_log::newline();

            // Compare all three hashes
            uart_log::write_str("[eip2718] TRACE: local==bytewise? ");
            uart_log::write_usize((hash_local == hash_bytewise) as usize);
            uart_log::newline();
            uart_log::write_str("[eip2718] TRACE: local==sig_hash? ");
            uart_log::write_usize((hash_local == sig_hash.as_u8_array()) as usize);
            uart_log::newline();
            uart_log::write_str("[eip2718] TRACE: bytewise==sig_hash? ");
            uart_log::write_usize((hash_bytewise == sig_hash.as_u8_array()) as usize);
            uart_log::newline();

            return Ok((payload, sig, sig_hash));
        }

        // For larger transactions, just do normal hashing
        let mut hasher = crypto::sha3::Keccak256::new();
        hasher.update(&[P::TX_TYPE]);
        apply_list_concatenation_encoding_to_hash(len32, &mut hasher);
        hasher.update(inner_slice);
        let sig_hash: Bytes32 = hasher.finalize().into();

        uart_log::write_str("[eip2718] HASH_OUT sig_hash=");
        uart_log::write_hex_slice(&sig_hash.as_u8_array());
        uart_log::newline();

        Ok((payload, sig, sig_hash))
    }
}
