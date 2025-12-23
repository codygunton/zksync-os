use crate::k256::{elliptic_curve::subtle::Choice, CompressedPoint, EncodedPoint, FieldBytes};

use crate::secp256k1::field::{FieldElement, FieldElementConst};

use super::{jacobian::JacobianConst, AffineStorage, Jacobian};

#[derive(Debug, Clone, Copy)]
pub(crate) struct AffineConst {
    pub(crate) x: FieldElementConst,
    pub(crate) y: FieldElementConst,
    pub(crate) infinity: bool,
}

impl AffineConst {
    pub(crate) const INFINITY: Self = Self {
        x: FieldElementConst::ZERO,
        y: FieldElementConst::ZERO,
        infinity: true,
    };

    pub(crate) const GENERATOR: Self = Self {
        x: FieldElementConst::from_bytes_unchecked(&[
            0x79, 0xbe, 0x66, 0x7e, 0xf9, 0xdc, 0xbb, 0xac, 0x55, 0xa0, 0x62, 0x95, 0xce, 0x87,
            0x0b, 0x07, 0x02, 0x9b, 0xfc, 0xdb, 0x2d, 0xce, 0x28, 0xd9, 0x59, 0xf2, 0x81, 0x5b,
            0x16, 0xf8, 0x17, 0x98,
        ]),
        y: FieldElementConst::from_bytes_unchecked(&[
            0x48, 0x3a, 0xda, 0x77, 0x26, 0xa3, 0xc4, 0x65, 0x5d, 0xa4, 0xfb, 0xfc, 0x0e, 0x11,
            0x08, 0xa8, 0xfd, 0x17, 0xb4, 0x48, 0xa6, 0x85, 0x54, 0x19, 0x9c, 0x47, 0xd0, 0x8f,
            0xfb, 0x10, 0xd4, 0xb8,
        ]),
        infinity: false,
    };

    #[cfg(debug_assertions)]
    const X_MAGNITUDE_MAX: u32 = 4;
    #[cfg(debug_assertions)]
    const Y_MAGNITUDE_MAX: u32 = 4;

    #[inline(always)]
    pub(crate) const fn assert_verify(&self) {
        #[cfg(all(debug_assertions, not(feature = "bigint_ops")))]
        {
            debug_assert!(self.x.0.magnitude <= Self::X_MAGNITUDE_MAX);
            debug_assert!(self.x.0.magnitude <= 32);
            debug_assert!(self.y.0.magnitude <= Self::Y_MAGNITUDE_MAX);
            debug_assert!(self.y.0.magnitude <= 32);
        }
    }

    pub(crate) const fn is_infinity(&self) -> bool {
        self.infinity || (self.x.normalizes_to_zero() && self.y.normalizes_to_zero())
    }

    #[allow(unused_mut)]
    pub(crate) const fn to_storage(mut self) -> AffineStorage {
        debug_assert!(!self.is_infinity());

        AffineStorage {
            x: self.x.normalize().to_storage(),
            y: self.y.normalize().to_storage(),
        }
    }

    pub(crate) const fn to_jacobian(self) -> JacobianConst {
        if self.is_infinity() {
            return JacobianConst::INFINITY;
        }
        JacobianConst {
            x: self.x,
            y: self.y,
            z: FieldElementConst::ONE,
        }
    }
}

#[cfg(test)]
impl PartialEq for AffineConst {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.infinity == other.infinity
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Affine {
    pub(crate) x: FieldElement,
    pub(crate) y: FieldElement,
    pub(crate) infinity: bool,
}

impl Affine {
    pub(crate) const DEFAULT: Self = Self {
        x: FieldElement::ZERO,
        y: FieldElement::ZERO,
        infinity: false,
    };

    pub(crate) const INFINITY: Self = Self {
        x: FieldElement::ZERO,
        y: FieldElement::ZERO,
        infinity: true,
    };

