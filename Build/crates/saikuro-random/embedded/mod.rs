use crate::shared::{init, EntropySource, Error};

/// Seed the process-wide DRBG from an application-provided [`EntropySource`].
///
/// Call this once at startup after constructing the MCU's entropy source, e.g.
/// a hardware RNG peripheral. There is no default source on `embedded`.
pub fn init_from(source: &impl EntropySource) -> Result<(), Error> {
    init(source)
}

/// The `embedded` engine has no default entropy source, so auto-seed is a
/// no-op that reports the DRBG as unseeded until the application calls
/// [`init_from`].
#[doc(hidden)]
pub fn try_auto_seed() -> Result<(), Error> {
    Err(Error::DrbgNotSeeded)
}
