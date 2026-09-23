use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    compliance::register(suite);
    memory_stress::register(suite);
    framing::register(suite);
    selector::register(suite);
}

pub mod compliance;
pub mod framing;
pub mod memory_stress;
pub mod selector;
