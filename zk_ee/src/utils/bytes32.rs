use crate::oracle::usize_serialization::UsizeDeserializable;
use crate::system::errors::internal::InternalError;
use crate::{internal_error, oracle::usize_serialization::UsizeSerializable};
use core::mem::MaybeUninit;
use ruint::aliases::{B160, U256};

#[cfg(target_pointer_width = "32")]
pub const BYTES32_USIZE_SIZE: usize = 8;

#[cfg(target_pointer_width = "64")]
pub const BYTES32_USIZE_SIZE: usize = 4;

#[repr(align(8))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bytes32 {
    inner: [usize; BYTES32_USIZE_SIZE],
}

const _: () = const {
    assert!(core::mem::size_of::<Bytes32>() == 32);
    assert!(core::mem::align_of::<Bytes32>() >= core::mem::align_of::<usize>());
};

// we compare as integers to avoid any potential ambiguity

impl Ord for Bytes32 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_u8_array_ref().cmp(other.as_u8_array_ref())
    }
}

impl PartialOrd for Bytes32 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::fmt::Debug for Bytes32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x")?;
        for word in self.inner.iter() {
            #[cfg(target_pointer_width = "32")]
            write!(f, "{:08x}", word.to_be())?;

            #[cfg(target_pointer_width = "64")]
            write!(f, "{:016x}", word.to_be())?;
        }

        Ok(())
    }
}

impl Bytes32 {
    pub const ZERO: Self = Self {
        inner: [0usize; BYTES32_USIZE_SIZE],
    };

    pub const MAX: Self = Self {
        inner: [usize::MAX; BYTES32_USIZE_SIZE],
    };

    #[inline(always)]
    pub fn uninit() -> MaybeUninit<Self> {
        MaybeUninit::uninit()
    }

    pub fn from_byte_fill(byte: u8) -> Self {
        let mut buffer = 0usize.to_ne_bytes();
        buffer.fill(byte);
        let init_value = usize::from_ne_bytes(buffer);
        Self {
            inner: [init_value; BYTES32_USIZE_SIZE],
        }
    }

    #[inline(always)]
    pub const fn zero() -> Self {
        Self {
            inner: [0usize; BYTES32_USIZE_SIZE],
        }
    }

    #[inline(always)]
    pub fn from_array(array: [u8; 32]) -> Self {
        unsafe { core::mem::transmute_copy(&array) }
    }

    #[inline]
    pub const fn num_trailing_nonzero_bytes(&self) -> usize {
        #[cfg(target_endian = "big")]
        compile_error!("unsupported architecture: big endian arch is not supported");

        let mut result = 32;
        let mut i = 0;
        while i < BYTES32_USIZE_SIZE {
            let word = self.inner[i];
            if word == 0 {
                result -= core::mem::size_of::<usize>() as u32;
            } else {
                // NOTE - we should BE it, so it's TRAILING
                result -= word.trailing_zeros() / 8;
                break;
            }

            i += 1;
        }

        result as usize
    }

    #[allow(clippy::needless_as_bytes)]
    pub const fn from_hex(input: &str) -> Self {
        const fn hex_to_digit(c: u8) -> u8 {
            match c {
                b'A'..=b'F' => c - b'A' + 10,
                b'a'..=b'f' => c - b'a' + 10,
                b'0'..=b'9' => c - b'0',
                _ => {
                    unreachable!()
                }
            }
        }

        assert!(input.len() == 64);
        assert!(input.as_bytes().len() == 64); // ASCII check in essence
        let mut result = Self::ZERO;
        let mut idx = 0;
        let dst = result.as_u8_array_mut();
        let src = input.as_bytes().as_chunks::<2>().0;
        while idx < 32 {
            let dst = &mut dst[idx];
            let [high, low] = src[idx];
            let high = hex_to_digit(high);
            let low = hex_to_digit(low);
            *dst = (high << 4) | low;

            idx += 1;
        }

        result
    }

    pub fn is_zero(&self) -> bool {
        self.inner.iter().all(|el| *el == 0)
    }

    fn as_usize_array_mut(&mut self) -> &mut [usize; BYTES32_USIZE_SIZE] {
        &mut self.inner
    }

    #[cfg(target_pointer_width = "32")]
    fn as_u32_array_ref(&self) -> &[u32; 8] {
        unsafe { &*(&self.inner as *const usize).cast::<[u32; 8]>() }
    }

    #[cfg(target_pointer_width = "64")]
    pub fn as_u64_array_ref(&self) -> &[u64; 4] {
        unsafe { &*(&self.inner as *const usize).cast::<[u64; 4]>() }
    }

