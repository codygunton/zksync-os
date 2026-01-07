#[cfg(any(
    all(any(target_arch = "riscv32", target_arch = "riscv64"), feature = "bigint_ops"),
    test,
    all(feature = "proving", fuzzing)
))]
use crate::ark_ff_delegation::MontFp;
use ark_ec::{
    bn,
    bn::{BnConfig, TwistType},
};

/// Simple UART logging for crypto debugging (RISC-V only)
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
pub mod uart {
    const HELLO_MARKER: u32 = u32::MAX;

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

    pub fn write_str(s: &str) {
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

    pub fn write_hex_u64(v: u64) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut buf = [0u8; 18]; // "0x" + 16 hex chars
        buf[0] = b'0';
        buf[1] = b'x';
        for i in 0..8 {
            let byte = ((v >> (56 - i * 8)) & 0xff) as u8;
            buf[2 + i * 2] = HEX[(byte >> 4) as usize];
            buf[2 + i * 2 + 1] = HEX[(byte & 0xf) as usize];
        }
        // Write as string
        let s = unsafe { core::str::from_utf8_unchecked(&buf) };
        write_str(s);
    }
}

#[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
pub mod uart {
    pub fn write_str(_s: &str) {}
    pub fn write_hex_u64(_v: u64) {}
}
#[cfg(not(any(
    all(any(target_arch = "riscv32", target_arch = "riscv64"), feature = "bigint_ops"),
    test,
    all(feature = "proving", fuzzing)
)))]
use ark_ff::MontFp;

use crate::bn254::fields::{Fq, Fq12Config, Fq2, Fq2Config, Fq6Config};

pub mod g1;
pub mod g2;

mod pairing_impl;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config;

impl BnConfig for Config {
    const X: &'static [u64] = &[4965661367192848881];
    /// `x` is positive.
    const X_IS_NEGATIVE: bool = false;
    const ATE_LOOP_COUNT: &'static [i8] = &[
        0, 0, 0, 1, 0, 1, 0, -1, 0, 0, -1, 0, 0, 0, 1, 0, 0, -1, 0, -1, 0, 0, 0, 1, 0, -1, 0, 0, 0,
        0, -1, 0, 0, 1, 0, -1, 0, 0, 1, 0, 0, 0, 0, 0, -1, 0, 0, -1, 0, 1, 0, -1, 0, 0, 0, -1, 0,
        -1, 0, 0, 0, 1, 0, 1, 1,
    ];

    const TWIST_MUL_BY_Q_X: Fq2 = Fq2::new(
        MontFp!("21575463638280843010398324269430826099269044274347216827212613867836435027261"),
        MontFp!("10307601595873709700152284273816112264069230130616436755625194854815875713954"),
    );
    const TWIST_MUL_BY_Q_Y: Fq2 = Fq2::new(
        MontFp!("2821565182194536844548159561693502659359617185244120367078079554186484126554"),
        MontFp!("3505843767911556378687030309984248845540243509899259641013678093033130930403"),
    );
    const TWIST_TYPE: TwistType = TwistType::D;
    type Fp = Fq;
    type Fp2Config = Fq2Config;
    type Fp6Config = Fq6Config;
    type Fp12Config = Fq12Config;
    type G1Config = g1::Config;
    type G2Config = g2::Config;
}

// pub type Bn254 = Bn<Config>;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Bn254;

pub type G1Affine = bn::G1Affine<Config>;
pub type G1Projective = bn::G1Projective<Config>;
pub type G2Affine = bn::G2Affine<Config>;
pub type G2Projective = bn::G2Projective<Config>;
