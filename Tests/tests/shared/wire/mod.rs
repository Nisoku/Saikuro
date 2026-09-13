use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    wire::register(suite);
}

// nested `wire` dir + `wire` submodule
#[allow(clippy::module_inception)]
pub mod wire;
