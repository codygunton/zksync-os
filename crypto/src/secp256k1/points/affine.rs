use crate::k256::{elliptic_curve::subtle::Choice, CompressedPoint, EncodedPoint, FieldBytes};

use crate::secp256k1::field::{FieldElement, FieldElementConst};
use crate::secp256k1::uart_log;

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

    pub unsafe fn from_xy_unchecked(x: FieldElement, y: FieldElement) -> Self {
        Self {
            x,
            y,
            infinity: false,
        }
    }

    pub(crate) fn decompress(x_bytes: &FieldBytes, y_is_odd: bool) -> Option<Self> {
        debug_assert!(x_bytes.as_slice().len() == 32);

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

    /// Decompress with logging to trace corruption
    pub(crate) fn decompress_with_logging(x_bytes: &FieldBytes, y_is_odd: bool, tx_num: usize) -> Option<Self> {
        #[allow(deprecated)]
        let len = x_bytes.as_slice().len();
        debug_assert!(len == 32);

        uart_log::write_str("[decompress] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" input x_bytes=");
        #[allow(deprecated)]
        uart_log::write_hex_slice(x_bytes.as_slice());
        uart_log::newline();

        #[allow(deprecated)]
        let x_array: &[u8; 32] = x_bytes.as_slice().try_into().ok()?;

        uart_log::write_str("[decompress] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" x_array=");
        uart_log::write_hex_slice(x_array);
        uart_log::newline();

        let x = FieldElement::from_bytes(x_array)?;

        // Log the field element right after from_bytes
        let x_back = x.to_bytes();
        uart_log::write_str("[decompress] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" x.to_bytes() after from_bytes=");
        uart_log::write_hex_slice(&x_back);
        uart_log::newline();

        let mut ret = Affine::DEFAULT;

        uart_log::write_str("[decompress] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" calling set_xo");
        uart_log::newline();

        if ret.set_xo(&x, y_is_odd) {
            // Log after set_xo
            let ret_x = ret.x.to_bytes();
            uart_log::write_str("[decompress] TX#");
            uart_log::write_usize(tx_num);
            uart_log::write_str(" ret.x.to_bytes() after set_xo=");
            uart_log::write_hex_slice(&ret_x);
            uart_log::newline();

            Some(ret)
        } else {
            uart_log::write_str("[decompress] TX#");
            uart_log::write_usize(tx_num);
            uart_log::write_str(" set_xo returned false");
            uart_log::newline();
            None
        }
    }

    /// Decompress that writes to output parameter to avoid return value corruption.
    /// Returns true on success, false on failure.
    pub(crate) fn decompress_to(x_bytes: &FieldBytes, y_is_odd: bool, out: &mut Self) -> bool {
        Self::decompress_to_impl(x_bytes, y_is_odd, out, None)
    }

    /// Decompress with optional logging for debugging
    pub(crate) fn decompress_to_with_logging(x_bytes: &FieldBytes, y_is_odd: bool, out: &mut Self, tx_num: usize) -> bool {
        Self::decompress_to_impl(x_bytes, y_is_odd, out, Some(tx_num))
    }

    fn decompress_to_impl(x_bytes: &FieldBytes, y_is_odd: bool, out: &mut Self, tx_num: Option<usize>) -> bool {
        #[allow(deprecated)]
        let len = x_bytes.as_slice().len();
        debug_assert!(len == 32);

        // Use volatile reads to copy bytes to local buffer to prevent
        // RISC-V 64-bit compiler optimization bugs
        #[allow(deprecated)]
        let slice = x_bytes.as_slice();
        let mut local_bytes = [0u8; 32];
        for i in 0..32 {
            unsafe {
                local_bytes[i] = core::ptr::read_volatile(&slice[i]);
            }
        }

        // Use from_bytes_to to avoid Copy trait corruption on RISC-V 64-bit
        let mut x = FieldElement::ZERO;
        if !FieldElement::from_bytes_to(&local_bytes, &mut x) {
            return false;
        }

        // Use volatile writes to prevent compiler optimization issues
        unsafe {
            core::ptr::write_volatile(&mut out.x, FieldElement::ZERO);
            core::ptr::write_volatile(&mut out.y, FieldElement::ZERO);
            core::ptr::write_volatile(&mut out.infinity, false);
        }

        if let Some(tx) = tx_num {
            if out.set_xo_with_logging(&x, y_is_odd, tx) {
                true
            } else {
                false
            }
        } else {
            if out.set_xo(&x, y_is_odd) {
                true
            } else {
                false
            }
        }
    }

    fn set_xo(&mut self, x: &FieldElement, y_is_odd: bool) -> bool {
        // Use volatile copy to prevent RISC-V 64-bit compiler optimization bugs
        self.y.volatile_copy_from(x);
        self.y.square_in_place();
        self.y *= x;
        self.y += 7;

        let ret = self.y.sqrt_in_place();
        self.y.normalize_in_place();

        if self.y.is_odd() != y_is_odd {
            self.y.negate_in_place(1);
        }

        // Use volatile copy to prevent RISC-V 64-bit compiler optimization bugs
        self.x.volatile_copy_from(x);
        self.infinity = false;

        ret
    }

    /// set_xo with logging for debugging
    fn set_xo_with_logging(&mut self, x: &FieldElement, y_is_odd: bool, tx_num: usize) -> bool {
        uart_log::write_str("[set_xo] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" input x=");
        uart_log::write_hex_slice(&x.to_bytes());
        uart_log::newline();

        // Use volatile copy to prevent RISC-V 64-bit compiler optimization bugs
        self.y.volatile_copy_from(x);
        let y_after_assign = self.y.to_bytes();
        uart_log::write_str("[set_xo] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" after y=x: ");
        uart_log::write_hex_slice(&y_after_assign);
        uart_log::newline();

        self.y.square_in_place();
        let y_after_sq = self.y.to_bytes();
        uart_log::write_str("[set_xo] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" after y.square: ");
        uart_log::write_hex_slice(&y_after_sq);
        uart_log::newline();

        self.y *= x;
        let y_after_mul = self.y.to_bytes();
        uart_log::write_str("[set_xo] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" after y*=x (x^3): ");
        uart_log::write_hex_slice(&y_after_mul);
        uart_log::newline();

        self.y += 7;
        let y_after_add = self.y.to_bytes();
        uart_log::write_str("[set_xo] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" after y+=7 (x^3+7): ");
        uart_log::write_hex_slice(&y_after_add);
        uart_log::newline();

        let ret = self.y.sqrt_in_place();
        let y_after_sqrt = self.y.to_bytes();
        uart_log::write_str("[set_xo] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" after sqrt: ");
        uart_log::write_hex_slice(&y_after_sqrt);
        uart_log::write_str(" ret=");
        uart_log::write_usize(ret as usize);
        uart_log::newline();

        self.y.normalize_in_place();
        let y_after_norm = self.y.to_bytes();
        uart_log::write_str("[set_xo] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" after normalize: ");
        uart_log::write_hex_slice(&y_after_norm);
        uart_log::newline();

        let is_odd = self.y.is_odd();
        uart_log::write_str("[set_xo] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" y.is_odd=");
        uart_log::write_usize(is_odd as usize);
        uart_log::write_str(" y_is_odd=");
        uart_log::write_usize(y_is_odd as usize);
        uart_log::newline();

        if is_odd != y_is_odd {
            self.y.negate_in_place(1);
            let y_after_neg = self.y.to_bytes();
            uart_log::write_str("[set_xo] TX#");
            uart_log::write_usize(tx_num);
            uart_log::write_str(" after negate: ");
            uart_log::write_hex_slice(&y_after_neg);
            uart_log::newline();
        }

        // Use volatile copy to prevent RISC-V 64-bit compiler optimization bugs
        self.x.volatile_copy_from(x);
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

    pub const fn to_jacobian(self) -> Jacobian {
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

    /// Writes the uncompressed SEC1-encoded point with extensive logging.
    /// Used for debugging RISC-V corruption issues.
    pub fn write_uncompressed_bytes_with_logging(self, out: &mut [u8; 65], tx_num: usize) {
        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" write_uncompressed_bytes_with_logging");
        uart_log::newline();

        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" is_infinity=");
        uart_log::write_usize(self.is_infinity() as usize);
        uart_log::newline();

        if self.is_infinity() {
            unsafe {
                for i in 0..65 {
                    core::ptr::write_volatile(&mut out[i], 0u8);
                }
            }
            uart_log::write_str("[affine] TX#");
            uart_log::write_usize(tx_num);
            uart_log::write_str(" wrote zeros (infinity)");
            uart_log::newline();
            return;
        }

        // Log the field element values before conversion
        let x_bytes_before = self.x.to_bytes();
        let y_bytes_before = self.y.to_bytes();
        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" self.x.to_bytes()=");
        uart_log::write_hex_slice(&x_bytes_before);
        uart_log::newline();

        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" self.y.to_bytes()=");
        uart_log::write_hex_slice(&y_bytes_before);
        uart_log::newline();

        // Write tag byte with volatile
        unsafe {
            core::ptr::write_volatile(&mut out[0], 0x04u8);
        }

        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" wrote tag 0x04");
        uart_log::newline();

        // Write x and y bytes directly to output buffer
        let (_, rest) = out.split_at_mut(1);
        let (x_part, y_part) = rest.split_at_mut(32);
        let x_slice: &mut [u8; 32] = x_part.try_into().unwrap();
        let y_slice: &mut [u8; 32] = y_part.try_into().unwrap();

        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" calling x.write_bytes_to");
        uart_log::newline();

        self.x.write_bytes_to(x_slice);

        // Log what was written to x
        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" x_slice after write=");
        uart_log::write_hex_slice(x_slice);
        uart_log::newline();

        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" calling y.write_bytes_to");
        uart_log::newline();

        self.y.write_bytes_to(y_slice);

        // Log what was written to y
        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" y_slice after write=");
        uart_log::write_hex_slice(y_slice);
        uart_log::newline();

        // Final check: read back the entire output buffer
        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" final out[0..9]=");
        uart_log::write_hex_slice(&out[0..9]);
        uart_log::newline();

        uart_log::write_str("[affine] TX#");
        uart_log::write_usize(tx_num);
        uart_log::write_str(" final out[33..41]=");
        uart_log::write_hex_slice(&out[33..41]);
        uart_log::newline();
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
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        let g = Affine::GENERATOR;
        let x = g.x;
        let y_is_odd = false;
        let mut a = Affine::DEFAULT;
        a.set_xo(&x, y_is_odd);
        assert_eq!(a, g);
    }

    #[test]
    fn jacobian_round_trip() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        proptest!(|(x: Affine)| {
            prop_assert_eq!(x.to_jacobian().to_affine(), x);
        });
    }
}
