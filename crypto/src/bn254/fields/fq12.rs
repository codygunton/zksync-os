use super::{Fq, Fq2, Fq6, Fq6Config};
#[cfg(any(
    all(any(target_arch = "riscv32", target_arch = "riscv64"), feature = "bigint_ops"),
    test,
    all(feature = "proving", fuzzing)
))]
use crate::ark_ff_delegation::MontFp;
#[cfg(not(any(
    all(any(target_arch = "riscv32", target_arch = "riscv64"), feature = "bigint_ops"),
    test,
    all(feature = "proving", fuzzing)
)))]
use ark_ff::MontFp;
use ark_ff::{AdditiveGroup, Field, Fp12, Fp12Config};

pub type Fq12 = Fp12<Fq12Config>;

/// Custom Fq12 inverse with volatile workarounds for RV64 compiler bugs
/// This replaces ark-ff's generic inverse which has corruption issues on RV64
#[cfg(target_arch = "riscv64")]
pub fn fq12_inverse_volatile(f: &Fq12) -> Option<Fq12> {
    use ark_ff::{Field, Fp6Config as Fp6ConfigTrait, Zero};

    if f.is_zero() {
        return None;
    }

    // Fq12 = Fq6[w]/(w^2 - v) where v is the nonresidue
    // inverse(a) = (c0 - c1*w) / (c0^2 - c1^2 * v)
    // We need: norm = c0^2 - c1^2 * v, then inverse = (c0 - c1*w) * norm^(-1)

    // Volatile refresh helper for Fq6 (24 u64 limbs)
    fn vol_refresh_fq6(val: &Fq6) -> Fq6 {
        let mut fresh = Fq6::default();
        let src = val as *const _ as *const u64;
        let dst = &mut fresh as *mut _ as *mut u64;
        for i in 0..24 {
            unsafe {
                let v = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), v);
            }
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        fresh
    }

    // Volatile refresh helper for Fq12 (48 u64 limbs)
    fn vol_refresh_fq12(val: &Fq12) -> Fq12 {
        let mut fresh = Fq12::default();
        let src = val as *const _ as *const u64;
        let dst = &mut fresh as *mut _ as *mut u64;
        for i in 0..48 {
            unsafe {
                let v = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), v);
            }
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        fresh
    }

    // Get c0 and c1 with volatile refresh
    let c0 = vol_refresh_fq6(&f.c0);
    let c1 = vol_refresh_fq6(&f.c1);

    // Compute c0^2
    let c0_sq = vol_refresh_fq6(&c0.square());

    // Compute c1^2
    let c1_sq = vol_refresh_fq6(&c1.square());

    // Compute c1^2 * nonresidue (v)
    // For Fq12 as Fq6 quadratic extension, the nonresidue is Fq6::new(Fq2::ZERO, Fq2::ONE, Fq2::ZERO)
    // Multiplying Fq6 by this is equivalent to "shifting" - multiply by w where w^3 = nonresidue
    // The formula is: (a, b, c) * nonresidue = (c * Fq2_nonresidue, a, b)
    let c1_sq_v = {
        // For Fq6, multiplying by the Fq12 nonresidue shifts the coefficients
        // c1_sq = (a, b, c) -> (c * fq6_nonresidue, a, b)
        let a = c1_sq.c0;
        let b = c1_sq.c1;
        let c = c1_sq.c2;
        // Multiply c by Fq6's nonresidue for Fq2 (which is 9 + u)
        let mut c_times_nr = c;
        <Fq6Config as Fp6ConfigTrait>::mul_fp2_by_nonresidue_in_place(&mut c_times_nr);
        vol_refresh_fq6(&Fq6::new(c_times_nr, a, b))
    };

    // norm = c0^2 - c1^2 * v
    let norm = vol_refresh_fq6(&(c0_sq - c1_sq_v));

    // Compute norm inverse using custom Fq6 inverse with volatile workarounds
    let norm_inv = match super::fq6::fq6_inverse_volatile(&norm) {
        Some(inv) => vol_refresh_fq6(&inv),
        None => return None,
    };

    // result.c0 = c0 * norm_inv
    let result_c0 = vol_refresh_fq6(&(c0 * norm_inv));

    // result.c1 = -c1 * norm_inv
    let result_c1 = vol_refresh_fq6(&(-(c1 * norm_inv)));

    // Construct result
    let result = Fq12::new(result_c0, result_c1);
    Some(vol_refresh_fq12(&result))
}

#[cfg(not(target_arch = "riscv64"))]
pub fn fq12_inverse_volatile(f: &Fq12) -> Option<Fq12> {
    use ark_ff::Field;
    f.inverse()
}

