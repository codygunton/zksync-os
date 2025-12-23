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
pub use recover::recover;

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
/// to EIP-2 should have an S value less than or equal to this.
///
/// `57896044618658097711785492504343953926418782139537452191302581570759080747168`
pub const SECP256K1N_HALF: [u8; 32] = [
    0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D, 0xDF, 0xE9, 0x2F, 0x46, 0x68, 0x1B, 0x20, 0xA0,
];

pub const SECP256K1N_HALF_U256: ruint::aliases::U256 =
    ruint::aliases::U256::from_be_bytes(SECP256K1N_HALF);

impl Display for Secp256k1Err {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OperationOverflow => write!(f, "secp256k1: restoring x-coordinate overflowed"),
            Self::InvalidParams => write!(
                f,
                "secp256k1: could not decompress signature to curve point"
            ),
            Self::RecoveredInfinity => write!(f, "secp256k1: recovered the point at infinity"),
        }
    }
}

#[cfg(feature = "secp256k1-static-context")]
pub fn ecrecover_test() {
    use crate::k256::{
        ecdsa::{hazmat::bits2field, SigningKey},
        elliptic_curve::{group::GroupEncoding, ops::Reduce},
        Scalar,
    };
    use crate::sha3::{Digest, Keccak256};
    let message = "In the beginning the Universe was created.
    This had made many people very angry and has been widely regarded as a bad move";
    let private_key = SigningKey::from_bytes(
        &[
            136, 84, 181, 46, 13, 86, 203, 113, 63, 17, 137, 177, 95, 211, 104, 70, 112, 232, 200,
            156, 225, 27, 123, 207, 243, 114, 4, 216, 148, 242, 81, 154,
        ]
        .into(),
    )
    .unwrap();
    let digest = {
        let mut hasher = Keccak256::new();
        hasher.update(message);
        let res = hasher.finalize();
        let mut hash_bytes = [0u8; 32];
        hash_bytes.copy_from_slice(&res);
        hash_bytes
    };

    let public_key = private_key.verifying_key().as_affine();

    let (signature, recovery_id) = private_key.sign_prehash_recoverable(&digest).unwrap();
    let msg = <Scalar as Reduce<crate::k256::U256>>::reduce_bytes(
        &bits2field::<crate::k256::Secp256k1>(&digest)
            .map_err(|_| ())
            .unwrap(),
    );

    let recovered_key = recover(&msg, &signature, &recovery_id).unwrap();

    assert_eq!(recovered_key.to_bytes(), public_key.to_bytes());
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn run_outside() {
        ecrecover_test();
    }
}
