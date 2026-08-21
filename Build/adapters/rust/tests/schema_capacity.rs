use saikuro::schema::{FunctionSchema, NamespaceSchema};
use saikuro::Error;
use saikuro_core::schema::SCHEMA_FUNCTIONS_CAPACITY;

#[test]
fn to_core_overflow_functions_returns_capacity_error() {
    let mut ns = NamespaceSchema::new();
    for i in 0..=SCHEMA_FUNCTIONS_CAPACITY {
        ns.insert(format!("fn_{i}"), FunctionSchema::default());
    }
    let err = ns.to_core().unwrap_err();
    assert!(matches!(err, Error::SchemaCapacityExceeded));
}
