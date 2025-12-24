/// Copy bytes from src to dst, zero-padding dst if src is shorter.
/// Uses volatile reads/writes to prevent compiler optimization issues on RV64 (ZisK).
/// See ai_plans/riscv-compiler-bugs.md for details.
pub fn copy_and_zeropad_nonoverlapping(src: &[u8], dst: &mut [u8]) {
    let to_copy = core::cmp::min(src.len(), dst.len());
    // WORKAROUND: Use volatile reads and writes to prevent data corruption on ZisK.
    // Normal copy_from_slice produces corrupted data when src is from EVM heap/calldata.
    for i in 0..to_copy {
        unsafe {
            let byte = core::ptr::read_volatile(&src[i]);
            core::ptr::write_volatile(&mut dst[i], byte);
        }
    }
    // Zero-fill remaining bytes
    for i in to_copy..dst.len() {
        unsafe {
            core::ptr::write_volatile(&mut dst[i], 0);
        }
    }
}
