use crate::shared::{init, EntropySource};
use saikuro_event::SaikuroError;

/// OS entropy source, backed by `getrandom`/`std`.
pub struct OsEntropy;

impl EntropySource for OsEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), SaikuroError> {
        getrandom::fill(dest).map_err(SaikuroError::from)
    }
}

/// Seed the process-wide DRBG from the OS entropy source.
pub fn init_default() -> Result<(), SaikuroError> {
    init(&OsEntropy)
}

/// Seed the global DRBG from the OS source if it hasn't been seeded yet.
///
/// Called automatically by [`crate::fill`] on first use so hosted binaries
/// don't have to seed explicitly.
#[doc(hidden)]
pub fn try_auto_seed() -> Result<(), SaikuroError> {
    if crate::shared::is_seeded() {
        return Ok(());
    }
    init(&OsEntropy)
}