/// Custom Fq12 cyclotomic inverse with volatile workarounds for RV64 compiler bugs
/// For cyclotomic elements, the inverse is simply the conjugate: (c0, c1) -> (c0, -c1)
#[cfg(target_arch = "riscv64")]
pub fn fq12_cyclotomic_inverse_volatile(f: &Fq12) -> Option<Fq12> {
    use ark_ff::Zero;

    if f.is_zero() {
        return None;
    }

    // Volatile refresh helper for Fq6 (24 u64 limbs)
    fn vol_refresh_fq6(val: &Fq6) -> Fq6 {
        let mut fresh = Fq6::default();
        let src = val as *const _ as *const u64;
        let dst = &mut fresh as *mut _ as *mut u64;
        for i in 0..24 {
            unsafe {
                let v = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), v);
            }
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        fresh
    }

    // Volatile refresh helper for Fq12 (48 u64 limbs)
    fn vol_refresh_fq12(val: &Fq12) -> Fq12 {
        let mut fresh = Fq12::default();
        let src = val as *const _ as *const u64;
        let dst = &mut fresh as *mut _ as *mut u64;
        for i in 0..48 {
            unsafe {
                let v = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), v);
            }
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        fresh
    }

    // For cyclotomic elements, inverse is conjugate: (c0, c1) -> (c0, -c1)
    let c0 = vol_refresh_fq6(&f.c0);
    let c1_neg = vol_refresh_fq6(&(-f.c1));

    let result = Fq12::new(c0, c1_neg);
    Some(vol_refresh_fq12(&result))
}

#[cfg(not(target_arch = "riscv64"))]
pub fn fq12_cyclotomic_inverse_volatile(f: &Fq12) -> Option<Fq12> {
    use ark_ff::CyclotomicMultSubgroup;
    f.cyclotomic_inverse()
}

#[derive(Clone, Copy)]
pub struct Fq12Config;

impl Fp12Config for Fq12Config {
    type Fp6Config = Fq6Config;

    const NONRESIDUE: Fq6 = Fq6::new(Fq2::ZERO, Fq2::ONE, Fq2::ZERO);

    const FROBENIUS_COEFF_FP12_C1: &'static [Fq2] = &[
        // Fp2::NONRESIDUE^(((q^0) - 1) / 6)
        Fq2::new(Fq::ONE, Fq::ZERO),
        // Fp2::NONRESIDUE^(((q^1) - 1) / 6)
        Fq2::new(
            MontFp!("8376118865763821496583973867626364092589906065868298776909617916018768340080"),
            MontFp!(
                "16469823323077808223889137241176536799009286646108169935659301613961712198316"
            ),
        ),
        // Fp2::NONRESIDUE^(((q^2) - 1) / 6)
        Fq2::new(
            MontFp!(
                "21888242871839275220042445260109153167277707414472061641714758635765020556617"
            ),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^(((q^3) - 1) / 6)
        Fq2::new(
            MontFp!(
                "11697423496358154304825782922584725312912383441159505038794027105778954184319"
            ),
            MontFp!("303847389135065887422783454877609941456349188919719272345083954437860409601"),
        ),
        // Fp2::NONRESIDUE^(((q^4) - 1) / 6)
        Fq2::new(
            MontFp!(
                "21888242871839275220042445260109153167277707414472061641714758635765020556616"
            ),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^(((q^5) - 1) / 6)
        Fq2::new(
            MontFp!("3321304630594332808241809054958361220322477375291206261884409189760185844239"),
            MontFp!("5722266937896532885780051958958348231143373700109372999374820235121374419868"),
        ),
        // Fp2::NONRESIDUE^(((q^6) - 1) / 6)
        Fq2::new(MontFp!("-1"), Fq::ZERO),
        // Fp2::NONRESIDUE^(((q^7) - 1) / 6)
        Fq2::new(
            MontFp!(
                "13512124006075453725662431877630910996106405091429524885779419978626457868503"
            ),
            MontFp!("5418419548761466998357268504080738289687024511189653727029736280683514010267"),
        ),
        // Fp2::NONRESIDUE^(((q^8) - 1) / 6)
        Fq2::new(
            MontFp!("2203960485148121921418603742825762020974279258880205651966"),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^(((q^9) - 1) / 6)
        Fq2::new(
            MontFp!(
                "10190819375481120917420622822672549775783927716138318623895010788866272024264"
            ),
            MontFp!(
                "21584395482704209334823622290379665147239961968378104390343953940207365798982"
            ),
        ),
        // Fp2::NONRESIDUE^(((q^10) - 1) / 6)
        Fq2::new(
            MontFp!("2203960485148121921418603742825762020974279258880205651967"),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^(((q^11) - 1) / 6)
        Fq2::new(
            MontFp!(
                "18566938241244942414004596690298913868373833782006617400804628704885040364344"
            ),
            MontFp!(
                "16165975933942742336466353786298926857552937457188450663314217659523851788715"
            ),
        ),
    ];
}
