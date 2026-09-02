use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    compliance::register(suite);
    memory_stress::register(suite);
}

pub mod compliance;
pub mod memory_stress;
