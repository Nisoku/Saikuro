use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    wire::register(suite);
}

pub mod wire;
