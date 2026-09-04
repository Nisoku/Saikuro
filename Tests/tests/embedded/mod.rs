//! Tests that exercise the embedded engines.

#[cfg(feature = "embedded")]
pub mod random;
#[cfg(feature = "embedded")]
pub mod storage;
#[cfg(feature = "embedded")]
pub mod transport;

/// Register every embedded-engine test suite.
#[cfg(feature = "embedded")]
pub fn register(suite: &mut saikuro_tests::TestSuite) {
    random::register(suite);
    storage::register(suite);
    transport::register(suite);
}

/// No-op when the embedded engine is not selected.
#[cfg(not(feature = "embedded"))]
pub fn register(_suite: &mut saikuro_tests::TestSuite) {}
