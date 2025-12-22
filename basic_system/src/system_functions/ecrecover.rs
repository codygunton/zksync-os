use super::*;
use crate::cost_constants::{ECRECOVER_COST_ERGS, ECRECOVER_NATIVE_COST};
use zk_ee::common_traits::TryExtend;
use zk_ee::out_of_return_memory;
use zk_ee::system::base_system_functions::{Secp256k1ECRecoverErrors, SystemFunction};
use zk_ee::system::errors::{subsystem::SubsystemError, system::SystemError};
use zk_ee::system::Computational;

///
/// ecrecover system function implementation.
///
pub struct EcRecoverImpl;

impl<R: Resources> SystemFunction<R, Secp256k1ECRecoverErrors> for EcRecoverImpl {
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    /// If the input is invalid(v != 27|28 or failed to recover signer) returns `Ok(0)`.
    ///
    /// Returns `OutOfGas` if not enough resources provided.
    fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1ECRecoverErrors>> {
        Ok(cycle_marker::wrap_with_resources!(
            "ecrecover",
            resources,
            { ecrecover_as_system_function_inner(input, output, resources) }
        )?)
    }
}

fn ecrecover_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SystemError> {
    resources.charge(&R::from_ergs_and_native(
        ECRECOVER_COST_ERGS,
        R::Native::from_computational(ECRECOVER_NATIVE_COST),
    ))?;
    // digest, v, r, s in ABI
    let mut buffer = [0u8; 128];
    for (dst, src) in buffer.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    // follow https://github.com/ethereum/go-ethereum/blob/aadcb886753079d419f966a3bc990f708f8d1c3b/core/vm/contracts.go#L188

    let mut it = buffer.array_chunks::<32>();
    let recovered_pubkey_bytes = unsafe {
        let digest = it.next().unwrap_unchecked();
        let v = it.next().unwrap_unchecked();
        let r = it.next().unwrap_unchecked();
        let s = it.next().unwrap_unchecked();

        if v[..31].iter().all(|el| *el == 0) == false {
            return Ok(());
        }

        let rec_id = v[31].wrapping_sub(27);
        if (rec_id == 0 || rec_id == 1) == false {
            return Ok(());
        }

        let Ok(pk_bytes) = ecrecover_inner(digest, r, s, rec_id) else {
            return Ok(());
        };

        pk_bytes
    };
    let bytes_ref = recovered_pubkey_bytes.as_ref();

    use crypto::sha3::{Digest, Keccak256};
    let address_hash = Keccak256::digest(&bytes_ref[1..]);

    dst.try_extend(core::iter::repeat_n(0, 12).chain(address_hash.into_iter().skip(12)))
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

/// CSR-based QuasiUART for RISC-V guests (both Airbender and Zisk)
/// Uses CSR 0x7c0 with the QuasiUART protocol for unified logging.
/// Buffers entire lines to produce clean output with single [GUEST] prefix.
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
mod uart_log {
    use arrayvec::ArrayString;

    const HELLO_MARKER: u32 = u32::MAX; // 0xffffffff = UART_QUERY_ID
    // Buffer size: enough for longest log line
    // "[ecrecover_inner] INPUT: digest=0x" + 64 + ", r=0x" + 64 + ", s=0x" + 64 + ", rec_id=" + 2 + newline
    const BUF_SIZE: usize = 280;

    // Single-threaded guest buffer
    static mut LINE_BUF: ArrayString<BUF_SIZE> = ArrayString::new_const();

    #[inline(always)]
    fn csr_write_word(word: usize) {
        unsafe {
            core::arch::asm!(
                "csrrw x0, 0x7c0, {rd}",
                rd = in(reg) word,
                options(nomem, nostack, preserves_flags)
            )
        }
    }

    fn flush_buffer(s: &str) {
        let len = s.len();
        if len == 0 {
            return;
        }
        // QuasiUART protocol:
        // 1. Write HELLO_MARKER (0xffffffff)
        // 2. Write word count (ceil(len/4) + 1 for length word)
        // 3. Write message length in bytes
        // 4. Write message data as 4-byte LE words
        csr_write_word(HELLO_MARKER as usize);
        csr_write_word(len.next_multiple_of(4) / 4 + 1);
        csr_write_word(len);

        // Write bytes in 4-byte words
        let bytes = s.as_bytes();
        let mut i = 0;
        while i + 4 <= len {
            let word = u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
            csr_write_word(word as usize);
            i += 4;
        }

        // Flush remaining bytes (padded with zeros)
        if i < len {
            let mut buf = [0u8; 4];
            for j in 0..(len - i) {
                buf[j] = bytes[i + j];
            }
            csr_write_word(u32::from_le_bytes(buf) as usize);
        }
    }

    #[inline(never)]
    pub fn write_str(s: &str) {
        unsafe {
            let _ = LINE_BUF.try_push_str(s);
        }
    }

    #[inline(never)]
    pub fn write_hex_byte(byte: u8) {
        const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
        unsafe {
            let _ = LINE_BUF.try_push(HEX_CHARS[(byte >> 4) as usize] as char);
            let _ = LINE_BUF.try_push(HEX_CHARS[(byte & 0xf) as usize] as char);
        }
    }

    #[inline(never)]
    pub fn write_hex_32(bytes: &[u8; 32]) {
        write_str("0x");
        for b in bytes {
            write_hex_byte(*b);
        }
    }

    #[inline(never)]
    pub fn write_hex_slice(bytes: &[u8]) {
        write_str("0x");
        for b in bytes {
            write_hex_byte(*b);
        }
    }

    #[inline(never)]
    pub fn newline() {
        unsafe {
            let _ = LINE_BUF.try_push('\n');
            flush_buffer(&LINE_BUF);
            LINE_BUF.clear();
        }
    }
}

/// No-op stub for non-RISC-V targets (host builds, benchmarks, etc.)
#[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
mod uart_log {
    #[inline(always)]
    pub fn write_str(_s: &str) {}
    #[inline(always)]
    pub fn write_hex_byte(_byte: u8) {}
    #[inline(always)]
    pub fn write_hex_32(_bytes: &[u8; 32]) {}
    #[inline(always)]
    pub fn write_hex_slice(_bytes: &[u8]) {}
    #[inline(always)]
    pub fn newline() {}
}

pub fn ecrecover_inner(
    digest: &[u8; 32],
    r: &[u8; 32],
    s: &[u8; 32],
    rec_id: u8,
) -> Result<crypto::k256::EncodedPoint, ()> {
    use crypto::k256::{
        ecdsa::{hazmat::bits2field, RecoveryId, Signature},
        elliptic_curve::ops::Reduce,
        Scalar,
    };

    // Log inputs
    uart_log::write_str("[ecrecover_inner] INPUT: digest=");
    uart_log::write_hex_32(digest);
    uart_log::write_str(", r=");
    uart_log::write_hex_32(r);
    uart_log::write_str(", s=");
    uart_log::write_hex_32(s);
    uart_log::write_str(", rec_id=");
    uart_log::write_hex_byte(rec_id);
    uart_log::newline();

    let signature = Signature::from_scalars(*r, *s).map_err(|_| {
        uart_log::write_str("[ecrecover_inner] OUTPUT: Error - failed to create signature from scalars\n");
    })?;
    let recovery_id = RecoveryId::try_from(rec_id).map_err(|_| {
        uart_log::write_str("[ecrecover_inner] OUTPUT: Error - invalid recovery id\n");
    })?;

    let message = <Scalar as Reduce<crypto::k256::U256>>::reduce_bytes(
        &bits2field::<crypto::k256::Secp256k1>(digest).map_err(|_| {
            uart_log::write_str("[ecrecover_inner] OUTPUT: Error - bits2field failed\n");
        })?,
    );

    // Log encoded message scalar
    uart_log::write_str("[ecrecover_inner] Encoded message scalar: ");
    uart_log::write_hex_slice(message.to_bytes().as_slice());
    uart_log::newline();

    let Ok(pk) = crypto::secp256k1::recover(&message, &signature, &recovery_id) else {
        uart_log::write_str("[ecrecover_inner] OUTPUT: Error - recovery failed\n");
        return Err(());
    };

    // represent as bytes, and we do not need compression
    let encoded = pk.to_encoded_point(false);

    // Log output
    uart_log::write_str("[ecrecover_inner] OUTPUT: Success - encoded_point=");
    uart_log::write_hex_slice(encoded.as_bytes());
    uart_log::newline();

    Ok(encoded)
}

#[cfg(test)]
mod test {
    use super::*;
    use hex;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::Resource;

    #[test]
    fn test_geth_ecrecover() {
        let input: [u8; 128] =
            hex::decode("38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02")
                .expect("should decode hex")
                .try_into()
                .unwrap();

        let expected_pubkey: [u8; 32] =
            hex::decode("000000000000000000000000ceaccac640adf55b2028469bd36ba501f28b699d")
                .expect("should decode pubkey")
                .try_into()
                .unwrap();

        let mut pubkey = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        ecrecover_as_system_function_inner(input.as_slice(), &mut pubkey, &mut resources)
            .expect("ecrecover");
        assert_eq!(pubkey.len(), 32, "Size should be 32");
        assert_eq!(
            pubkey, expected_pubkey,
            "pubkey should be equal to reference"
        )
    }

    #[test]
    fn test_empty_input() {
        let input = [0u8; 128];
        let mut pubkey = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        ecrecover_as_system_function_inner(input.as_slice(), &mut pubkey, &mut resources)
            .expect("ecrecover");
        assert_eq!(pubkey.len(), 0, "Size should be 0");
    }

    #[test]
    fn test_point_of_infinity_in_result() {
        let input: [u8; 128] =
            hex::decode("6b8d2c81b11b2d699528dde488dbdf2f94293d0d33c32e347f255fa4a6c1f0a9000000000000000000000000000000000000000000000000000000000000001b79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f817986b8d2c81b11b2d699528dde488dbdf2f94293d0d33c32e347f255fa4a6c1f0a9")
                .expect("should decode hex")
                .try_into()
                .unwrap();

        let mut pubkey = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        ecrecover_as_system_function_inner(input.as_slice(), &mut pubkey, &mut resources)
            .expect("ecrecover");
        assert_eq!(pubkey.len(), 0, "Size should be 0 in case of error");
    }

    #[test]
    fn test_affine_point_decompression_regression() {
        let input: [u8; 128] =
            hex::decode("00c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000000000000000000000000000001c00b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75f00b940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549")
                .expect("should decode hex")
                .try_into()
                .unwrap();

        let mut pubkey = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        ecrecover_as_system_function_inner(input.as_slice(), &mut pubkey, &mut resources)
            .expect("ecrecover");
        assert_eq!(pubkey.len(), 0, "Size should be 0 in case of error");
    }

    #[test]
    fn test_regressions() {
        let input: [u8; 128] = [
            34, 189, 7, 49, 212, 191, 250, 136, 64, 38, 37, 181, 186, 57, 224, 78, 233, 173, 214,
            83, 76, 49, 218, 108, 17, 157, 130, 90, 57, 130, 43, 41, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 102, 102, 116, 99,
            212, 10, 196, 65, 102, 33, 136, 237, 62, 102, 50, 156, 33, 172, 161, 101, 19, 51, 146,
            204, 26, 20, 184, 68, 133, 96, 10, 135, 80, 135, 255, 193, 105, 5, 204, 108, 234, 239,
            23, 70, 48, 206, 157, 208, 196, 11, 63, 78, 148, 255, 0, 238, 54, 88, 166, 166, 127,
            236, 38, 19,
        ];
        let mut pubkey = vec![];
        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        let expected_pubkey: [u8; 32] = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99, 249, 114, 95, 16, 115, 88, 201, 17, 91, 201,
            216, 108, 114, 221, 88, 35, 233, 177, 230,
        ];

        ecrecover_as_system_function_inner(input.as_slice(), &mut pubkey, &mut resources)
            .expect("ecrecover");
        assert_eq!(pubkey.len(), 32, "Size should be 32");
        assert_eq!(
            pubkey, expected_pubkey,
            "pubkey should be equal to reference"
        )
    }
}
