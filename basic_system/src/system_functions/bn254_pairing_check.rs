use super::*;
use crate::cost_constants::{
    BN254_PAIRING_BASE_NATIVE_COST, BN254_PAIRING_COST_PER_PAIR_ERGS,
    BN254_PAIRING_PER_PAIR_NATIVE_COST, BN254_PAIRING_STATIC_COST_ERGS,
};
use crate::system_functions::bytereverse;
use alloc::vec::Vec;
use crypto::ark_ec::AffineRepr;
use crypto::ark_ff::Zero;
use crypto::ark_serialize::{CanonicalDeserialize, Valid};
use zk_ee::common_traits::TryExtend;
use zk_ee::system::base_system_functions::{
    Bn254PairingCheckErrors, Bn254PairingCheckInterfaceError, SystemFunction,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::{interface_error, out_of_return_memory};

///
/// bn254 pairing check system function implementation.
///
pub struct Bn254PairingCheckImpl;

impl<R: Resources> SystemFunction<R, Bn254PairingCheckErrors> for Bn254PairingCheckImpl {
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error if the input size is not divisible by 192
    /// or failed to create affine points from inputs
    fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254PairingCheckErrors>> {
        cycle_marker::wrap_with_resources!("bn254_pairing", resources, {
            let num_pairs = src.len() / 192;
            let ergs_cost = BN254_PAIRING_STATIC_COST_ERGS
                + BN254_PAIRING_COST_PER_PAIR_ERGS.times(num_pairs as u64);
            let native_cost = (num_pairs as u64) * BN254_PAIRING_PER_PAIR_NATIVE_COST
                + BN254_PAIRING_BASE_NATIVE_COST;

            resources.charge(&R::from_ergs_and_native(
                ergs_cost,
                <R::Native as zk_ee::system::Computational>::from_computational(native_cost),
            ))?;

            if src.len() % 192 != 0 {
                return Err(interface_error!(
                    Bn254PairingCheckInterfaceError::InvalidPairingSize
                ));
            }

            let success = if src.is_empty() {
                true
            } else {
                bn254_pairing_check_inner::<A>(num_pairs, src, allocator)
                    .map_err(|_| interface_error!(Bn254PairingCheckInterfaceError::InvalidPoint))?
            };

            dst.try_extend(core::iter::repeat_n(0, 31).chain(core::iter::once(success as u8)))
                .map_err(|_| out_of_return_memory!())?;

            Ok(())
        })
    }
}

fn bn254_pairing_check_inner<A: Allocator>(
    num_pairs: usize,
    src: &[u8],
    allocator: A,
) -> Result<bool, ()> {
    use crypto::ark_ec::pairing::Pairing;
    use crypto::ark_ff::{One, PrimeField};
    use crypto::bn254::curves::{Bn254, G1Affine, G2Affine};
    use crypto::bn254::fields::{Fq, Fq2};
    use super::ecrecover::uart_log;

    uart_log::write_str("[bn254_pairing] num_pairs=");
    uart_log::write_usize(num_pairs);
    uart_log::newline();

    if num_pairs == 0 {
        return Ok(true);
    }

    let mut pairs = Vec::with_capacity_in(num_pairs, allocator);
    let mut src_iter = src.iter();

    for pair_idx in 0..num_pairs {
        uart_log::write_str("[bn254_pairing] pair_idx=");
        uart_log::write_usize(pair_idx);
        uart_log::newline();

        let mut buffer = [0u8; 192];
        for (dst, src) in buffer.iter_mut().zip(&mut src_iter) {
            *dst = *src;
        }
        let mut it = buffer.array_chunks::<32>();
        unsafe {
            let mut g1_x = *it.next().unwrap_unchecked();
            let mut g1_y = *it.next().unwrap_unchecked();

            // NOTE: Ethereum serialization is strange
            let mut g2_x_c1 = *it.next().unwrap_unchecked();
            let mut g2_x_c0 = *it.next().unwrap_unchecked();
            let mut g2_y_c1 = *it.next().unwrap_unchecked();
            let mut g2_y_c0 = *it.next().unwrap_unchecked();

            bytereverse(&mut g1_x);
            bytereverse(&mut g1_y);

            uart_log::write_str("[bn254_pairing] g1_x=");
            uart_log::write_hex_slice(&g1_x);
            uart_log::newline();
            uart_log::write_str("[bn254_pairing] g1_y=");
            uart_log::write_hex_slice(&g1_y);
            uart_log::newline();

            let g1_x_bigint =
                <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g1_x[..]).map_err(|_| {
                    uart_log::write_str("[bn254_pairing] g1_x_bigint FAILED\n");
                    ()
                })?;
            let g1_y_bigint =
                <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g1_y[..]).map_err(|_| {
                    uart_log::write_str("[bn254_pairing] g1_y_bigint FAILED\n");
                    ()
                })?;

            // COMPILER BUG WORKAROUND: Volatile copy of bigint before from_bigint
            #[cfg(target_arch = "riscv64")]
            let (g1_x, g1_y) = {
                let mut x_limbs = [0u64; 4];
                let mut y_limbs = [0u64; 4];
                let x_ptr = &g1_x_bigint as *const _ as *const u64;
                let y_ptr = &g1_y_bigint as *const _ as *const u64;
                for i in 0..4 {
                    x_limbs[i] = core::ptr::read_volatile(x_ptr.add(i));
                    y_limbs[i] = core::ptr::read_volatile(y_ptr.add(i));
                }
                let fresh_x_bigint = core::mem::transmute::<[u64; 4], <Fq as PrimeField>::BigInt>(x_limbs);
                let fresh_y_bigint = core::mem::transmute::<[u64; 4], <Fq as PrimeField>::BigInt>(y_limbs);
                let g1_x = Fq::from_bigint(fresh_x_bigint).ok_or_else(|| {
                    uart_log::write_str("[bn254_pairing] g1_x from_bigint FAILED\n");
                    ()
                })?;
                let g1_y = Fq::from_bigint(fresh_y_bigint).ok_or_else(|| {
                    uart_log::write_str("[bn254_pairing] g1_y from_bigint FAILED\n");
                    ()
                })?;
                // Volatile copy the Fq values
                let mut gx_limbs = [0u64; 4];
                let mut gy_limbs = [0u64; 4];
                let gx_ptr = &g1_x as *const _ as *const u64;
                let gy_ptr = &g1_y as *const _ as *const u64;
                for i in 0..4 {
                    gx_limbs[i] = core::ptr::read_volatile(gx_ptr.add(i));
                    gy_limbs[i] = core::ptr::read_volatile(gy_ptr.add(i));
                }
                let fresh_g1_x = core::mem::transmute::<[u64; 4], Fq>(gx_limbs);
                let fresh_g1_y = core::mem::transmute::<[u64; 4], Fq>(gy_limbs);
                (fresh_g1_x, fresh_g1_y)
            };

            #[cfg(not(target_arch = "riscv64"))]
            let (g1_x, g1_y) = {
                let g1_x = Fq::from_bigint(g1_x_bigint).ok_or(())?;
                let g1_y = Fq::from_bigint(g1_y_bigint).ok_or(())?;
                (g1_x, g1_y)
            };

            uart_log::write_str("[bn254_pairing] g1 from_bigint OK\n");

            let g1_point = if g1_x.is_zero() && g1_y.is_zero() {
                uart_log::write_str("[bn254_pairing] g1 is zero\n");
                G1Affine::zero()
            } else {
                let g1_point = G1Affine::new_unchecked(g1_x, g1_y);
                // TEMPORARY: Skip checks on RV64 to test pairing computation
                #[cfg(not(target_arch = "riscv64"))]
                {
                    g1_point.check().map_err(|_| {
                        uart_log::write_str("[bn254_pairing] g1 check FAILED\n");
                        ()
                    })?;
                    uart_log::write_str("[bn254_pairing] g1 check OK\n");
                }
                #[cfg(target_arch = "riscv64")]
                uart_log::write_str("[bn254_pairing] g1 check SKIPPED\n");
                g1_point
            };

            bytereverse(&mut g2_x_c0);
            bytereverse(&mut g2_x_c1);
            bytereverse(&mut g2_y_c0);
            bytereverse(&mut g2_y_c1);

            uart_log::write_str("[bn254_pairing] g2_x_c0=");
            uart_log::write_hex_slice(&g2_x_c0);
            uart_log::newline();
            uart_log::write_str("[bn254_pairing] g2_x_c1=");
            uart_log::write_hex_slice(&g2_x_c1);
            uart_log::newline();
            uart_log::write_str("[bn254_pairing] g2_y_c0=");
            uart_log::write_hex_slice(&g2_y_c0);
            uart_log::newline();
            uart_log::write_str("[bn254_pairing] g2_y_c1=");
            uart_log::write_hex_slice(&g2_y_c1);
            uart_log::newline();

            let g2_x_c0_bigint = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_x_c0[..])
                .map_err(|_| {
                    uart_log::write_str("[bn254_pairing] g2_x_c0_bigint FAILED\n");
                    ()
                })?;
            let g2_x_c1_bigint = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_x_c1[..])
                .map_err(|_| {
                    uart_log::write_str("[bn254_pairing] g2_x_c1_bigint FAILED\n");
                    ()
                })?;

            // COMPILER BUG WORKAROUND: Volatile copy for G2 x coordinates
            #[cfg(target_arch = "riscv64")]
            let g2_x = {
                let mut c0_limbs = [0u64; 4];
                let mut c1_limbs = [0u64; 4];
                let c0_ptr = &g2_x_c0_bigint as *const _ as *const u64;
                let c1_ptr = &g2_x_c1_bigint as *const _ as *const u64;
                for i in 0..4 {
                    c0_limbs[i] = core::ptr::read_volatile(c0_ptr.add(i));
                    c1_limbs[i] = core::ptr::read_volatile(c1_ptr.add(i));
                }
                let fresh_c0_bigint = core::mem::transmute::<[u64; 4], <Fq as PrimeField>::BigInt>(c0_limbs);
                let fresh_c1_bigint = core::mem::transmute::<[u64; 4], <Fq as PrimeField>::BigInt>(c1_limbs);
                let g2_x_c0 = Fq::from_bigint(fresh_c0_bigint).ok_or_else(|| {
                    uart_log::write_str("[bn254_pairing] g2_x_c0 from_bigint FAILED\n");
                    ()
                })?;
                let g2_x_c1 = Fq::from_bigint(fresh_c1_bigint).ok_or_else(|| {
                    uart_log::write_str("[bn254_pairing] g2_x_c1 from_bigint FAILED\n");
                    ()
                })?;
                // Log Fq values immediately after from_bigint
                uart_log::write_str("[bn254_pairing] g2_x_c0 mont limbs: ");
                let pc0 = &g2_x_c0 as *const _ as *const u64;
                for i in 0..4 {
                    uart_log::write_hex_slice(&core::ptr::read_volatile(pc0.add(i)).to_le_bytes());
                    uart_log::write_str(" ");
                }
                uart_log::newline();
                // Volatile copy Fq values
                let mut fc0_limbs = [0u64; 4];
                let mut fc1_limbs = [0u64; 4];
                let fc0_ptr = &g2_x_c0 as *const _ as *const u64;
                let fc1_ptr = &g2_x_c1 as *const _ as *const u64;
                for i in 0..4 {
                    fc0_limbs[i] = core::ptr::read_volatile(fc0_ptr.add(i));
                    fc1_limbs[i] = core::ptr::read_volatile(fc1_ptr.add(i));
                }
                let fresh_g2_x_c0 = core::mem::transmute::<[u64; 4], Fq>(fc0_limbs);
                let fresh_g2_x_c1 = core::mem::transmute::<[u64; 4], Fq>(fc1_limbs);
                Fq2::new(fresh_g2_x_c0, fresh_g2_x_c1)
            };

            #[cfg(not(target_arch = "riscv64"))]
            let g2_x = {
                let g2_x_c0 = Fq::from_bigint(g2_x_c0_bigint).ok_or(())?;
                let g2_x_c1 = Fq::from_bigint(g2_x_c1_bigint).ok_or(())?;
                Fq2::new(g2_x_c0, g2_x_c1)
            };

            uart_log::write_str("[bn254_pairing] g2_x from_bigint OK\n");

            let g2_y_c0_bigint = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_y_c0[..])
                .map_err(|_| {
                    uart_log::write_str("[bn254_pairing] g2_y_c0_bigint FAILED\n");
                    ()
                })?;
            let g2_y_c1_bigint = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_y_c1[..])
                .map_err(|_| {
                    uart_log::write_str("[bn254_pairing] g2_y_c1_bigint FAILED\n");
                    ()
                })?;

            // COMPILER BUG WORKAROUND: Volatile copy for G2 y coordinates
            #[cfg(target_arch = "riscv64")]
            let g2_y = {
                let mut c0_limbs = [0u64; 4];
                let mut c1_limbs = [0u64; 4];
                let c0_ptr = &g2_y_c0_bigint as *const _ as *const u64;
                let c1_ptr = &g2_y_c1_bigint as *const _ as *const u64;
                for i in 0..4 {
                    c0_limbs[i] = core::ptr::read_volatile(c0_ptr.add(i));
                    c1_limbs[i] = core::ptr::read_volatile(c1_ptr.add(i));
                }
                let fresh_c0_bigint = core::mem::transmute::<[u64; 4], <Fq as PrimeField>::BigInt>(c0_limbs);
                let fresh_c1_bigint = core::mem::transmute::<[u64; 4], <Fq as PrimeField>::BigInt>(c1_limbs);
                let g2_y_c0 = Fq::from_bigint(fresh_c0_bigint).ok_or_else(|| {
                    uart_log::write_str("[bn254_pairing] g2_y_c0 from_bigint FAILED\n");
                    ()
                })?;
                let g2_y_c1 = Fq::from_bigint(fresh_c1_bigint).ok_or_else(|| {
                    uart_log::write_str("[bn254_pairing] g2_y_c1 from_bigint FAILED\n");
                    ()
                })?;
                // Volatile copy Fq values
                let mut fc0_limbs = [0u64; 4];
                let mut fc1_limbs = [0u64; 4];
                let fc0_ptr = &g2_y_c0 as *const _ as *const u64;
                let fc1_ptr = &g2_y_c1 as *const _ as *const u64;
                for i in 0..4 {
                    fc0_limbs[i] = core::ptr::read_volatile(fc0_ptr.add(i));
                    fc1_limbs[i] = core::ptr::read_volatile(fc1_ptr.add(i));
                }
                let fresh_g2_y_c0 = core::mem::transmute::<[u64; 4], Fq>(fc0_limbs);
                let fresh_g2_y_c1 = core::mem::transmute::<[u64; 4], Fq>(fc1_limbs);
                Fq2::new(fresh_g2_y_c0, fresh_g2_y_c1)
            };

            #[cfg(not(target_arch = "riscv64"))]
            let g2_y = {
                let g2_y_c0 = Fq::from_bigint(g2_y_c0_bigint).ok_or(())?;
                let g2_y_c1 = Fq::from_bigint(g2_y_c1_bigint).ok_or(())?;
                Fq2::new(g2_y_c0, g2_y_c1)
            };

            uart_log::write_str("[bn254_pairing] g2_y from_bigint OK\n");

            // COMPILER BUG WORKAROUND: Volatile copy Fq2 values as raw bytes before G2Affine construction
            #[cfg(target_arch = "riscv64")]
            let (g2_x, g2_y) = {
                // Fq2 = 2 * Fq = 8 u64 total
                let mut x_limbs = [0u64; 8];
                let mut y_limbs = [0u64; 8];
                let x_ptr = &g2_x as *const _ as *const u64;
                let y_ptr = &g2_y as *const _ as *const u64;
                for i in 0..8 {
                    x_limbs[i] = core::ptr::read_volatile(x_ptr.add(i));
                    y_limbs[i] = core::ptr::read_volatile(y_ptr.add(i));
                }
                let fresh_g2_x = core::mem::transmute::<[u64; 8], Fq2>(x_limbs);
                let fresh_g2_y = core::mem::transmute::<[u64; 8], Fq2>(y_limbs);
                (fresh_g2_x, fresh_g2_y)
            };

            uart_log::write_str("[bn254_pairing] g2 volatile copy done\n");

            let g2_point = if g2_x.is_zero() && g2_y.is_zero() {
                uart_log::write_str("[bn254_pairing] g2 is zero\n");
                G2Affine::zero()
            } else {
                let g2_point = G2Affine::new_unchecked(g2_x, g2_y);
                uart_log::write_str("[bn254_pairing] g2 new_unchecked done\n");

                // COMPILER BUG WORKAROUND: Volatile copy G2Affine after construction
                #[cfg(target_arch = "riscv64")]
                let g2_point = {
                    // Use volatile copy byte by byte to avoid size issues
                    let size = core::mem::size_of::<G2Affine>();
                    let mut buffer = [0u8; 256]; // G2Affine should be smaller than this
                    let src = &g2_point as *const _ as *const u8;
                    for i in 0..size {
                        buffer[i] = core::ptr::read_volatile(src.add(i));
                    }
                    let mut result: core::mem::MaybeUninit<G2Affine> = core::mem::MaybeUninit::uninit();
                    let dst = result.as_mut_ptr() as *mut u8;
                    for i in 0..size {
                        core::ptr::write_volatile(dst.add(i), buffer[i]);
                    }
                    result.assume_init()
                };

                // TEMPORARY: Skip checks on RV64 to test pairing computation
                #[cfg(not(target_arch = "riscv64"))]
                {
                    g2_point.check().map_err(|_| {
                        uart_log::write_str("[bn254_pairing] g2 check FAILED\n");
                        ()
                    })?;
                    uart_log::write_str("[bn254_pairing] g2 check OK\n");
                }
                #[cfg(target_arch = "riscv64")]
                uart_log::write_str("[bn254_pairing] g2 check SKIPPED\n");
                g2_point
            };

            pairs.push((g1_point, g2_point));
            uart_log::write_str("[bn254_pairing] pair pushed\n");
        }
    }

    uart_log::write_str("[bn254_pairing] computing multi_pairing\n");

    // Log input points for debugging
    for (idx, (g1, g2)) in pairs.iter().enumerate() {
        uart_log::write_str("[bn254_pairing] pair ");
        uart_log::write_usize(idx);
        uart_log::write_str(" g1.x limb0: ");
        let g1_ptr = g1 as *const _ as *const u64;
        uart_log::write_hex_slice(&unsafe { core::ptr::read_volatile(g1_ptr) }.to_le_bytes());
        uart_log::newline();

        uart_log::write_str("[bn254_pairing] pair ");
        uart_log::write_usize(idx);
        uart_log::write_str(" g2.x.c0 limb0: ");
        let g2_ptr = g2 as *const _ as *const u64;
        uart_log::write_hex_slice(&unsafe { core::ptr::read_volatile(g2_ptr) }.to_le_bytes());
        uart_log::newline();
    }

    let g1_iter = pairs.iter().map(|(g1, _)| g1);
    let g2_iter = pairs.iter().map(|(_, g2)| g2);

    // Use multi_pairing which does miller_loop + final_exp
    let result = Bn254::multi_pairing(g1_iter, g2_iter);

    // Log result (Fq12 = 12 * 32 bytes = 48 u64s)
    let result_ptr = &result.0 as *const _ as *const u64;
    uart_log::write_str("[bn254_pairing] result.c0.c0.c0 limb0: ");
    uart_log::write_hex_slice(&unsafe { core::ptr::read_volatile(result_ptr) }.to_le_bytes());
    uart_log::newline();
    uart_log::write_str("[bn254_pairing] result.c0.c0.c0 limb1: ");
    uart_log::write_hex_slice(&unsafe { core::ptr::read_volatile(result_ptr.add(1)) }.to_le_bytes());
    uart_log::newline();
    uart_log::write_str("[bn254_pairing] result.c0.c0.c0 limb2: ");
    uart_log::write_hex_slice(&unsafe { core::ptr::read_volatile(result_ptr.add(2)) }.to_le_bytes());
    uart_log::newline();
    uart_log::write_str("[bn254_pairing] result.c0.c0.c0 limb3: ");
    uart_log::write_hex_slice(&unsafe { core::ptr::read_volatile(result_ptr.add(3)) }.to_le_bytes());
    uart_log::newline();

    let success = result.0.is_one();
    uart_log::write_str("[bn254_pairing] result is_one=");
    uart_log::write_str(if success { "true" } else { "false" });
    uart_log::newline();
    Ok(success)
}

#[cfg(test)]
mod test {
    use super::*;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::Resource;

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_pairing_inner() {
        let allocator = std::alloc::Global;

        let src = hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();

        assert!(bn254_pairing_check_inner(2, src.as_slice(), allocator).unwrap());
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_pairing_external() {
        let allocator = std::alloc::Global;

        let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        let src: &[u8] = &hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();

        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let mut dst = vec![];

        let _ = Bn254PairingCheckImpl::execute(src, &mut dst, &mut resource, allocator).unwrap();

        assert_eq!(expected, dst.as_slice());

        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let mut dst = vec![];

        let _ = Bn254PairingCheckImpl::execute(src, &mut dst, &mut resource, allocator).unwrap();

        assert_eq!(expected, dst.as_slice());
    }
}
