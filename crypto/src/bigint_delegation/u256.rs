use super::{delegation, DelegatedBarretParams, DelegatedModParams, DelegatedMontParams};
use crate::ark_ff_delegation::{BigInt, BigInteger};
use core::{fmt::Debug, marker::PhantomData};

/// UART helpers for debugging (RV64 only)
#[cfg(target_arch = "riscv64")]
fn u256_uart_byte(b: u8) {
    unsafe { core::ptr::write_volatile(0xa000_0200u64 as *mut u8, b); }
}
#[cfg(target_arch = "riscv64")]
fn u256_uart_str(s: &str) {
    for b in s.bytes() { u256_uart_byte(b); }
}
#[cfg(target_arch = "riscv64")]
fn u256_uart_hex_u64(v: u64) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for i in (0..8).rev() {
        let b = ((v >> (i * 8)) & 0xff) as u8;
        u256_uart_byte(HEX[(b >> 4) as usize]);
        u256_uart_byte(HEX[(b & 0xf) as usize]);
    }
}
#[cfg(target_arch = "riscv64")]
fn u256_uart_bigint(name: &str, val: &BigInt<4>) {
    u256_uart_str(name);
    u256_uart_str("=[");
    for i in 0..4 {
        u256_uart_hex_u64(unsafe { core::ptr::read_volatile(&val.0[i]) });
        if i < 3 { u256_uart_str(","); }
    }
    u256_uart_str("]\n");
}

/// Counter for logging control
#[cfg(target_arch = "riscv64")]
static mut MONT_MUL_CALL_COUNT: u32 = 0;

pub(super) type U256 = BigInt<4>;

static mut COPY_PLACE_0: U256 = U256::zero();
static mut COPY_PLACE_1: U256 = U256::zero();
static mut COPY_PLACE_2: U256 = U256::zero();
static mut COPY_PLACE_3: U256 = U256::zero();
static mut SCRATCH: U256 = U256::zero();

static ONE: U256 = U256::one();
static ZERO: U256 = U256::zero();

pub const fn from_bytes_unchecked(bytes: &[u8; 32]) -> U256 {
    BigInt::<4>([
        u64::from_le_bytes([
            bytes[31], bytes[30], bytes[29], bytes[28], bytes[27], bytes[26], bytes[25], bytes[24],
        ]),
        u64::from_le_bytes([
            bytes[23], bytes[22], bytes[21], bytes[20], bytes[19], bytes[18], bytes[17], bytes[16],
        ]),
        u64::from_le_bytes([
            bytes[15], bytes[14], bytes[13], bytes[12], bytes[11], bytes[10], bytes[9], bytes[8],
        ]),
        u64::from_le_bytes([
            bytes[7], bytes[6], bytes[5], bytes[4], bytes[3], bytes[2], bytes[1], bytes[0],
        ]),
    ])
}

/// Runtime version of from_bytes_unchecked with volatile reads.
/// This prevents RISC-V 64-bit compiler optimization bugs that corrupt data.
#[inline(always)]
pub fn from_bytes_volatile(bytes: &[u8; 32]) -> U256 {
    // Read bytes using volatile reads and construct limbs directly
    // to prevent compiler optimization issues
    unsafe {
        let read = |i: usize| core::ptr::read_volatile(&bytes[i]);

        BigInt::<4>([
            u64::from_le_bytes([
                read(31), read(30), read(29), read(28), read(27), read(26), read(25), read(24),
            ]),
            u64::from_le_bytes([
                read(23), read(22), read(21), read(20), read(19), read(18), read(17), read(16),
            ]),
            u64::from_le_bytes([
                read(15), read(14), read(13), read(12), read(11), read(10), read(9), read(8),
            ]),
            u64::from_le_bytes([
                read(7), read(6), read(5), read(4), read(3), read(2), read(1), read(0),
            ]),
        ])
    }
}

pub fn to_be_bytes(a: U256) -> [u8; 32] {
    // Use volatile reads to prevent RISC-V 64-bit compiler optimization bugs
    let mut r = [0u8; 32];
    unsafe {
        // Read limbs using volatile reads
        let limb3 = core::ptr::read_volatile(&a.0[3]);
        let limb2 = core::ptr::read_volatile(&a.0[2]);
        let limb1 = core::ptr::read_volatile(&a.0[1]);
        let limb0 = core::ptr::read_volatile(&a.0[0]);

        // Write bytes using volatile writes
        let bytes3 = limb3.to_be_bytes();
        let bytes2 = limb2.to_be_bytes();
        let bytes1 = limb1.to_be_bytes();
        let bytes0 = limb0.to_be_bytes();

        for i in 0..8 {
            core::ptr::write_volatile(&mut r[i], bytes3[i]);
            core::ptr::write_volatile(&mut r[8 + i], bytes2[i]);
            core::ptr::write_volatile(&mut r[16 + i], bytes1[i]);
            core::ptr::write_volatile(&mut r[24 + i], bytes0[i]);
        }
    }

    r
}

