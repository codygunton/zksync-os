use super::*;
use crate::cost_constants::{BN254_ECADD_COST_ERGS, BN254_ECADD_NATIVE_COST};
use crate::system_functions::bytereverse;
use crypto::ark_ec::CurveGroup;
use crypto::ark_ff::PrimeField;
use crypto::ark_serialize::{CanonicalSerialize, Valid};
use zk_ee::common_traits::TryExtend;
use zk_ee::system::base_system_functions::{
    Bn254AddErrors, Bn254AddInterfaceError, SystemFunction,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::{interface_error, out_of_return_memory};

///
/// bn254 ecadd system function implementation.
///
pub struct Bn254AddImpl;

impl<R: Resources> SystemFunction<R, Bn254AddErrors> for Bn254AddImpl {
    /// Returns the size in bytes of output.
    ///
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    ///
    /// If output len less than needed(64) returns `InternalError`.
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error only if failed to create affine points from inputs.
    fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Bn254AddErrors>> {
        cycle_marker::wrap_with_resources!("bn254_ecadd", resources, {
            bn254_ecadd_as_system_function_inner(src, dst, resources)
        })
    }
}

fn bn254_ecadd_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SubsystemError<Bn254AddErrors>> {
    resources.charge(&R::from_ergs_and_native(
        BN254_ECADD_COST_ERGS,
        <R::Native as zk_ee::system::Computational>::from_computational(BN254_ECADD_NATIVE_COST),
    ))?;

    let mut buffer = [0u8; 128];
    for (dst, src) in buffer.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    let coordinates = buffer.as_chunks::<64>().0.try_into().unwrap();

    // Use out-parameter to avoid function return value corruption on RV64
    let mut serialized_result = [0u8; 64];
    bn254_ecadd_inner(coordinates, &mut serialized_result).map_err(|_| -> SubsystemError<_> {
        interface_error!(Bn254AddInterfaceError::InvalidPoint)
    })?;

    dst.try_extend(serialized_result)
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

/// Process a single point - riscv64 uses volatile workarounds for compiler bug, others use original logic
#[inline(never)]
fn process_single_point(xy: &[u8; 64]) -> Result<crypto::bn254::G1Affine, ()> {
    use crypto::ark_ec::AffineRepr;
    use crypto::ark_ff::PrimeField;
    use crypto::ark_serialize::CanonicalDeserialize;
    use crypto::bn254::*;

    let [mut x, mut y]: [[u8; 32]; 2] = xy.as_chunks::<32>().0.try_into().unwrap();

    let is_zero = x.iter().all(|el| *el == 0) && y.iter().all(|el| *el == 0);
    if is_zero {
        return Ok(G1Affine::identity());
    }

    bytereverse(&mut x);
    bytereverse(&mut y);

    // Non-riscv64: Original simple logic (includes x86_64 native and riscv32)
    #[cfg(not(target_arch = "riscv64"))]
    {
        let x_bigint =
            <Fq as PrimeField>::BigInt::deserialize_uncompressed(&x[..]).map_err(|_| ())?;
        let y_bigint =
            <Fq as PrimeField>::BigInt::deserialize_uncompressed(&y[..]).map_err(|_| ())?;
        let x_coordinate = Fq::from_bigint(x_bigint).ok_or(())?;
        let y_coordinate = Fq::from_bigint(y_bigint).ok_or(())?;
        let affine_point = G1Affine::new_unchecked(x_coordinate, y_coordinate);
        affine_point.check().map_err(|_| ())?;
        return Ok(affine_point);
    }

    // riscv64: Volatile workarounds for LLVM compiler bugs
    // These prevent data corruption in field arithmetic operations
    #[cfg(target_arch = "riscv64")]
    {
        // WORKAROUND: Use volatile reads to prevent LLVM corruption of byte arrays
        let (x_bigint, y_bigint) = {
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

        // WORKAROUND: Volatile refresh bigints before from_bigint
        let (x_bigint, y_bigint) = {
            let mut x_fresh = <Fq as PrimeField>::BigInt::from(0u64);
            let mut y_fresh = <Fq as PrimeField>::BigInt::from(0u64);
            for i in 0..4 {
                unsafe {
                    let xv = core::ptr::read_volatile(&x_bigint.0[i]);
                    core::ptr::write_volatile(&mut x_fresh.0[i], xv);
                    let yv = core::ptr::read_volatile(&y_bigint.0[i]);
                    core::ptr::write_volatile(&mut y_fresh.0[i], yv);
                }
            }
            (x_fresh, y_fresh)
        };

        let x_coordinate = Fq::from_bigint(x_bigint).ok_or(())?;

        // WORKAROUND: Volatile refresh coordinate after from_bigint
        let x_coordinate = {
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

        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

        let y_coordinate = Fq::from_bigint(y_bigint).ok_or(())?;

        // WORKAROUND: Volatile refresh coordinate after from_bigint
        let y_coordinate = {
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

        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

        let affine_point = G1Affine::new_unchecked(x_coordinate, y_coordinate);

        // WORKAROUND: Custom curve check using volatile-refreshed arithmetic
        // The built-in check() may use corrupted intermediate values
        let is_on_curve = {
            use crypto::ark_ff::Field;

            // Helper to volatile-refresh an Fq value
            let volatile_refresh = |val: Fq| -> Fq {
                let mut fresh = Fq::from(0u64);
                unsafe {
                    let src = &val as *const Fq as *const u64;
                    let dst = &mut fresh as *mut Fq as *mut u64;
                    for i in 0..4 {
                        let v = core::ptr::read_volatile(src.add(i));
                        core::ptr::write_volatile(dst.add(i), v);
                    }
                }
                fresh
            };

            let y_sq = volatile_refresh(y_coordinate.square());
            let x_sq = volatile_refresh(x_coordinate.square());
            let x_cu = volatile_refresh(x_sq * x_coordinate);
            let b = Fq::from(3u64);
            let rhs = volatile_refresh(x_cu + b);

            y_sq == rhs
        };

        if is_on_curve {
            Ok(affine_point)
        } else {
            Err(())
        }
    }
}

/// Computes BN254 point addition and writes result to `out` using volatile writes.
/// Uses out-parameter instead of returning [u8; 64] to avoid function return value corruption on RV64.
/// See ai_plans/riscv-compiler-bugs.md for details.
pub fn bn254_ecadd_inner(coordinates: &[[u8; 64]; 2], out: &mut [u8; 64]) -> Result<(), ()> {
    use crypto::ark_ec::AffineRepr;
    use crypto::bn254::*;

    // Process each point using the extracted function to avoid loop optimization issues
    let point0 = process_single_point(&coordinates[0])?;
    let point1 = process_single_point(&coordinates[1])?;

    let [a, b] = [point0, point1];

    // COMPILER BUG WORKAROUND: Volatile copy before into_group() and point addition
    #[cfg(target_arch = "riscv64")]
    let result = {
        use crypto::ark_ec::CurveGroup;

        // Volatile copy a's coordinates
        let mut ax_limbs = [0u64; 4];
        let mut ay_limbs = [0u64; 4];
        let ax_ptr = &a.x as *const _ as *const u64;
        let ay_ptr = &a.y as *const _ as *const u64;
        for i in 0..4 {
            unsafe {
                ax_limbs[i] = core::ptr::read_volatile(ax_ptr.add(i));
                ay_limbs[i] = core::ptr::read_volatile(ay_ptr.add(i));
            }
        }
        let fresh_ax = unsafe { core::mem::transmute::<[u64; 4], Fq>(ax_limbs) };
        let fresh_ay = unsafe { core::mem::transmute::<[u64; 4], Fq>(ay_limbs) };
        let fresh_a = G1Affine::new_unchecked(fresh_ax, fresh_ay);

        // Volatile copy b's coordinates
        let mut bx_limbs = [0u64; 4];
        let mut by_limbs = [0u64; 4];
        let bx_ptr = &b.x as *const _ as *const u64;
        let by_ptr = &b.y as *const _ as *const u64;
        for i in 0..4 {
            unsafe {
                bx_limbs[i] = core::ptr::read_volatile(bx_ptr.add(i));
                by_limbs[i] = core::ptr::read_volatile(by_ptr.add(i));
            }
        }
        let fresh_bx = unsafe { core::mem::transmute::<[u64; 4], Fq>(bx_limbs) };
        let fresh_by = unsafe { core::mem::transmute::<[u64; 4], Fq>(by_limbs) };
        let fresh_b = G1Affine::new_unchecked(fresh_bx, fresh_by);

        // Convert to projective
        let proj_a = fresh_a.into_group();

        // Volatile copy projective coordinates
        let mut px_limbs = [0u64; 4];
        let mut py_limbs = [0u64; 4];
        let mut pz_limbs = [0u64; 4];
        let px_ptr = &proj_a.x as *const _ as *const u64;
        let py_ptr = &proj_a.y as *const _ as *const u64;
        let pz_ptr = &proj_a.z as *const _ as *const u64;
        for i in 0..4 {
            unsafe {
                px_limbs[i] = core::ptr::read_volatile(px_ptr.add(i));
                py_limbs[i] = core::ptr::read_volatile(py_ptr.add(i));
                pz_limbs[i] = core::ptr::read_volatile(pz_ptr.add(i));
            }
        }
        let fresh_px = unsafe { core::mem::transmute::<[u64; 4], Fq>(px_limbs) };
        let fresh_py = unsafe { core::mem::transmute::<[u64; 4], Fq>(py_limbs) };
        let fresh_pz = unsafe { core::mem::transmute::<[u64; 4], Fq>(pz_limbs) };
        let mut result = G1Projective::new_unchecked(fresh_px, fresh_py, fresh_pz);

        // Add b
        result += &fresh_b;

        // Volatile copy result
        let mut rx_limbs = [0u64; 4];
        let mut ry_limbs = [0u64; 4];
        let mut rz_limbs = [0u64; 4];
        let rx_ptr = &result.x as *const _ as *const u64;
        let ry_ptr = &result.y as *const _ as *const u64;
        let rz_ptr = &result.z as *const _ as *const u64;
        for i in 0..4 {
            unsafe {
                rx_limbs[i] = core::ptr::read_volatile(rx_ptr.add(i));
                ry_limbs[i] = core::ptr::read_volatile(ry_ptr.add(i));
                rz_limbs[i] = core::ptr::read_volatile(rz_ptr.add(i));
            }
        }
        let fresh_rx = unsafe { core::mem::transmute::<[u64; 4], Fq>(rx_limbs) };
        let fresh_ry = unsafe { core::mem::transmute::<[u64; 4], Fq>(ry_limbs) };
        let fresh_rz = unsafe { core::mem::transmute::<[u64; 4], Fq>(rz_limbs) };
        let fresh_result = G1Projective::new_unchecked(fresh_rx, fresh_ry, fresh_rz);

        serialize_projective(fresh_result)
    };

    #[cfg(not(target_arch = "riscv64"))]
    let result = {
        let mut result: G1Projective = a.into_group();
        result += &b;
        serialize_projective(result)
    };

    // Write result to output using volatile writes to avoid corruption during function return
    for i in 0..64 {
        unsafe { core::ptr::write_volatile(&mut out[i], result[i]); }
    }

    Ok(())
}

pub(crate) fn serialize_projective(point: crypto::bn254::G1Projective) -> [u8; 64] {
    use crypto::ark_ec::AffineRepr;
    use crypto::ark_ff::Zero;
    if point.is_zero() {
        // canonical for zero point
        [0u8; 64]
    } else {
        // COMPILER BUG WORKAROUND: Volatile copy projective point before into_affine()
        #[cfg(target_arch = "riscv64")]
        let point = {
            let mut x_limbs = [0u64; 4];
            let mut y_limbs = [0u64; 4];
            let mut z_limbs = [0u64; 4];
            let x_ptr = &point.x as *const _ as *const u64;
            let y_ptr = &point.y as *const _ as *const u64;
            let z_ptr = &point.z as *const _ as *const u64;
            for i in 0..4 {
                unsafe {
                    x_limbs[i] = core::ptr::read_volatile(x_ptr.add(i));
                    y_limbs[i] = core::ptr::read_volatile(y_ptr.add(i));
                    z_limbs[i] = core::ptr::read_volatile(z_ptr.add(i));
                }
            }
            let fresh_x = unsafe { core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(x_limbs) };
            let fresh_y = unsafe { core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(y_limbs) };
            let fresh_z = unsafe { core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(z_limbs) };
            crypto::bn254::G1Projective::new_unchecked(fresh_x, fresh_y, fresh_z)
        };

        let affine_result = point.into_affine();

        // COMPILER BUG WORKAROUND: Volatile copy affine coordinates after into_affine()
        #[cfg(target_arch = "riscv64")]
        let (x_bigint, y_bigint) = {
            let mut x_limbs = [0u64; 4];
            let mut y_limbs = [0u64; 4];
            let x_ptr = &affine_result.x as *const _ as *const u64;
            let y_ptr = &affine_result.y as *const _ as *const u64;
            for i in 0..4 {
                unsafe {
                    x_limbs[i] = core::ptr::read_volatile(x_ptr.add(i));
                    y_limbs[i] = core::ptr::read_volatile(y_ptr.add(i));
                }
            }
            let fresh_x = unsafe { core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(x_limbs) };
            let fresh_y = unsafe { core::mem::transmute::<[u64; 4], crypto::bn254::Fq>(y_limbs) };
            (fresh_x.into_bigint(), fresh_y.into_bigint())
        };

        #[cfg(not(target_arch = "riscv64"))]
        let (x_bigint, y_bigint) = {
            let (x, y) = affine_result.xy().unwrap();
            (x.into_bigint(), y.into_bigint())
        };
        let mut result = [0u8; 64];
        x_bigint.serialize_uncompressed(&mut result[0..32]).unwrap();
        bytereverse(&mut result[0..32]);
        y_bigint
            .serialize_uncompressed(&mut result[32..64])
            .unwrap();
        bytereverse(&mut result[32..64]);

        result
    }
}
