//! Process-unique provider registration identity.

use portable_atomic::{AtomicU64, Ordering};

static NEXT_REGISTRATION_TOKEN: AtomicU64 = AtomicU64::new(1);

/// Opaque, monotonically increasing identity for one provider registration.
///
/// A token distinguishes successive connections that use the same provider ID.
/// Tokens are unique for the lifetime of the process and are not wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistrationToken(u64);

impl RegistrationToken {
    /// Allocate the next process-unique registration token.
    ///
    /// Panics if all `u64` token values have been exhausted. The counter does
    /// not wrap, so a token is never reused within a process.
    pub fn new() -> Self {
        let value = NEXT_REGISTRATION_TOKEN
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("registration token space exhausted without wrapping");
        Self(value)
    }
}

impl Default for RegistrationToken {
    fn default() -> Self {
        Self::new()
    }
}
