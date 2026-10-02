use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    config::register(suite);
    util_tests::register(suite);
    #[cfg(feature = "sqlite")]
    sqlite_tests::register(suite);
}

pub mod config;
#[cfg(feature = "sqlite")]
pub mod sqlite_tests;
pub mod util_tests;
