use super::location::ErrorLocation;

#[cfg_attr(any(target_arch = "riscv32", target_arch = "riscv64"), derive(Copy))]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Metadata {
    pub location: ErrorLocation,
}

impl Metadata {
    pub fn new(location: ErrorLocation) -> Self {
        Self { location }
    }
}

impl From<ErrorLocation> for Metadata {
    fn from(location: ErrorLocation) -> Self {
        Self::new(location)
    }
}

impl core::fmt::Display for Metadata {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self { location } = self;
        writeln!(f, "-- at {location}")?;
        Ok(())
    }
}
