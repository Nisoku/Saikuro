use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    value::register(suite);
    value_map::register(suite);
    error::register(suite);
}

pub mod error;
pub mod value;
pub mod value_map;
