pub use sha3::*;

impl crate::MiniDigest for Keccak256 {
    type HashOutput = [u8; 32];

    #[inline(always)]
    fn new() -> Self {
        <Keccak256 as Digest>::new()
    }

    #[inline(always)]
    fn digest(input: impl AsRef<[u8]>) -> Self::HashOutput {
        let mut hasher = <Keccak256 as Digest>::new();
        // WORKAROUND: Use volatile reads and byte-by-byte updates to prevent
        // compiler optimization issues on RV64 (ZisK). Passing slices directly
        // to the hasher can produce incorrect hashes when the slice comes from
        // certain memory regions (e.g., stack-allocated buffers that were written
        // via volatile writes). See ai_plans/riscv-compiler-bugs.md for details.
        let slice = input.as_ref();
        for i in 0..slice.len() {
            let byte = unsafe { core::ptr::read_volatile(&slice[i]) };
            <Keccak256 as Digest>::update(&mut hasher, &[byte]);
        }
        let digest = <Keccak256 as Digest>::finalize(hasher);
        let mut result = [0u8; 32];
        #[allow(deprecated)]
        result.copy_from_slice(digest.as_slice());
        result
    }

    #[inline(always)]
    fn update(&mut self, input: impl AsRef<[u8]>) {
        // WORKAROUND: Use volatile reads and byte-by-byte updates.
        // See digest() comment for details.
        let slice = input.as_ref();
        for i in 0..slice.len() {
            let byte = unsafe { core::ptr::read_volatile(&slice[i]) };
            <Keccak256 as Digest>::update(self, &[byte]);
        }
    }

    #[inline(always)]
    fn finalize(self) -> Self::HashOutput {
        <Keccak256 as Digest>::finalize(self).into()
    }

    #[inline(always)]
    fn finalize_reset(&mut self) -> Self::HashOutput {
        <Keccak256 as Digest>::finalize_reset(self).into()
    }
}
