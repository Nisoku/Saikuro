//! Transport tests running on the embedded engines (QEMU).

#[cfg(feature = "embedded")]
mod embedded_io;

#[cfg(feature = "embedded")]
pub fn register(suite: &mut saikuro_tests::TestSuite) {
    embedded_io::register(suite);
}

/// No-op when the embedded engine is not selected.
#[cfg(not(feature = "embedded"))]
pub fn register(_suite: &mut saikuro_tests::TestSuite) {}