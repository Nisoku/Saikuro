#[cfg(feature = "no_std")]
use crate::shared::{init, EntropySource};
#[cfg(feature = "no_std")]
use saikuro_event::SaikuroError;

/// WASI entropy source, backed by `getrandom`'s built-in backend.
#[cfg(feature = "no_std")]
pub struct WasiEntropy;

#[cfg(feature = "no_std")]
impl EntropySource for WasiEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), SaikuroError> {
        getrandom::fill(dest).map_err(|e| SaikuroError::from(e))
    }
}

/// Seed the process-wide DRBG from the WASI entropy source.
#[cfg(feature = "no_std")]
pub fn init_default() -> Result<(), SaikuroError> {
    init(&WasiEntropy)
}

/// Seed the global DRBG from the WASI source if it hasn't been seeded yet.
///
/// Called automatically by [`crate::fill`] on first use.
#[cfg(feature = "no_std")]
#[doc(hidden)]
pub fn try_auto_seed() -> Result<(), SaikuroError> {
    init(&WasiEntropy)
}
