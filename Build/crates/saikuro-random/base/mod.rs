#[cfg(feature = "no_std")]
use crate::shared::{init, EntropySource, Error};

/// WASI entropy source, backed by `getrandom`'s built-in backend.
#[cfg(feature = "no_std")]
pub struct WasiEntropy;

#[cfg(feature = "no_std")]
impl EntropySource for WasiEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), Error> {
        getrandom::fill(dest).map_err(|e| Error::from(e))
    }
}

/// Seed the process-wide DRBG from the WASI entropy source.
#[cfg(feature = "no_std")]
pub fn init_default() -> Result<(), Error> {
    init(&WasiEntropy)
}

/// Seed the global DRBG from the WASI source if it hasn't been seeded yet.
///
/// Called automatically by [`crate::fill`] on first use.
#[cfg(feature = "no_std")]
#[doc(hidden)]
pub fn try_auto_seed() -> Result<(), Error> {
    init(&WasiEntropy)
}
