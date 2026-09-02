//! Host/native-only tests.

pub mod adapter;
pub mod core;
pub mod exec;
pub mod storage;

/// Register host-only tests into the shared [`TestSuite`] instance the native
/// runner owns.  Returns the same suite so callers can chain with
/// `shared::register_all`.
pub fn register(suite: &mut saikuro_tests::TestSuite) {
    adapter::register(suite);
    core::register(suite);
    exec::register(suite);
    storage::register(suite);
}
