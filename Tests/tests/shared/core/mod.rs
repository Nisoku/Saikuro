use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    envelope_roundtrip::register(suite);
    value::register(suite);
    invocation::register(suite);
    resource::register(suite);
    error_propagation::register(suite);
}

pub mod envelope_roundtrip;
pub mod error_propagation;
pub mod invocation;
pub mod resource;
pub mod value;
