//! Storage-backend tests run on the embedded engine (QEMU).

#[cfg(feature = "flash")]
mod flash;

#[cfg(feature = "flash")]
pub fn register(suite: &mut saikuro_tests::TestSuite) {
    flash::register(suite);
}

/// No-op when flash is disabled.
#[cfg(not(feature = "flash"))]
pub fn register(_suite: &mut saikuro_tests::TestSuite) {}