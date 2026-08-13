use crate::shared::{init, EntropySource, Error};

/// OS entropy source, backed by `getrandom`/`std`.
pub struct OsEntropy;

impl EntropySource for OsEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), Error> {
        getrandom::fill(dest).map_err(|e| Error::from(e))
    }
}

/// Seed the process-wide DRBG from the OS entropy source.
pub fn init_default() -> Result<(), Error> {
    init(&OsEntropy)
}

/// Seed the global DRBG from the OS source if it hasn't been seeded yet.
///
/// Called automatically by [`crate::fill`] on first use so hosted binaries
/// don't have to seed explicitly.
#[doc(hidden)]
pub fn try_auto_seed() -> Result<(), Error> {
    init(&OsEntropy)
}
