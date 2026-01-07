#![allow(long_running_const_eval)]
#![allow(clippy::precedence)]

mod context;
mod field;
mod points;
mod recover;
mod scalars;

#[cfg(test)]
mod test_vectors;

use core::fmt::Debug;
use core::fmt::Display;

pub use context::ECMultContext;
pub use recover::recover_with_context;
pub use recover::recover_with_logging;

#[cfg(feature = "secp256k1-static-context")]
pub use context::ECRECOVER_CONTEXT;

#[cfg(feature = "secp256k1-static-context")]
pub use recover::recover;

// #[cfg(target_arch = "riscv32")]
pub use scalars::Scalar;
// #[cfg(target_arch = "riscv32")]
pub use field::FieldElement;
// #[cfg(target_arch = "riscv32")]
pub use points::{Affine, Jacobian};
// #[cfg(target_arch = "riscv32")]
pub use recover::ecmult;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    scalars::init();
    field::init();
}

/// CSR-based QuasiUART for RISC-V guests (both Airbender and Zisk)
/// Uses CSR 0x7c0 with the QuasiUART protocol for unified logging.
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
pub mod uart_log {
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
        if len == 0 {
            return;
        }
        csr_write_word(HELLO_MARKER as usize);
        csr_write_word(len.next_multiple_of(4) / 4 + 1);
        csr_write_word(len);
        let bytes = s.as_bytes();
        let mut i = 0;
        while i + 4 <= len {
            let word = u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
            csr_write_word(word as usize);
            i += 4;
        }
        if i < len {
            let mut buf = [0u8; 4];
            for j in 0..(len - i) {
                buf[j] = bytes[i + j];
            }
            csr_write_word(u32::from_le_bytes(buf) as usize);
        }
    }

    #[inline(never)]
    pub fn write_str(s: &str) {
        unsafe { let _ = LINE_BUF.try_push_str(s); }
    }

    #[inline(never)]
    pub fn write_usize(mut val: usize) {
        if val == 0 {
            unsafe { let _ = LINE_BUF.try_push('0'); }
            return;
        }
        let mut digits = [0u8; 20];
        let mut i = 0;
        while val > 0 {
            digits[i] = (val % 10) as u8;
            val /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            unsafe { let _ = LINE_BUF.try_push((b'0' + digits[i]) as char); }
        }
    }

    #[inline(never)]
    pub fn write_hex_byte(byte: u8) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        unsafe {
            let _ = LINE_BUF.try_push(HEX[(byte >> 4) as usize] as char);
            let _ = LINE_BUF.try_push(HEX[(byte & 0xf) as usize] as char);
        }
    }

    #[inline(never)]
    pub fn write_hex_slice(bytes: &[u8]) {
        write_str("0x");
        for b in bytes { write_hex_byte(*b); }
    }

    #[inline(never)]
    pub fn newline() {
        unsafe {
            let _ = LINE_BUF.try_push('\n');
            flush_buffer(&LINE_BUF);
            LINE_BUF.clear();
        }
    }
}

#[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
pub mod uart_log {
    #[inline(always)] pub fn write_str(_: &str) {}
    #[inline(always)] pub fn write_usize(_: usize) {}
    #[inline(always)] pub fn write_hex_byte(_: u8) {}
    #[inline(always)] pub fn write_hex_slice(_: &[u8]) {}
    #[inline(always)] pub fn newline() {}
}

#[derive(Debug, PartialEq)]
pub enum Secp256k1Err {
    OperationOverflow,
    InvalidParams,
    RecoveredInfinity,
}

/// The order of the secp256k1 curve, divided by two. Signatures that should be checked according
/// to a BIP-0062 / EIP-2 rule should verify that the 's' component is at most this value.
pub const SECP256K1_N_DIV2: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
];