#[inline(always)]
/// adds `rhs` to `self` and returns the carry
pub fn add_assign(a: &mut U256, b: &U256) -> bool {
    delegation::add(a, b) != 0
}

#[inline(always)]
/// subtracts `rhs` from `self` and returns the borrow
pub fn sub_assign(a: &mut U256, b: &U256) -> bool {
    delegation::sub(a, b) != 0
}

#[inline(always)]
/// subtracts `self` from `rhs` and reutrns the borrow
pub fn sub_and_negate_assign(a: &mut U256, b: &U256) -> bool {
    delegation::sub_and_negate(a, b) != 0
}

#[inline(always)]
/// multiplies `self` with `rhs` and storest the lowest 256 bits in self
pub fn mul_low_assign(a: &mut U256, b: &U256) {
    delegation::mul_low(a, b);
}

#[inline(always)]
/// multiplies `self` with `rhs` and storest the highest 256 bits in self
pub fn mul_high_assign(a: &mut U256, b: &U256) {
    delegation::mul_high(a, b);
}

#[inline(always)]
pub fn mul_wide(a: &U256, b: &U256) -> (U256, U256) {
    let mut low = U256::zero();
    let mut high = U256::zero();

    delegation::memcpy(&mut low, a);
    delegation::memcpy(&mut high, a);

    delegation::mul_low(&mut low, b);
    delegation::mul_high(&mut high, b);

    (low, high)
}

#[inline(always)]
/// computes `self = self - rhs - carry` and returns the borrow
pub fn sub_with_carry_bit(a: &mut U256, b: &U256, carry: bool) -> bool {
    delegation::sub_with_carry_bit(a, b, carry) != 0
}

#[inline(always)]
/// computes `self = self + rhs + carry` and returns the carry
pub fn add_with_carry_bit(a: &mut U256, b: &U256, carry: bool) -> bool {
    delegation::add_with_carry_bit(a, b, carry) != 0
}

#[inline(always)]
/// computes `self = rhs - self - carry` and returns the borrow
pub fn sub_and_negate_with_carry(a: &mut U256, b: &U256, carry: bool) -> bool {
    delegation::sub_and_negate_with_carry_bit(a, b, carry) != 0
}

#[inline(always)]
/// Tries to get `self` in the range `[0..modulus)`.
/// Note: we assume `self < 2*modulus`, otherwise the result might not be in the range
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn sub_mod_with_carry<T: DelegatedModParams<4>>(a: &mut U256, carry: bool) {
    let borrow = delegation::sub(a, T::modulus()) != 0;
    if borrow && !carry {
        delegation::add(a, T::modulus());
    }
}

#[inline(always)]
/// computes `self = self + rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn add_mod_assign<T: DelegatedModParams<4>>(a: &mut U256, b: &U256) {
    let carry = delegation::add(a, b) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// computes `self = self - rhs mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn sub_mod_assign<T: DelegatedModParams<4>>(a: &mut U256, b: &U256) {
    let borrow = delegation::sub(a, b);
    if borrow != 0 {
        delegation::add(a, T::modulus());
    }
}

#[inline(always)]
/// Computes `self = self + self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn double_mod_assign<T: DelegatedModParams<4>>(a: &mut U256) {
    delegation::memcpy(&mut COPY_PLACE_0, a);
    let carry = delegation::add(a, &COPY_PLACE_0) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// Computes `self = -self mod modulus`
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn neg_mod_assign<T: DelegatedModParams<4>>(a: &mut U256) {
    // delegation::eq returns 1 if they are equal and zero if not
    if delegation::eq(a, &ZERO) == 0 {
        delegation::sub_and_negate(a, T::modulus());
    }
}

#[inline(always)]
pub fn eq(a: &U256, b: &U256) -> bool {
    delegation::eq(a, b) != 0
}

#[inline(always)]
pub fn is_zero(a: &U256) -> bool {
    delegation::eq(a, &ZERO) != 0
}

#[inline(always)]
/// it takes `a` as mutable for the purposes of delegation calls, but doesn't mutate it
pub fn is_one(a: &U256) -> bool {
    delegation::eq(a, &ONE) != 0
}

#[inline(always)]
/// # Safety
/// `DelegationModParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn is_zero_mod<T: DelegatedModParams<4>>(a: &U256) -> bool {
    (delegation::eq(a, &ZERO) != 0) || (delegation::eq(a, T::modulus()) != 0)
}

