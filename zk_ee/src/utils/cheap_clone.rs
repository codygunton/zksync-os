/// Using this trait allows using Copy instead of Clone on RISCV target. It is
/// useful for types like errors which contain less fields when compiled for
/// RISCV
pub trait CheapCloneRiscV {
    /// Clone for standard target, Copy if on RISC-V.
    /// Used for performance on small types
    fn clone_or_copy(&self) -> Self;
}

#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
impl<T> CheapCloneRiscV for T
where
    T: Copy,
{
    #[inline]
    fn clone_or_copy(&self) -> Self {
        *self
    }
}

#[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
impl<T> CheapCloneRiscV for T
where
    T: Clone,
{
    #[inline]
    fn clone_or_copy(&self) -> Self {
        self.clone()
    }
}
