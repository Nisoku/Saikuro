use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    drbg::register(suite);
}

pub mod drbg;
