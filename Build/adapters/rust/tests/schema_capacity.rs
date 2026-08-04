use saikuro::schema::{build_schema, FunctionSchema, NamespaceSchema};
use saikuro::Error;
use saikuro_core::schema::{SCHEMA_FUNCTIONS_CAPACITY, SCHEMA_NAMESPACES_CAPACITY};
use std::collections::HashMap;

#[test]
fn to_core_overflow_functions_returns_capacity_error() {
    let mut ns = NamespaceSchema::new();
    for i in 0..=SCHEMA_FUNCTIONS_CAPACITY {
        ns.insert(format!("fn_{i}"), FunctionSchema::default());
    }
    let err = ns.to_core().unwrap_err();
    assert!(matches!(err, Error::SchemaCapacityExceeded));
}

#[test]
fn build_schema_overflow_namespaces_returns_capacity_error() {
    let mut namespaces = HashMap::new();
    for i in 0..=SCHEMA_NAMESPACES_CAPACITY {
        let mut ns = NamespaceSchema::new();
        ns.insert("f", FunctionSchema::default());
        namespaces.insert(format!("ns_{i}"), ns);
    }
    let err = build_schema(&namespaces).unwrap_err();
    assert!(matches!(err, Error::SchemaCapacityExceeded));
}
