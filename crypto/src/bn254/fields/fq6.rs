use super::{Fq, Fq2, Fq2Config};
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
use ark_ff::{AdditiveGroup, Field, Fp2Config, Fp6, Fp6Config, Zero};

pub type Fq6 = Fp6<Fq6Config>;

/// Custom Fq6 inverse with volatile workarounds for RV64 compiler bugs
#[cfg(target_arch = "riscv64")]
pub fn fq6_inverse_volatile(f: &Fq6) -> Option<Fq6> {
    if f.is_zero() {
        return None;
    }

    // Volatile refresh helper for Fq2 (8 u64 limbs)
    fn vol_refresh_fq2(val: &Fq2) -> Fq2 {
        let mut fresh = Fq2::default();
        let src = val as *const _ as *const u64;
        let dst = &mut fresh as *mut _ as *mut u64;
        for i in 0..8 {
            unsafe {
                let v = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), v);
            }
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        fresh
    }

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

    // Algorithm 17 from "High-Speed Software Implementation of the Optimal Ate Pairing"
    let c0 = vol_refresh_fq2(&f.c0);
    let c1 = vol_refresh_fq2(&f.c1);
    let c2 = vol_refresh_fq2(&f.c2);

    let t0 = vol_refresh_fq2(&c0.square());
    let t1 = vol_refresh_fq2(&c1.square());
    let t2 = vol_refresh_fq2(&c2.square());
    let t3 = vol_refresh_fq2(&(c0 * c1));
    let t4 = vol_refresh_fq2(&(c0 * c2));
    let t5 = vol_refresh_fq2(&(c1 * c2));

    // n5 = t5 * nonresidue
    let n5 = {
        let mut tmp = t5;
        Fq6Config::mul_fp2_by_nonresidue_in_place(&mut tmp);
        vol_refresh_fq2(&tmp)
    };

    // s0 = t0 - n5
    let s0 = vol_refresh_fq2(&(t0 - n5));

    // s1 = t2 * nonresidue - t3
    let s1 = {
        let mut tmp = t2;
        Fq6Config::mul_fp2_by_nonresidue_in_place(&mut tmp);
        vol_refresh_fq2(&(tmp - t3))
    };

    // s2 = t1 - t4
    let s2 = vol_refresh_fq2(&(t1 - t4));

    // a1 = c2 * s1, a2 = c1 * s2
    let a1 = vol_refresh_fq2(&(c2 * s1));
    let a2 = vol_refresh_fq2(&(c1 * s2));

    // a3 = (a1 + a2) * nonresidue
    let a3 = {
        let sum = vol_refresh_fq2(&(a1 + a2));
        let mut tmp = sum;
        Fq6Config::mul_fp2_by_nonresidue_in_place(&mut tmp);
        vol_refresh_fq2(&tmp)
    };

    // t6 = (c0 * s0 + a3).inverse()
    let t6_pre = vol_refresh_fq2(&(c0 * s0 + a3));
    let t6 = match t6_pre.inverse() {
        Some(inv) => vol_refresh_fq2(&inv),
        None => return None,
    };

    let result_c0 = vol_refresh_fq2(&(t6 * s0));
    let result_c1 = vol_refresh_fq2(&(t6 * s1));
    let result_c2 = vol_refresh_fq2(&(t6 * s2));

    Some(vol_refresh_fq6(&Fq6::new(result_c0, result_c1, result_c2)))
}

#[cfg(not(target_arch = "riscv64"))]
pub fn fq6_inverse_volatile(f: &Fq6) -> Option<Fq6> {
    f.inverse()
}

#[derive(Clone, Copy)]
pub struct Fq6Config;

impl Fp6Config for Fq6Config {
    type Fp2Config = Fq2Config;

    /// NONRESIDUE = U+9
    const NONRESIDUE: Fq2 = Fq2::new(MontFp!("9"), Fq::ONE);

    const FROBENIUS_COEFF_FP6_C1: &'static [Fq2] = &[
        // Fp2::NONRESIDUE^(((q^0) - 1) / 3)
        Fq2::new(Fq::ONE, Fq::ZERO),
        // Fp2::NONRESIDUE^(((q^1) - 1) / 3)
        Fq2::new(
            MontFp!(
                "21575463638280843010398324269430826099269044274347216827212613867836435027261"
            ),
            MontFp!(
                "10307601595873709700152284273816112264069230130616436755625194854815875713954"
            ),
        ),
        // Fp2::NONRESIDUE^(((q^2) - 1) / 3)
        Fq2::new(
            MontFp!(
                "21888242871839275220042445260109153167277707414472061641714758635765020556616"
            ),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^(((q^3) - 1) / 3)
        Fq2::new(
            MontFp!("3772000881919853776433695186713858239009073593817195771773381919316419345261"),
            MontFp!("2236595495967245188281701248203181795121068902605861227855261137820944008926"),
        ),
        // Fp2::NONRESIDUE^(((q^4) - 1) / 3)
        Fq2::new(
            MontFp!("2203960485148121921418603742825762020974279258880205651966"),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^(((q^5) - 1) / 3)
        Fq2::new(
            MontFp!(
                "18429021223477853657660792034369865839114504446431234726392080002137598044644"
            ),
            MontFp!("9344045779998320333812420223237981029506012124075525679208581902008406485703"),
        ),
    ];

    const FROBENIUS_COEFF_FP6_C2: &'static [Fq2] = &[
        // Fp2::NONRESIDUE^((2*(q^0) - 2) / 3)
        Fq2::new(Fq::ONE, Fq::ZERO),
        // Fp2::NONRESIDUE^((2*(q^1) - 2) / 3)
        Fq2::new(
            MontFp!("2581911344467009335267311115468803099551665605076196740867805258568234346338"),
            MontFp!(
                "19937756971775647987995932169929341994314640652964949448313374472400716661030"
            ),
        ),
        // Fp2::NONRESIDUE^((2*(q^2) - 2) / 3)
        Fq2::new(
            MontFp!("2203960485148121921418603742825762020974279258880205651966"),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^((2*(q^3) - 2) / 3)
        Fq2::new(
            MontFp!("5324479202449903542726783395506214481928257762400643279780343368557297135718"),
            MontFp!(
                "16208900380737693084919495127334387981393726419856888799917914180988844123039"
            ),
        ),
        // Fp2::NONRESIDUE^((2*(q^4) - 2) / 3)
        Fq2::new(
            MontFp!(
                "21888242871839275220042445260109153167277707414472061641714758635765020556616"
            ),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^((2*(q^5) - 2) / 3)
        Fq2::new(
            MontFp!(
                "13981852324922362344252311234282257507216387789820983642040889267519694726527"
            ),
            MontFp!("7629828391165209371577384193250820201684255241773809077146787135900891633097"),
        ),
    ];

    #[inline(always)]
    fn mul_fp2_by_nonresidue_in_place(fe: &mut Fq2) -> &mut Fq2 {
        // (c0+u*c1)*(9+u) = (9*c0-c1)+u*(9*c1+c0)
        let mut f = *fe;
        f.double_in_place().double_in_place().double_in_place();
        let mut c0 = fe.c1;
        Fq2Config::mul_fp_by_nonresidue_in_place(&mut c0);
        c0 += &f.c0;
        c0 += &fe.c0;
        let c1 = f.c1 + fe.c1 + fe.c0;
        *fe = Fq2::new(c0, c1);
        fe
    }
}