    pub fn as_u8_ref(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts((&self.inner as *const usize).cast::<u8>(), 32) }
    }

    pub fn as_u8_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut((&mut self.inner as *mut usize).cast::<u8>(), 32) }
    }

    pub const fn as_u8_array(self) -> [u8; 32] {
        unsafe { core::mem::transmute(self) }
    }

    pub const fn as_u8_array_ref(&self) -> &[u8; 32] {
        unsafe { &*(&self.inner as *const usize).cast::<[u8; 32]>() }
    }

    pub const fn as_u8_array_mut(&mut self) -> &mut [u8; 32] {
        unsafe { &mut *(&mut self.inner as *mut usize).cast::<[u8; 32]>() }
    }

    pub fn bytereverse(&mut self) {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                self.inner.swap(0, 7);
                self.inner.swap(1, 6);
                self.inner.swap(2, 5);
                self.inner.swap(3, 4);
                for el in self.inner.iter_mut() {
                    *el = el.to_be();
                }
                return;
            } else if #[cfg(target_pointer_width = "64")] {
                self.inner.swap(0, 3);
                self.inner.swap(1, 2);
                for el in self.inner.iter_mut() {
                    // NOTE: we are on LE
                    *el = el.swap_bytes();
                }
                return;
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }

    pub fn into_u256_le(self) -> U256 {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else {
                unsafe {
                    #[allow(clippy::missing_transmute_annotations)]
                    return core::mem::transmute(self);
                }
            }
        );
    }

    pub fn into_u256_be(self) -> U256 {
        U256::from_be_bytes(self.as_u8_array())
    }

    pub fn from_u256_le(value: &U256) -> Self {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else {
                unsafe {
                    #[allow(clippy::missing_transmute_annotations)]
                    return core::mem::transmute_copy(value);
                }
            }
        );
    }

    pub fn from_u256_be(value: &U256) -> Self {
        Self::from_array(value.to_be_bytes())
    }
}

// here we assume left-padding of zeroes for future
#[allow(clippy::from_over_into)]
impl Into<B160> for Bytes32 {
    fn into(self) -> B160 {
        // let's hope compiler optimizes it out
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&self.as_u8_array_ref()[12..]);
        B160::from_be_bytes(bytes)
    }
}

impl From<B160> for Bytes32 {
    fn from(value: B160) -> Self {
        let mut new = Bytes32::zero();
        new.as_u8_array_mut()[12..].copy_from_slice(&value.to_be_bytes::<{ B160::BYTES }>()[..]);

        new
    }
}

impl From<[u8; 32]> for Bytes32 {
    fn from(value: [u8; 32]) -> Self {
        Self::from_array(value)
    }
}

