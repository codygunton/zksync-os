#[cfg(all(any(target_arch = "riscv32", target_arch = "riscv64"), not(feature = "bigint_ops")))]
compile_error!("feature `bigint_ops` must be activated for RISC-V target");

pub type Fq = Fp256<MontBackend<FqConfig, 4>>;
use crate::ark_ff_delegation::{BigInt, BigIntMacro, Fp, Fp256, MontBackend, MontConfig};
use crate::bigint_delegation::{u256, DelegatedModParams, DelegatedMontParams};
use ark_ff::ark_ff_macros::unroll_for_loops;
use ark_ff::{AdditiveGroup, Zero};

type B = BigInt<4>;
type F = Fp<MontBackend<FqConfig, 4usize>, 4usize>;

static MODULUS_CONSTANT: B =
    BigIntMacro!("21888242871839275222246405745257275088696311157297823662689037894645226208583");
// it's - MODULUS^-1 mod 2^256
static MONT_REDUCTION_CONSTANT: B =
    BigIntMacro!("111032442853175714102588374283752698368366046808579839647964533820976443843465");

// a^-1 = a ^ (p - 2), using Fermat's little theorem
const INVERSION_POW: B = BigInt([
    4332616871279656263u64 - 2,  // Subtract 2 from lowest limb
    10917124144477883021u64,
    13281191951274694749u64,
    3486998266802970665u64,
]);

#[derive(Default)]
struct FqParams;

impl DelegatedModParams<4> for FqParams {
    unsafe fn modulus() -> &'static BigInt<4> {
        &MODULUS_CONSTANT
    }
}

impl DelegatedMontParams<4> for FqParams {
    unsafe fn reduction_const() -> &'static BigInt<4> {
        &MONT_REDUCTION_CONSTANT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FqConfig;

// NOTE: even though we pretend to be u64 everywhere, on LE machine (and our RISC-V 32IM is such) we do not care
// for purposes of our precompile calls

impl MontConfig<4usize> for FqConfig {
    const MODULUS: B = BigInt([
        4332616871279656263u64,
        10917124144477883021u64,
        13281191951274694749u64,
        3486998266802970665u64,
    ]);
    const GENERATOR: F = {
        let (is_positive, limbs) = (true, [3u64]);
        Fp::from_sign_and_limbs(is_positive, &limbs)
    };
    const TWO_ADIC_ROOT_OF_UNITY: F = {
        let (is_positive, limbs) = (
            true,
            [
                4332616871279656262u64,
                10917124144477883021u64,
                13281191951274694749u64,
                3486998266802970665u64,
            ],
        );
        Fp::from_sign_and_limbs(is_positive, &limbs)
    };
    #[inline(always)]
    fn add_assign(a: &mut F, b: &F) {
        unsafe {
            u256::add_mod_assign::<FqParams>(&mut a.0, &b.0);
        }
    }
    #[inline(always)]
    fn sub_assign(a: &mut F, b: &F) {
        unsafe {
            u256::sub_mod_assign::<FqParams>(&mut a.0, &b.0);
        }
    }
    #[inline(always)]
    fn double_in_place(a: &mut F) {
        unsafe {
            u256::double_mod_assign::<FqParams>(&mut a.0);
        }
    }
    /// Sets `a = -a`.
    #[inline(always)]
    fn neg_in_place(a: &mut F) {
        unsafe {
            u256::neg_mod_assign::<FqParams>(&mut a.0);
        }
    }
    #[inline(always)]
    fn mul_assign(a: &mut F, b: &F) {
        unsafe {
            u256::mul_assign_montgomery::<FqParams>(&mut a.0, &b.0);
        }
    }

    #[inline(always)]
    fn square_in_place(a: &mut F) {
        unsafe {
            u256::square_assign_montgomery::<FqParams>(&mut a.0);
        }
    }

    // We will override to also use 256-digit approach here
    #[inline(always)]
    fn into_bigint(mut a: Fp<MontBackend<Self, 4>, 4>) -> BigInt<4> {
        // for now it's just a multiplication with 1 literal
        unsafe {
            u256::mul_assign_montgomery::<FqParams>(&mut a.0, &BigInt::one());
        }

        a.0
    }

    #[inline(always)]
    fn inverse(a: &Fp<MontBackend<Self, 4>, 4>) -> Option<Fp<MontBackend<Self, 4>, 4>> {
        __gcd_inverse(a)
    }

