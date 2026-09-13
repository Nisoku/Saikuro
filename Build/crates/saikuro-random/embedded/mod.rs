use crate::shared::{init, EntropySource};
use alloc::string::ToString;
use saikuro_event::SaikuroError;

/// Seed the process-wide DRBG from an application-provided [`EntropySource`].
///
/// Call this once at startup after constructing the MCU's entropy source, e.g.
/// a hardware RNG peripheral. There is no default source on `embedded`.
pub fn init_from(source: &impl EntropySource) -> Result<(), SaikuroError> {
    init(source)
}

/// The `embedded` engine has no default entropy source, so auto-seed is a
/// no-op that reports the DRBG as unseeded until the application calls
/// [`init_from`].
#[doc(hidden)]
pub fn try_auto_seed() -> Result<(), SaikuroError> {
    Err(SaikuroError::Entropy(
        "DRBG used before being seeded".to_string(),
    ))
}
