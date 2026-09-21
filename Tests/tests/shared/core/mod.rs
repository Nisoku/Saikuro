use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    edge::register(suite);
    envelope_access::register(suite);
    envelope_roundtrip::register(suite);
    value::register(suite);
    invocation::register(suite);
    resource::register(suite);
    error_propagation::register(suite);
    frame_classify::register(suite);
    relay_fidelity::register(suite);
}

pub mod edge;
pub mod envelope_access;
pub mod envelope_roundtrip;
pub mod error_propagation;
pub mod frame_classify;
pub mod invocation;
pub mod relay_fidelity;
pub mod resource;
pub mod value;