pub fn lt(a: &U256, b: &U256) -> bool {
    let temp = unsafe { &mut COPY_PLACE_0 };
    delegation::memcpy(temp, a);

    // if we get a borrow, then self < other
    delegation::sub(temp, b) != 0
}

pub fn leq(a: &U256, b: &U256) -> bool {
    let temp = unsafe { &mut COPY_PLACE_0 };
    delegation::memcpy(temp, a);

    // if we get a borrow, then self < other
    delegation::eq(temp, b) != 0 || delegation::sub(temp, b) != 0
}

#[inline(always)]
/// modular multiplication with barret reduction
/// # Safety
/// `DelegationBarretParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn mul_assign_barret<T: DelegatedBarretParams<4>>(a: &mut U256, b: &U256) {
    let temp0 = unsafe { &mut COPY_PLACE_1 };
    let temp1 = unsafe { &mut COPY_PLACE_2 };

    // we will keep high part of product in temp0 until the very end
    delegation::memcpy(temp0, a);

    delegation::mul_low(a, b);
    delegation::mul_high(temp0, b);

    delegation::memcpy(temp1, temp0);

    // multiply copy_place0 by 2^256 - modulus
    delegation::mul_low(temp1, T::neg_modulus());
    delegation::mul_high(temp0, T::neg_modulus());

    // add and propagate the carry
    let carry = delegation::add(a, temp1) != 0;
    if carry {
        delegation::add(temp0, &ONE);
    }

    delegation::mul_low(temp0, T::neg_modulus());

    let carry = delegation::add(a, temp0) != 0;
    sub_mod_with_carry::<T>(a, carry);
}

#[inline(always)]
/// # Safety
/// `DelegationBarretParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_barret<T: DelegatedBarretParams<4>>(a: &mut U256) {
    let b = unsafe { &mut COPY_PLACE_0 };
    delegation::memcpy(b, a);

    mul_assign_barret::<T>(a, b);
}

#[inline(always)]
/// # Safety
/// `DelegationMontParams` should only provide references to mutable statics.
/// It is the responsibility of the caller to make sure that is the case
pub unsafe fn square_assign_montgomery<T: DelegatedMontParams<4>>(a: &mut U256) {
    let b = unsafe { &mut COPY_PLACE_0 };

    // COMPILER BUG WORKAROUND: Use volatile copy instead of delegation::memcpy
    // The memcpy may use optimized paths that get corrupted on riscv64
    #[cfg(target_arch = "riscv64")]
    {
        for i in 0..4 {
            let val = core::ptr::read_volatile(&a.0[i]);
            core::ptr::write_volatile(&mut b.0[i], val);
        }
    }
    #[cfg(not(target_arch = "riscv64"))]
    {
        delegation::memcpy(b, a);
    }

    mul_assign_montgomery::<T>(a, b);
}

