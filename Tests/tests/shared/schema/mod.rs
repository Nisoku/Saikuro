use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    schema_validation::register(suite);
    capability_enforcement::register(suite);
    registry::register(suite);
}

pub mod capability_enforcement;
pub mod registry;
pub mod schema_validation;