impl UsizeSerializable for Bytes32 {
    const USIZE_LEN: usize = const {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let size = 8;
            } else if #[cfg(target_pointer_width = "64")] {
                let size = 4;
            } else {
                compile_error!("unsupported architecture")
            }
        );
        #[allow(clippy::let_and_return)]
        size
    };

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                return self.as_u32_array_ref().into_iter().map(|el| *el as usize);
            } else if #[cfg(target_pointer_width = "64")] {
                return self.as_u64_array_ref().map(|el| el as usize).into_iter();
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for Bytes32 {
    const USIZE_LEN: usize = <Bytes32 as UsizeSerializable>::USIZE_LEN;

    #[inline(never)]
    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        if src.len() < <Self as UsizeDeserializable>::USIZE_LEN {
            return Err(internal_error!("Bytes32 deserialization failed: too short"));
        }
        let mut new = Bytes32::ZERO;

        // RV64 diagnostic: track raw iterator values to find corruption source
        // Only enable on actual riscv64 target, not x86_64
        #[cfg(target_arch = "riscv64")]
        {
            const UART_ADDR: u64 = 0xa000_0200;

            #[inline(never)]
            fn uart_byte(b: u8) {
                unsafe { core::ptr::write_volatile(UART_ADDR as *mut u8, b); }
            }

            #[inline(never)]
            fn uart_hex(val: u64) {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for i in (0..16).rev() {
                    uart_byte(HEX[((val >> (i * 4)) & 0xf) as usize]);
                }
            }

            // Read all 4 usizes and store raw values
            let v0 = unsafe { src.next().unwrap_unchecked() };
            let v1 = unsafe { src.next().unwrap_unchecked() };
            let v2 = unsafe { src.next().unwrap_unchecked() };
            let v3 = unsafe { src.next().unwrap_unchecked() };

            // Check for "corrupted zero" pattern: v0 == 0x0100 (byte1=1, rest=0) and v1,v2,v3 all zero
            // This catches storage slots that SHOULD be zero but have byte 1 corrupted to 0x01
            if v0 == 0x0000_0000_0000_0100 && v1 == 0 && v2 == 0 && v3 == 0 {
                // Exact corrupted zero pattern!
                uart_byte(b'[');
                uart_byte(b'C');
                uart_byte(b'O');
                uart_byte(b'R');
                uart_byte(b'R');
                uart_byte(b'-');
                uart_byte(b'Z');
                uart_byte(b'E');
                uart_byte(b'R');
                uart_byte(b'O');
                uart_byte(b']');
                uart_byte(b'\n');
            }

            // Also print any value where byte1 is suspiciously 0x01 with byte0=0
            // This helps identify other potential corruption patterns
            let byte0 = (v0 & 0xff) as u8;
            let byte1 = ((v0 >> 8) & 0xff) as u8;
            if byte1 == 0x01 && byte0 == 0x00 {
                uart_byte(b'[');
                uart_byte(b'B');
                uart_byte(b'1');
                uart_byte(b']');
                uart_byte(b' ');
                uart_hex(v0 as u64);
                uart_byte(b' ');
                uart_hex(v1 as u64);
                uart_byte(b' ');
                uart_hex(v2 as u64);
                uart_byte(b' ');
                uart_hex(v3 as u64);
                uart_byte(b'\n');
            }

            let arr = new.as_usize_array_mut();
            arr[0] = v0;
            arr[1] = v1;
            arr[2] = v2;
            arr[3] = v3;

            // Check AFTER storing: did the store corrupt the value?
            let stored0 = unsafe { core::ptr::read_volatile(&arr[0]) };
            let stored1 = unsafe { core::ptr::read_volatile(&arr[1]) };
            let stored2 = unsafe { core::ptr::read_volatile(&arr[2]) };
            let stored3 = unsafe { core::ptr::read_volatile(&arr[3]) };

            // Check for corruption: stored value differs from what we wrote
            if stored0 != v0 || stored1 != v1 || stored2 != v2 || stored3 != v3 {
                uart_byte(b'[');
                uart_byte(b'S');
                uart_byte(b'T');
                uart_byte(b'O');
                uart_byte(b'R');
                uart_byte(b'E');
                uart_byte(b']');
                uart_byte(b' ');
                uart_hex(v0 as u64);
                uart_byte(b'-');
                uart_byte(b'>');
                uart_hex(stored0 as u64);
                uart_byte(b'\n');
            }

            // Check for corrupted zero pattern in stored value
            if stored0 == 0x0100 && stored1 == 0 && stored2 == 0 && stored3 == 0 {
                uart_byte(b'[');
                uart_byte(b'Z');
                uart_byte(b'C');
                uart_byte(b'O');
                uart_byte(b'R');
                uart_byte(b'R');
                uart_byte(b']');
                uart_byte(b'\n');
            }
        }

        #[cfg(not(target_arch = "riscv64"))]
        for dst in new.as_usize_array_mut().iter_mut() {
            *dst = unsafe { src.next().unwrap_unchecked() };
        }

        // Final check before return - read the value back and check for corruption
        #[cfg(target_arch = "riscv64")]
        {
            const UART_ADDR: u64 = 0xa000_0200;
            fn uart_byte(b: u8) {
                unsafe { core::ptr::write_volatile(UART_ADDR as *mut u8, b); }
            }
            fn uart_hex(val: u64) {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for i in (0..16).rev() {
                    uart_byte(HEX[((val >> (i * 4)) & 0xf) as usize]);
                }
            }

            let final0 = unsafe { core::ptr::read_volatile(&new.inner[0]) };
            if final0 == 0x0100 {
                let final1 = unsafe { core::ptr::read_volatile(&new.inner[1]) };
                let final2 = unsafe { core::ptr::read_volatile(&new.inner[2]) };
                let final3 = unsafe { core::ptr::read_volatile(&new.inner[3]) };
                if final1 == 0 && final2 == 0 && final3 == 0 {
                    uart_byte(b'[');
                    uart_byte(b'F');
                    uart_byte(b'I');
                    uart_byte(b'N');
                    uart_byte(b'A');
                    uart_byte(b'L');
                    uart_byte(b']');
                    uart_byte(b'\n');
                }
            }
        }

        Ok(new)
    }

    unsafe fn init_from_iter(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        if src.len() < <Self as UsizeDeserializable>::USIZE_LEN {
            return Err(internal_error!("b160 deserialization failed: too short"));
        }
        for dst in this.assume_init_mut().as_usize_array_mut().iter_mut() {
            *dst = src.next().unwrap_unchecked()
        }

        Ok(())
    }
}
