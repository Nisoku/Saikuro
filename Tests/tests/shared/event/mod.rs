use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    error::register(suite);
    sinks::register(suite);
    value::register(suite);
    value_map::register(suite);
}

pub mod error;
pub mod sinks;
pub mod value;
pub mod value_map;