#[inline(always)]
/// Modular multiplication with montgomery reduction.
/// It's the responsibility of the caller to make sure the parameters are in montgomery form.
/// # Safety
///
pub unsafe fn mul_assign_montgomery<T: DelegatedMontParams<4>>(a: &mut U256, b: &U256) {
    let temp0 = unsafe { &mut COPY_PLACE_1 };
    let temp1 = unsafe { &mut COPY_PLACE_2 };
    let temp2 = unsafe { &mut COPY_PLACE_3 };

    // Debug logging for specific calls
    #[cfg(target_arch = "riscv64")]
    let should_log = {
        MONT_MUL_CALL_COUNT += 1;
        // Log calls that match ecadd bigint pattern (check first limb using volatile)
        let limb0 = unsafe { core::ptr::read_volatile(&a.0[0]) };
        let is_ecadd_x_bigint = limb0 == 0xd783f2acffffff02u64;
        let is_ecadd_y_bigint = limb0 == 0x9588025e5716cab7u64;
        // Also log calls with x_coord and y_coord Montgomery form values
        let is_ecadd_x_mont = limb0 == 0xa04b42a5dec86eefu64;
        let is_ecadd_y_mont = limb0 == 0xb40b893f50aa3bc9u64;
        // Also log x^2 (for x^3 = x^2 * x calculation)
        let is_ecadd_x_sq = limb0 == 0xee5601594cddbbddu64;
        is_ecadd_x_bigint || is_ecadd_y_bigint || is_ecadd_x_mont || is_ecadd_y_mont || is_ecadd_x_sq || MONT_MUL_CALL_COUNT <= 5
    };

    #[cfg(target_arch = "riscv64")]
    if should_log {
        u256_uart_str("[mont_mul] === CALL ");
        u256_uart_hex_u64(MONT_MUL_CALL_COUNT as u64);
        u256_uart_str(" ===\n");
        u256_uart_bigint("[mont_mul] INPUT a", a);
        u256_uart_bigint("[mont_mul] INPUT b", b);
    }

    // COMPILER BUG WORKAROUND: Use volatile copy for both operands
    // See ai_plans/riscv-compiler-bugs.md
    #[cfg(target_arch = "riscv64")]
    {
        // Volatile copy a -> temp0 for mul_low
        for i in 0..4 {
            let val = core::ptr::read_volatile(&a.0[i]);
            core::ptr::write_volatile(&mut temp0.0[i], val);
        }
        // Volatile copy b -> SCRATCH
        for i in 0..4 {
            let val = core::ptr::read_volatile(&b.0[i]);
            core::ptr::write_volatile(&mut SCRATCH.0[i], val);
        }
        delegation::mul_low(temp0, &SCRATCH);

        // Also volatile copy a for mul_high (don't reuse temp0, it's been modified)
        // Copy a -> a itself via volatile to force compiler to not optimize reads
        for i in 0..4 {
            let val = core::ptr::read_volatile(&a.0[i]);
            core::ptr::write_volatile(&mut a.0[i], val);
        }
        delegation::mul_high(a, &SCRATCH);
    }
    #[cfg(not(target_arch = "riscv64"))]
    {
        delegation::memcpy(temp0, a);
        delegation::mul_low(temp0, b);
        delegation::mul_high(a, b);
    }

    #[cfg(target_arch = "riscv64")]
    if should_log {
        u256_uart_bigint("[mont_mul] after mul_low temp0", temp0);
        u256_uart_bigint("[mont_mul] after mul_high a", a);
    }

    // COMPILER BUG WORKAROUND: Use volatile copies for all memcpy operations
    #[cfg(target_arch = "riscv64")]
    {
        // Volatile copy temp0 -> temp1
        for i in 0..4 {
            let val = core::ptr::read_volatile(&temp0.0[i]);
            core::ptr::write_volatile(&mut temp1.0[i], val);
        }
    }
    #[cfg(not(target_arch = "riscv64"))]
    delegation::memcpy(temp1, temp0);

    delegation::mul_low(temp1, T::reduction_const());

    // COMPILER BUG WORKAROUND: Use volatile copies for all memcpy operations
    #[cfg(target_arch = "riscv64")]
    {
        // Volatile copy temp1 -> temp2
        for i in 0..4 {
            let val = core::ptr::read_volatile(&temp1.0[i]);
            core::ptr::write_volatile(&mut temp2.0[i], val);
        }
    }
    #[cfg(not(target_arch = "riscv64"))]
    delegation::memcpy(temp2, temp1);

    delegation::mul_low(temp2, T::modulus());
    delegation::mul_high(temp1, T::modulus());

    let carry = delegation::add(temp2, temp0) != 0;

    debug_assert!(temp2.is_zero());

    if carry {
        delegation::add(temp1, &ONE);
    }

    let carry = delegation::add(a, temp1) != 0;
    sub_mod_with_carry::<T>(a, carry);

    #[cfg(target_arch = "riscv64")]
    if should_log {
        u256_uart_bigint("[mont_mul] RESULT a", a);
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct U256Wrapper<T: DelegatedModParams<4>>(pub U256, PhantomData<T>);

#[cfg(test)]
impl<T: DelegatedModParams<4> + Debug> proptest::arbitrary::Arbitrary for U256Wrapper<T> {
    type Parameters = T;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Just, Strategy};

        any::<[u64; 4]>().prop_map(|words| {
            let mut res = BigInt::<4>(words);
            unsafe {
                sub_mod_with_carry::<Self::Parameters>(&mut res, false);
                sub_mod_with_carry::<Self::Parameters>(&mut res, false);
            }
            Self(res, PhantomData::default())
        })
    }

    type Strategy = proptest::arbitrary::Mapped<[u64; 4], Self>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ark_ff_delegation::BigInt;

    use ark_ff::{BigInt as BigIntRef, BigInteger};
    use proptest::{prop_assert_eq, proptest};

    #[derive(Default, Debug)]
    struct ZeroMod;

    impl DelegatedModParams<4> for ZeroMod {
        unsafe fn modulus() -> &'static BigInt<4> {
            &ZERO
        }
    }

    #[ignore = "requires a single threaded runner"]
    #[test]
    fn test_mul_wide() {
        proptest!(|(x: U256Wrapper<ZeroMod>, y: U256Wrapper<ZeroMod>)| {
            let (x, y) = (x.0, y.0);
            let x_ref = BigIntRef::new(x.0);
            let y_ref = BigIntRef::new(y.0);

            let (ref_low, ref_high) = x_ref.mul(&y_ref);
            let (low, high) = mul_wide(&x, &y);

            prop_assert_eq!(low.0, ref_low.0);
            prop_assert_eq!(high.0, ref_high.0);

        })
    }
}
