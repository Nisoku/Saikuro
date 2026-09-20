use crate::TestSuite;

pub fn register(suite: &mut TestSuite) {
    schema_validation::register(suite);
    capability_enforcement::register(suite);
    registry::register(suite);
    registry_edges::register(suite);
    builder::register(suite);
}

pub mod builder;
pub mod capability_enforcement;
pub mod registry;
pub mod registry_edges;
pub mod schema_validation;