    #[cfg(test)]
    pub(crate) const GENERATOR: Self = Self {
        x: FieldElement::from_bytes_unchecked(&[
            0x79, 0xbe, 0x66, 0x7e, 0xf9, 0xdc, 0xbb, 0xac, 0x55, 0xa0, 0x62, 0x95, 0xce, 0x87,
            0x0b, 0x07, 0x02, 0x9b, 0xfc, 0xdb, 0x2d, 0xce, 0x28, 0xd9, 0x59, 0xf2, 0x81, 0x5b,
            0x16, 0xf8, 0x17, 0x98,
        ]),
        y: FieldElement::from_bytes_unchecked(&[
            0x48, 0x3a, 0xda, 0x77, 0x26, 0xa3, 0xc4, 0x65, 0x5d, 0xa4, 0xfb, 0xfc, 0x0e, 0x11,
            0x08, 0xa8, 0xfd, 0x17, 0xb4, 0x48, 0xa6, 0x85, 0x54, 0x19, 0x9c, 0x47, 0xd0, 0x8f,
            0xfb, 0x10, 0xd4, 0xb8,
        ]),
        infinity: false,
    };

    #[cfg(debug_assertions)]
    const X_MAGNITUDE_MAX: u32 = 4;
    #[cfg(debug_assertions)]
    const Y_MAGNITUDE_MAX: u32 = 4;

    #[inline(always)]
    pub(crate) const fn assert_verify(&self) {
        #[cfg(all(debug_assertions, not(feature = "bigint_ops")))]
        {
            debug_assert!(self.x.0.magnitude <= Self::X_MAGNITUDE_MAX);
            debug_assert!(self.x.0.magnitude <= 32);
            debug_assert!(self.y.0.magnitude <= Self::Y_MAGNITUDE_MAX);
            debug_assert!(self.y.0.magnitude <= 32);
        }
    }

    pub fn is_infinity(&self) -> bool {
        self.infinity || (self.x.normalizes_to_zero() && self.y.normalizes_to_zero())
    }

    pub(crate) fn decompress(x_bytes: &FieldBytes, y_is_odd: bool) -> Option<Self> {
        #[allow(deprecated)]
        let len = x_bytes.as_slice().len();
        debug_assert!(len == 32);

        #[allow(deprecated)]
        x_bytes.as_slice().try_into().ok().and_then(|x| {
            let x = FieldElement::from_bytes(x)?;
            let mut ret = Affine::DEFAULT;
            if ret.set_xo(&x, y_is_odd) {
                Some(ret)
            } else {
                None
            }
        })
    }

    fn set_xo(&mut self, x: &FieldElement, y_is_odd: bool) -> bool {
        self.y = *x;
        self.y.square_in_place();
        self.y *= x;
        self.y += 7;

        let ret = self.y.sqrt_in_place();
        self.y.normalize_in_place();

        if self.y.is_odd() != y_is_odd {
            self.y.negate_in_place(1);
        }

        self.x = *x;
        self.infinity = false;

        ret
    }

    pub(crate) fn normalize_in_place(&mut self) {
        self.x.normalize_in_place();
        self.y.normalize_in_place();
    }

    pub(crate) fn set_gej_zinv(&mut self, a: &Jacobian, z: &FieldElement) {
        a.assert_verify();

        if a.is_infinity() {
            *self = Self::INFINITY;
        } else {
            self.set_ge_zinv(
                &Affine {
                    x: a.x,
                    y: a.y,
                    infinity: false,
                },
                z,
            );
        }
    }

    pub(crate) fn set_ge_zinv(&mut self, a: &Affine, z: &FieldElement) {
        a.assert_verify();

        let mut z2 = *z;
        z2.square_in_place();

        let mut z3 = z2;
        z3 *= z;

        self.x = a.x;
        self.x *= z2;

        self.y = a.y;
        self.y *= z3;

        self.infinity = a.infinity;
    }

    pub(crate) fn to_jacobian(self) -> Jacobian {
        if self.is_infinity() {
            return Jacobian::INFINITY;
        }
        Jacobian {
            x: self.x,
            y: self.y,
            z: FieldElement::ONE,
        }
    }