    // default impl
    #[inline(always)]
    #[unroll_for_loops(8)]
    fn sum_of_products<const M: usize>(a: &[F; M], b: &[F; M]) -> F {
        let mut sum = F::ZERO;
        for i in 0..a.len() {
            sum += a[i] * &b[i];
        }
        sum
    }
}

fn __gcd_inverse(a: &F) -> Option<F> {
    if a.is_zero() {
        return None;
    }
    // Guajardo Kumar Paar Pelzl
    // Efficient Software-Implementation of Finite Fields with Applications to
    // Cryptography
    // Algorithm 16 (BEA for Inversion in Fp)

    use ark_ff::BigInteger;
    use ark_ff::PrimeField;

    // UART logging for RV64 debugging
    #[cfg(target_arch = "riscv64")]
    fn log_gcd(msg: &str, val: &B) {
        static mut LOG_COUNT: u32 = 0;
        unsafe {
            LOG_COUNT += 1;
            if LOG_COUNT > 50 { return; } // Limit logging
        }
        const HELLO_MARKER: u32 = u32::MAX;
        fn csr_write_word(word: usize) {
            unsafe {
                core::arch::asm!(
                    "csrrw x0, 0x7c0, {rd}",
                    rd = in(reg) word,
                    options(nomem, nostack, preserves_flags)
                )
            }
        }
        fn write_str(s: &str) {
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
        fn write_hex_u64(v: u64) {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let mut buf = [0u8; 16];
            for i in 0..16 {
                buf[15 - i] = HEX[((v >> (i * 4)) & 0xf) as usize];
            }
            let s = unsafe { core::str::from_utf8_unchecked(&buf) };
            write_str(s);
        }
        write_str("[GUEST] [gcd_inv] ");
        write_str(msg);
        write_str(" limb0=0x");
        write_hex_u64(unsafe { core::ptr::read_volatile(&val.0[0]) });
        write_str("\n");
    }
    #[cfg(not(target_arch = "riscv64"))]
    fn log_gcd(_msg: &str, _val: &B) {}

    let mut u = a.0;
    let mut v = F::MODULUS;
    let mut b = Fp::new_unchecked(F::R2); // Avoids unnecessary reduction step.
    let mut c = Fp::zero();
    let modulus = F::MODULUS;

    log_gcd("input a", &a.0);
    log_gcd("initial u", &u);
    log_gcd("initial v", &v);
    log_gcd("initial b", &b.0);

    let mut iter_count = 0u32;
    while !u256::is_one(&u) && !u256::is_one(&v) {
        iter_count += 1;
        if iter_count > 1000 { break; } // Safety limit

        while u.is_even() {
            u.div2();

            if b.0.is_even() {
                b.0.div2();
            } else {
                let _carry = u256::add_assign(&mut b.0, &modulus);
                b.0.div2();
            }
        }

        while v.is_even() {
            v.div2();

            if c.0.is_even() {
                c.0.div2();
            } else {
                let _carry = u256::add_assign(&mut c.0, &modulus);
                c.0.div2();
            }
        }

        if v.lt(&u) {
            u256::sub_assign(&mut u, &v);
            b -= &c;
        } else {
            u256::sub_assign(&mut v, &u);
            c -= &b;
        }

        // Log every 100 iterations
        if iter_count % 100 == 0 {
            log_gcd("u @iter", &u);
            log_gcd("b @iter", &b.0);
        }
    }

    log_gcd("final u", &u);
    log_gcd("final b", &b.0);
    log_gcd("final c", &c.0);

    let result = if u256::is_one(&u) { b } else { c };

    // Verify result
    let check = *a * result;
    log_gcd("a*result", &check.0);

    if u256::is_one(&u) {
        Some(b)
    } else {
        Some(c)
    }
}

#[cfg(test)]
mod test {
    use super::Fq;
    use ark_ff::{Field, One, UniformRand, Zero};

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_mul_properties() {
        const ITERATIONS: usize = 1000;
        use ark_std::test_rng;
        let mut rng = test_rng();
        let zero = Fq::zero();
        let one = Fq::one();
        assert_eq!(one.inverse().unwrap(), one, "One inverse failed");
        assert!(one.is_one(), "One is not one");

        assert!(Fq::ONE.is_one(), "One constant is not one");
        assert_eq!(Fq::ONE, one, "One constant is incorrect");

        for _ in 0..ITERATIONS {
            // Associativity
            let a = Fq::rand(&mut rng);
            let b = Fq::rand(&mut rng);
            let c = Fq::rand(&mut rng);
            assert_eq!((a * b) * c, a * (b * c), "Associativity failed");

            // Commutativity
            assert_eq!(a * b, b * a, "Commutativity failed");

            // Identity
            assert_eq!(one * a, a, "Identity mul failed");
            assert_eq!(one * b, b, "Identity mul failed");
            assert_eq!(one * c, c, "Identity mul failed");

            assert_eq!(zero * a, zero, "Mul by zero failed");
            assert_eq!(zero * b, zero, "Mul by zero failed");
            assert_eq!(zero * c, zero, "Mul by zero failed");

            // Inverses
            assert_eq!(a * a.inverse().unwrap(), one, "Mul by inverse failed");
            assert_eq!(b * b.inverse().unwrap(), one, "Mul by inverse failed");
            assert_eq!(c * c.inverse().unwrap(), one, "Mul by inverse failed");

            // Associativity and commutativity simultaneously
            let t0 = (a * b) * c;
            let t1 = (a * c) * b;
            let t2 = (b * c) * a;
            assert_eq!(t0, t1, "Associativity + commutativity failed");
            assert_eq!(t1, t2, "Associativity + commutativity failed");

            // Squaring
            assert_eq!(a * a, a.square(), "Squaring failed");
            assert_eq!(b * b, b.square(), "Squaring failed");
            assert_eq!(c * c, c.square(), "Squaring failed");

            // Distributivity
            assert_eq!(a * (b + c), a * b + a * c, "Distributivity failed");
            assert_eq!(b * (a + c), b * a + b * c, "Distributivity failed");
            assert_eq!(c * (a + b), c * a + c * b, "Distributivity failed");
            assert_eq!(
                (a + b).square(),
                a.square() + b.square() + a * ark_ff::AdditiveGroup::double(&b),
                "Distributivity for square failed"
            );
            assert_eq!(
                (b + c).square(),
                c.square() + b.square() + c * ark_ff::AdditiveGroup::double(&b),
                "Distributivity for square failed"
            );
            assert_eq!(
                (c + a).square(),
                a.square() + c.square() + a * ark_ff::AdditiveGroup::double(&c),
                "Distributivity for square failed"
            );
        }
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_mul_correctness() {
        use std::str::FromStr;

        type RefFq = ark_bn254::Fq;

        let a = Fq::from_str("-1").unwrap();
        let ref_a = RefFq::from_str("-1").unwrap();

        assert_eq!(a.0 .0, ref_a.0 .0);
    }

    // NOTE: those tests are backported as we need to init static and run single thread
    // instead of full arkwords test suite. This coverage is ok as our base math is just
    // very small

    pub const ITERATIONS: usize = 100;
    use crate::bn254::curves::Bn254;
    use ark_ec::{pairing::*, CurveGroup, PrimeGroup};
    use ark_ff::{CyclotomicMultSubgroup, PrimeField};
    use ark_std::test_rng;

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_bilinearity() {
        for _ in 0..ITERATIONS {
            let mut rng = test_rng();
            let a: <Bn254 as Pairing>::G1 = UniformRand::rand(&mut rng);
            let b: <Bn254 as Pairing>::G2 = UniformRand::rand(&mut rng);
            let s: <Bn254 as Pairing>::ScalarField = UniformRand::rand(&mut rng);

            let sa = a * s;
            let sb = b * s;

            let ans1 = <Bn254>::pairing(sa, b);
            let ans2 = <Bn254>::pairing(a, sb);
            let ans3 = <Bn254>::pairing(a, b) * s;

            assert_eq!(ans1, ans2);
            assert_eq!(ans2, ans3);

            assert_ne!(ans1, PairingOutput::zero());
            assert_ne!(ans2, PairingOutput::zero());
            assert_ne!(ans3, PairingOutput::zero());
            let group_order = <<Bn254 as Pairing>::ScalarField>::characteristic();

            assert_eq!(ans1.mul_bigint(group_order), PairingOutput::zero());
            assert_eq!(ans2.mul_bigint(group_order), PairingOutput::zero());
            assert_eq!(ans3.mul_bigint(group_order), PairingOutput::zero());
        }
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_multi_pairing() {
        for _ in 0..ITERATIONS {
            let rng = &mut test_rng();

            let a = <Bn254 as Pairing>::G1::rand(rng).into_affine();
            let b = <Bn254 as Pairing>::G2::rand(rng).into_affine();
            let c = <Bn254 as Pairing>::G1::rand(rng).into_affine();
            let d = <Bn254 as Pairing>::G2::rand(rng).into_affine();
            let ans1 = <Bn254>::pairing(a, b) + &<Bn254>::pairing(c, d);
            let ans2 = <Bn254>::multi_pairing(&[a, c], &[b, d]);
            assert_eq!(ans1, ans2);
        }
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_final_exp() {
        for _ in 0..ITERATIONS {
            let rng = &mut test_rng();
            let fp_ext = <Bn254 as Pairing>::TargetField::rand(rng);
            let gt = <Bn254 as Pairing>::final_exponentiation(MillerLoopOutput(fp_ext))
                .unwrap()
                .0;
            let r = <Bn254 as Pairing>::ScalarField::MODULUS;
            assert!(gt.cyclotomic_exp(r).is_one());
        }
    }
}
