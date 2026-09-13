use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    util_tests::register(suite);
    #[cfg(feature = "sqlite")]
    sqlite_tests::register(suite);
}

#[cfg(feature = "sqlite")]
pub mod sqlite_tests;
pub mod util_tests;