    pub fn to_encoded_point(self, compress: bool) -> EncodedPoint {
        let x_bytes = self.x.to_bytes();
        let y_bytes = self.y.to_bytes();

        // Manually construct encoded point bytes using volatile writes to prevent
        // compiler optimization issues on RISC-V
        if self.is_infinity() {
            EncodedPoint::identity()
        } else if compress {
            // Compressed format: 0x02/0x03 + 32 bytes x
            let mut buf = [0u8; 33];
            let tag = if self.y.is_odd() { 0x03u8 } else { 0x02u8 };
            unsafe {
                core::ptr::write_volatile(&mut buf[0], tag);
                for i in 0..32 {
                    core::ptr::write_volatile(&mut buf[1 + i], x_bytes[i]);
                }
            }
            EncodedPoint::from_bytes(&buf).expect("valid compressed point")
        } else {
            // Uncompressed format: 0x04 + 32 bytes x + 32 bytes y
            let mut buf = [0u8; 65];
            unsafe {
                core::ptr::write_volatile(&mut buf[0], 0x04u8);
                for i in 0..32 {
                    core::ptr::write_volatile(&mut buf[1 + i], x_bytes[i]);
                }
                for i in 0..32 {
                    core::ptr::write_volatile(&mut buf[33 + i], y_bytes[i]);
                }
            }
            EncodedPoint::from_bytes(&buf).expect("valid uncompressed point")
        }
    }

    /// Writes the uncompressed SEC1-encoded point to the output buffer.
    /// Uses volatile writes throughout to prevent compiler optimization issues on RISC-V.
    /// Format: 0x04 || x (32 bytes) || y (32 bytes)
    /// Writes all zeros if this is the point at infinity.
    pub fn write_uncompressed_bytes(self, out: &mut [u8; 65]) {
        if self.is_infinity() {
            unsafe {
                for i in 0..65 {
                    core::ptr::write_volatile(&mut out[i], 0u8);
                }
            }
            return;
        }

        // Write tag byte with volatile
        unsafe {
            core::ptr::write_volatile(&mut out[0], 0x04u8);
        }

        // Write x and y bytes directly to output buffer
        // This avoids intermediate local arrays that get corrupted on return
        // Use split_at_mut to avoid borrow checker issues
        let (_, rest) = out.split_at_mut(1);
        let (x_part, y_part) = rest.split_at_mut(32);
        let x_slice: &mut [u8; 32] = x_part.try_into().unwrap();
        let y_slice: &mut [u8; 32] = y_part.try_into().unwrap();
        self.x.write_bytes_to(x_slice);
        self.y.write_bytes_to(y_slice);
    }

    /// Returns the uncompressed SEC1-encoded point as raw bytes.
    /// Uses volatile writes throughout to prevent compiler optimization issues on RISC-V.
    /// Returns `[u8; 65]` with format: 0x04 || x (32 bytes) || y (32 bytes)
    /// Returns all zeros if this is the point at infinity.
    pub fn to_uncompressed_bytes(self) -> [u8; 65] {
        let mut buf = [0u8; 65];
        self.write_uncompressed_bytes(&mut buf);
        buf
    }

    pub fn to_bytes(self) -> CompressedPoint {
        let encoded = self.to_encoded_point(true);
        let mut result = CompressedPoint::default();
        result[..encoded.len()].copy_from_slice(encoded.as_bytes());
        result
    }
}

#[cfg(test)]
impl PartialEq for Affine {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.infinity == other.infinity
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for Affine {
    type Parameters = ();

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<FieldElement>().prop_map(|x| {
            let mut ret = Affine::DEFAULT;
            ret.set_xo(&x, true);

            ret
        })
    }

    type Strategy = proptest::arbitrary::Mapped<FieldElement, Self>;
}

#[cfg(test)]
mod tests {
    use super::Affine;

    use proptest::{prop_assert_eq, proptest};

    #[test]
    fn test_set_xo() {
        let g = Affine::GENERATOR;
        let x = g.x;
        let y_is_odd = false;
        let mut a = Affine::DEFAULT;
        a.set_xo(&x, y_is_odd);
        assert_eq!(a, g);
    }

    #[test]
    fn jacobian_round_trip() {
        proptest!(|(x: Affine)| {
            prop_assert_eq!(x.to_jacobian().to_affine(), x);
        });
    }
}
