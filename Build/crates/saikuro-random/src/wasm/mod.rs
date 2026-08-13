use crate::shared::{init, EntropySource, Error};

/// Browser entropy source, backed by `getrandom`/`wasm_js`.
pub struct JsEntropy;

impl EntropySource for JsEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), Error> {
        getrandom::fill(dest).map_err(|e| Error::from(e))
    }
}

/// Seed the process-wide DRBG from the browser entropy source.
pub fn init_default() -> Result<(), Error> {
    init(&JsEntropy)
}

/// Seed the global DRBG from the browser source if it hasn't been seeded yet.
///
/// Called automatically by [`crate::fill`] on first use.
#[doc(hidden)]
pub fn try_auto_seed() -> Result<(), Error> {
    init(&JsEntropy)
}
