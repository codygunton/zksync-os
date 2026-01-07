use super::*;
use crate::cost_constants::BN254_ECMUL_NATIVE_COST;
use crate::system_functions::bytereverse;
use crate::{
    cost_constants::BN254_ECMUL_COST_ERGS, system_functions::bn254_ecadd::serialize_projective,
};
use crypto::ark_serialize::Valid;
use zk_ee::common_traits::TryExtend;
use zk_ee::system::base_system_functions::{
    Bn254MulErrors, Bn254MulInterfaceError, SystemFunction,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::{interface_error, out_of_return_memory};

///
/// bn254 ecmul system function implementation.
///
pub struct Bn254MulImpl;

impl<R: Resources> SystemFunction<R, Bn254MulErrors> for Bn254MulImpl {
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    ///
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error only if failed to create affine points from inputs.
    fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), SubsystemError<Bn254MulErrors>> {
        cycle_marker::wrap_with_resources!("bn254_ecmul", resources, {
            bn254_ecmul_as_system_function_inner(input, output, resources)
        })
    }
}

fn bn254_ecmul_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SubsystemError<Bn254MulErrors>> {
    resources.charge(&R::from_ergs_and_native(
        BN254_ECMUL_COST_ERGS,
        <R::Native as zk_ee::system::Computational>::from_computational(BN254_ECMUL_NATIVE_COST),
    ))?;

    let mut buffer = [0u8; 96];
    for (dst, src) in buffer.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    let mut it = buffer.array_chunks::<32>();
    // Use out-parameter to avoid function return value corruption on RV64
    let mut serialized_result = [0u8; 64];
    unsafe {
        let x0 = it.next().unwrap_unchecked();
        let y0 = it.next().unwrap_unchecked();
        let scalar = it.next().unwrap_unchecked();

        bn254_ecmul_inner(x0, y0, scalar, &mut serialized_result).map_err(|_| -> SubsystemError<_> {
            interface_error!(Bn254MulInterfaceError::InvalidPoint)
        })?
    };

    dst.try_extend(serialized_result)
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

/// UART helpers for BN254 logging (RV64)
#[cfg(target_arch = "riscv64")]
fn bn254_uart_byte(b: u8) {
    unsafe { core::ptr::write_volatile(0xa000_0200u64 as *mut u8, b); }
}
#[cfg(target_arch = "riscv64")]
fn bn254_uart_str(s: &str) {
    for b in s.bytes() { bn254_uart_byte(b); }
}
#[cfg(target_arch = "riscv64")]
fn bn254_uart_hex_byte(b: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    bn254_uart_byte(HEX[(b >> 4) as usize]);
    bn254_uart_byte(HEX[(b & 0xf) as usize]);
}
#[cfg(target_arch = "riscv64")]
fn bn254_uart_hex_slice(bytes: &[u8]) {
    for b in bytes { bn254_uart_hex_byte(*b); }
}

/// Computes BN254 scalar multiplication and writes result to `out` using volatile writes.
/// Uses out-parameter instead of returning [u8; 64] to avoid function return value corruption on RV64.
/// See ai_plans/riscv-compiler-bugs.md for details.
pub fn bn254_ecmul_inner(x: &[u8; 32], y: &[u8; 32], scalar: &[u8; 32], out: &mut [u8; 64]) -> Result<(), ()> {
    use crypto::ark_ec::AffineRepr;
    use crypto::ark_ff::PrimeField;
    use crypto::ark_serialize::CanonicalDeserialize;
    use crypto::bn254::*;

    // Log input for debugging (unified logging for both RV32 and RV64)
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        use super::ecrecover::uart_log;
        uart_log::write_str("[bn254_ecmul] x=");
        uart_log::write_hex_slice(x);
        uart_log::newline();
        uart_log::write_str("[bn254_ecmul] y=");
        uart_log::write_hex_slice(y);
        uart_log::newline();
        uart_log::write_str("[bn254_ecmul] scalar=");
        uart_log::write_hex_slice(scalar);
        uart_log::newline();
    }

    let is_zero = x.iter().all(|el| *el == 0) && y.iter().all(|el| *el == 0);
    if is_zero {
        #[cfg(target_arch = "riscv64")]
        bn254_uart_str("[bn254_ecmul] is_zero=true, returning [0;64]\n");
        // Use volatile writes to avoid corruption
        for i in 0..64 {
            unsafe { core::ptr::write_volatile(&mut out[i], 0u8); }
        }
        return Ok(());
    }
    let mut x = *x;
    let mut y = *y;
    bytereverse(&mut x);
    bytereverse(&mut y);

    #[cfg(target_arch = "riscv64")]
    {
        bn254_uart_str("[bn254_ecmul] x_rev=");
        bn254_uart_hex_slice(&x);
        bn254_uart_str("\n");
        bn254_uart_str("[bn254_ecmul] y_rev=");
        bn254_uart_hex_slice(&y);
        bn254_uart_str("\n");
    }

    // COMPILER BUG WORKAROUND: Use volatile reads to prevent LLVM corruption
    // See ai_plans/riscv-compiler-bugs.md for details
    #[cfg(target_arch = "riscv64")]
    let (x_bigint, y_bigint) = {
        // Volatile read the bytes
        let mut x_vol = [0u8; 32];
        let mut y_vol = [0u8; 32];
        for i in 0..32 {
            unsafe {
                x_vol[i] = core::ptr::read_volatile(&x[i]);
                y_vol[i] = core::ptr::read_volatile(&y[i]);
            }
        }
        let x_bi = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&x_vol[..]).map_err(|_| ())?;
        let y_bi = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&y_vol[..]).map_err(|_| ())?;
        (x_bi, y_bi)
    };
    #[cfg(not(target_arch = "riscv64"))]
    let (x_bigint, y_bigint) = {
        let x_bi = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&x[..]).map_err(|_| ())?;
        let y_bi = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&y[..]).map_err(|_| ())?;
        (x_bi, y_bi)
    };

    // Log ALL BigInt limbs before Montgomery conversion
    #[cfg(target_arch = "riscv64")]
    {
        for i in 0..4 {
            bn254_uart_str("[bn254_ecmul] x_bigint limb");
            bn254_uart_hex_byte(i as u8);
            bn254_uart_str("=");
            let limb = x_bigint.0[i];
            for j in (0..8).rev() {
                bn254_uart_hex_byte(((limb >> (j * 8)) & 0xff) as u8);
            }
            bn254_uart_str("\n");
        }
    }

    // Log what Fq::ONE looks like (Montgomery form of 1)
    #[cfg(target_arch = "riscv64")]
    {
        use crypto::ark_ff::One;
        let one = Fq::one();
        bn254_uart_str("[bn254_ecmul] Fq::ONE (Montgomery)=");
        let one_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(one) };
        bn254_uart_hex_slice(&one_bytes);
        bn254_uart_str("\n");

        // Compute ONE * ONE using mul (different code path than square)
        let one_mul = one * one;
        bn254_uart_str("[bn254_ecmul] Fq::ONE*ONE (Montgomery)=");
        let mul_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(one_mul) };
        bn254_uart_hex_slice(&mul_bytes);
        bn254_uart_str("\n");

        // Test from_bigint with 1
        let test_bigint = <Fq as PrimeField>::BigInt::from(1u64);
        bn254_uart_str("[bn254_ecmul] BigInt(1) limb0=");
        for j in (0..8).rev() {
            bn254_uart_hex_byte(((test_bigint.0[0] >> (j * 8)) & 0xff) as u8);
        }
        bn254_uart_str("\n");

        // Now do from_bigint(1)
        if let Some(from_bi) = Fq::from_bigint(test_bigint) {
            bn254_uart_str("[bn254_ecmul] Fq::from_bigint(1)=");
            let bi_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(from_bi) };
            bn254_uart_hex_slice(&bi_bytes);
            bn254_uart_str("\n");

            // Compare
            bn254_uart_str("[bn254_ecmul] from_bigint(1) == ONE? ");
            if from_bi == one {
                bn254_uart_str("YES\n");
            } else {
                bn254_uart_str("NO (DIVERGED!)\n");
            }
        } else {
            bn254_uart_str("[bn254_ecmul] from_bigint(1) returned None!\n");
        }
    }

    // Trace the ORIGINAL code path - no modifications, just logging
    #[cfg(target_arch = "riscv64")]
    {
        // Log x_bigint limbs immediately before from_bigint
        bn254_uart_str("[TRACE] x_bigint.0 immediately before from_bigint:\n");
        for i in 0..4 {
            bn254_uart_str("  [");
            bn254_uart_hex_byte(i as u8);
            bn254_uart_str("]=");
            let limb = x_bigint.0[i];
            for j in (0..8).rev() {
                bn254_uart_hex_byte(((limb >> (j * 8)) & 0xff) as u8);
            }
            bn254_uart_str("\n");
        }
    }

    // ORIGINAL code path - unchanged
    let x_coordinate = Fq::from_bigint(x_bigint).ok_or_else(|| {
        #[cfg(target_arch = "riscv64")]
        bn254_uart_str("[bn254_ecmul] ERROR: x_coordinate from_bigint failed\n");
        ()
    })?;

    // COMPILER BUG WORKAROUND: Volatile refresh x_coordinate after from_bigint
    #[cfg(target_arch = "riscv64")]
    let x_coordinate = {
        use crypto::ark_ff::PrimeField;
        let mut fresh = Fq::from(0u64);
        unsafe {
            let src = &x_coordinate as *const Fq as *const u64;
            let dst = &mut fresh as *mut Fq as *mut u64;
            for i in 0..4 {
                let val = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), val);
            }
        }
        fresh
    };

    #[cfg(target_arch = "riscv64")]
    {
        // Log result immediately after from_bigint
        bn254_uart_str("[TRACE] x_coordinate after from_bigint:\n");
        let xc_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(x_coordinate) };
        bn254_uart_str("  raw=");
        bn254_uart_hex_slice(&xc_bytes);
        bn254_uart_str("\n");

        // Compare with expected
        use crypto::ark_ff::One;
        let expected = Fq::one();
        let exp_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(expected) };
        bn254_uart_str("[TRACE] expected (Fq::one()):\n");
        bn254_uart_str("  raw=");
        bn254_uart_hex_slice(&exp_bytes);
        bn254_uart_str("\n");
        bn254_uart_str("[TRACE] x_coordinate == expected? ");
        if x_coordinate == expected {
            bn254_uart_str("YES\n");
        } else {
            bn254_uart_str("NO - CORRUPTION\n");
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        bn254_uart_str("[TRACE] y_bigint.0 immediately before from_bigint:\n");
        for i in 0..4 {
            bn254_uart_str("  [");
            bn254_uart_hex_byte(i as u8);
            bn254_uart_str("]=");
            let limb = y_bigint.0[i];
            for j in (0..8).rev() {
                bn254_uart_hex_byte(((limb >> (j * 8)) & 0xff) as u8);
            }
            bn254_uart_str("\n");
        }
    }

    // COMPILER BUG WORKAROUND: Volatile refresh y_bigint before from_bigint
    // The compiler may have cached incorrect values for y_bigint
    #[cfg(target_arch = "riscv64")]
    let y_bigint = {
        let mut fresh = <Fq as PrimeField>::BigInt::from(0u64);
        for i in 0..4 {
            unsafe {
                let val = core::ptr::read_volatile(&y_bigint.0[i]);
                core::ptr::write_volatile(&mut fresh.0[i], val);
            }
        }
        fresh
    };

    let y_coordinate = Fq::from_bigint(y_bigint).ok_or_else(|| {
        #[cfg(target_arch = "riscv64")]
        bn254_uart_str("[bn254_ecmul] ERROR: y_coordinate from_bigint failed\n");
        ()
    })?;

    // COMPILER BUG WORKAROUND: Volatile refresh y_coordinate after from_bigint
    // The return value may have been corrupted during the return
    #[cfg(target_arch = "riscv64")]
    let y_coordinate = {
        use crypto::ark_ff::PrimeField;
        let mut fresh = Fq::from(0u64);
        unsafe {
            let src = &y_coordinate as *const Fq as *const u64;
            let dst = &mut fresh as *mut Fq as *mut u64;
            for i in 0..4 {
                let val = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), val);
            }
        }
        fresh
    };

    // Log the internal representation of coordinates
    #[cfg(target_arch = "riscv64")]
    {
        bn254_uart_str("[bn254_ecmul] x_coord (Montgomery form)=");
        let x_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(x_coordinate) };
        bn254_uart_hex_slice(&x_bytes);
        bn254_uart_str("\n");

        bn254_uart_str("[bn254_ecmul] y_coord (Montgomery form)=");
        let y_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(y_coordinate) };
        bn254_uart_hex_slice(&y_bytes);
        bn254_uart_str("\n");

        // Also log x*x to check if squaring works
        use crypto::ark_ff::Field;
        let x_squared = x_coordinate.square();
        bn254_uart_str("[bn254_ecmul] x^2 (Montgomery)=");
        let xs_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(x_squared) };
        bn254_uart_hex_slice(&xs_bytes);
        bn254_uart_str("\n");
    }

    let affine_point = G1Affine::new_unchecked(x_coordinate, y_coordinate);

    // Debug: manually check if the point is on the curve y^2 = x^3 + 3
    #[cfg(target_arch = "riscv64")]
    {
        use crypto::ark_ff::Field;
        let y_squared = y_coordinate.square();
        let x_cubed = x_coordinate * x_coordinate * x_coordinate;
        let b = Fq::from(3u64);
        let rhs = x_cubed + b;

        // Log the first limb of each value to see if arithmetic is correct
        bn254_uart_str("[bn254_ecmul] y^2 limb0=");
        let y2_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(y_squared) };
        bn254_uart_hex_slice(&y2_bytes[0..8]);
        bn254_uart_str("\n");

        bn254_uart_str("[bn254_ecmul] x^3+3 limb0=");
        let rhs_bytes = unsafe { core::mem::transmute::<_, [u8; 32]>(rhs) };
        bn254_uart_hex_slice(&rhs_bytes[0..8]);
        bn254_uart_str("\n");

        bn254_uart_str("[bn254_ecmul] y^2 == x^3+3? ");
        if y_squared == rhs {
            bn254_uart_str("YES\n");
        } else {
            bn254_uart_str("NO (MISMATCH!)\n");
        }
    }

    affine_point.check().map_err(|e| {
        #[cfg(target_arch = "riscv64")]
        bn254_uart_str("[bn254_ecmul] ERROR: affine_point.check() failed - InvalidPoint\n");
        ()
    })?;

    let mut scalar = *scalar;
    bytereverse(&mut scalar);

    // Log scalar after bytereverse
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        use super::ecrecover::uart_log;
        uart_log::write_str("[bn254_ecmul] scalar_rev=");
        uart_log::write_hex_slice(&scalar);
        uart_log::newline();
    }

    let scalar =
        <Fr as PrimeField>::BigInt::deserialize_uncompressed(&scalar[..]).map_err(|_| ())?;

    // Log scalar limbs
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        use super::ecrecover::uart_log;
        uart_log::write_str("[bn254_ecmul] scalar_limbs=");
        for limb in scalar.0.iter() {
            uart_log::write_hex_slice(&limb.to_le_bytes());
            uart_log::write_str(",");
        }
        uart_log::newline();
    }

    // Log affine point coordinates before multiplication
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        use super::ecrecover::uart_log;
        use crypto::ark_ff::PrimeField;
        let x_bi = affine_point.x.into_bigint();
        let y_bi = affine_point.y.into_bigint();
        uart_log::write_str("[bn254_ecmul] affine_x_limbs=");
        for limb in x_bi.0.iter() {
            uart_log::write_hex_slice(&limb.to_le_bytes());
            uart_log::write_str(",");
        }
        uart_log::newline();
        uart_log::write_str("[bn254_ecmul] affine_y_limbs=");
        for limb in y_bi.0.iter() {
            uart_log::write_hex_slice(&limb.to_le_bytes());
            uart_log::write_str(",");
        }
        uart_log::newline();
    }

    // DEBUG: Use custom double-and-add with logging on both RV32 and RV64 to compare
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    let result = {
        use crypto::ark_ec::CurveGroup;
        use crypto::ark_ff::Zero;
        use super::ecrecover::uart_log;

        fn log_projective_uart(prefix: &str, p: &crypto::bn254::G1Projective) {
            use crypto::ark_ff::PrimeField;
            use super::ecrecover::uart_log;
            uart_log::write_str(prefix);
            let xb = p.x.into_bigint();
            let yb = p.y.into_bigint();
            let zb = p.z.into_bigint();
            uart_log::write_str("x0=");
            uart_log::write_hex_slice(&xb.0[0].to_le_bytes()[0..4]);
            uart_log::write_str(" y0=");
            uart_log::write_hex_slice(&yb.0[0].to_le_bytes()[0..4]);
            uart_log::write_str(" z0=");
            uart_log::write_hex_slice(&zb.0[0].to_le_bytes()[0..4]);
            uart_log::newline();
        }

        let mut result = crypto::bn254::G1Projective::zero();

        // Log the affine point's internal Montgomery representation BEFORE into_group
        uart_log::write_str("[ecmul_debug] Affine x Montgomery (raw limb0): ");
        let x_ptr = &affine_point.x as *const _ as *const u64;
        let raw_x = unsafe { core::ptr::read_volatile(x_ptr) };
        uart_log::write_hex_slice(&raw_x.to_le_bytes());
        uart_log::newline();

        uart_log::write_str("[ecmul_debug] Affine y Montgomery (raw limb0): ");
        let y_ptr = &affine_point.y as *const _ as *const u64;
        let raw_y = unsafe { core::ptr::read_volatile(y_ptr) };
        uart_log::write_hex_slice(&raw_y.to_le_bytes());
        uart_log::newline();

        // COMPILER BUG WORKAROUND: Volatile copy of affine point before into_group()
        // to prevent corruption of coordinates during the conversion
        #[cfg(target_arch = "riscv64")]
        let base: crypto::bn254::G1Projective = {
            use crypto::ark_ec::CurveGroup;
            use crypto::ark_ff::Field;

            // Volatile copy the affine coordinates
            let mut x_limbs = [0u64; 4];
            let mut y_limbs = [0u64; 4];
            let x_ptr = &affine_point.x as *const _ as *const u64;
            let y_ptr = &affine_point.y as *const _ as *const u64;
            for i in 0..4 {
                unsafe {
                    x_limbs[i] = core::ptr::read_volatile(x_ptr.add(i));
                    y_limbs[i] = core::ptr::read_volatile(y_ptr.add(i));
                }
            }

            // Create fresh affine point from volatile-copied limbs
            let fresh_x = unsafe {
                core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(x_limbs)
            };
            let fresh_y = unsafe {
                core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(y_limbs)
            };
            let fresh_affine = crypto::bn254::G1Affine::new_unchecked(fresh_x, fresh_y);

            // Convert to projective
            let proj = fresh_affine.into_group();

            // Volatile copy the projective coordinates to prevent corruption on return
            let mut px_limbs = [0u64; 4];
            let mut py_limbs = [0u64; 4];
            let mut pz_limbs = [0u64; 4];
            let px_ptr = &proj.x as *const _ as *const u64;
            let py_ptr = &proj.y as *const _ as *const u64;
            let pz_ptr = &proj.z as *const _ as *const u64;
            for i in 0..4 {
                unsafe {
                    px_limbs[i] = core::ptr::read_volatile(px_ptr.add(i));
                    py_limbs[i] = core::ptr::read_volatile(py_ptr.add(i));
                    pz_limbs[i] = core::ptr::read_volatile(pz_ptr.add(i));
                }
            }

            // Create fresh projective point from volatile-copied limbs
            let fresh_px = unsafe {
                core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(px_limbs)
            };
            let fresh_py = unsafe {
                core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(py_limbs)
            };
            let fresh_pz = unsafe {
                core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(pz_limbs)
            };
            crypto::bn254::G1Projective::new_unchecked(fresh_px, fresh_py, fresh_pz)
        };

        #[cfg(not(target_arch = "riscv64"))]
        let base: crypto::bn254::G1Projective = affine_point.into_group();

        // Log the projective point's internal Montgomery representation AFTER into_group
        uart_log::write_str("[ecmul_debug] Projective x Montgomery (raw limb0): ");
        let px_ptr = &base.x as *const _ as *const u64;
        let raw_px = unsafe { core::ptr::read_volatile(px_ptr) };
        uart_log::write_hex_slice(&raw_px.to_le_bytes());
        uart_log::newline();

        uart_log::write_str("[ecmul_debug] Starting double-and-add");
        uart_log::newline();
        log_projective_uart("[ecmul_debug] base: ", &base);

        let mut first_nonzero_step = 0u32;
        let mut step_count = 0u32;

        // Simple double-and-add (MSB first)
        for i in (0..4).rev() {
            let limb = scalar.0[i];
            for j in (0..64).rev() {
                // Double
                result = result + &result;
                step_count += 1;

                if (limb >> j) & 1 == 1 {
                    // Add base
                    result = result + &base;
                    if first_nonzero_step == 0 {
                        first_nonzero_step = step_count;
                        uart_log::write_str("[ecmul_debug] First add at step ");
                        uart_log::write_hex_slice(&step_count.to_le_bytes());
                        uart_log::newline();
                        log_projective_uart("[ecmul_debug] After first add: ", &result);
                    }
                }

                // Log first few steps
                if step_count <= 5 {
                    uart_log::write_str("[ecmul_debug] Step ");
                    uart_log::write_hex_slice(&[step_count as u8]);
                    uart_log::write_str(": ");
                    log_projective_uart("", &result);
                }
            }
        }

        uart_log::write_str("[ecmul_debug] Total steps: ");
        uart_log::write_hex_slice(&step_count.to_le_bytes());
        uart_log::newline();
        log_projective_uart("[ecmul_debug] Final result: ", &result);

        result
    };

    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
    let result = affine_point.mul_bigint(&scalar);

    // Log projective point before serialization
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        use super::ecrecover::uart_log;
        use crypto::ark_ff::Zero;
        uart_log::write_str("[bn254_ecmul] result_is_zero=");
        if result.is_zero() {
            uart_log::write_str("true");
        } else {
            uart_log::write_str("false");
        }
        uart_log::newline();
    }

    let result = serialize_projective(result);

    // Log the OUTPUT of ecmul (unified logging for both RV32 and RV64)
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        use super::ecrecover::uart_log;
        uart_log::write_str("[bn254_ecmul] OUTPUT=");
        uart_log::write_hex_slice(&result);
        uart_log::newline();
    }

    // Write result to output using volatile writes to avoid corruption during function return
    for i in 0..64 {
        unsafe { core::ptr::write_volatile(&mut out[i], result[i]); }
    }

    Ok(())
}
