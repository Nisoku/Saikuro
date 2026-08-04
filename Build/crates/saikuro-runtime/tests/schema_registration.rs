use saikuro_core::schema::{
    FunctionMap, FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility,
};
use saikuro_runtime::SaikuroRuntime;

/// Smoke test: build a runtime, register a schema, verify lookup works.
#[test]
fn schema_registration_roundtrip() {
    let rt = SaikuroRuntime::builder().build();

    let mut functions = FunctionMap::new();
    functions
        .insert(
            "ping".to_owned(),
            FunctionSchema {
                args: vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::String),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: true,
                doc: Some("Returns 'pong'".to_owned()),
            },
        )
        .ok();

    let ns = NamespaceSchema {
        functions: Box::new(functions),
        doc: None,
    };

    let mut schema = Schema::new();
    schema.namespaces.insert("health".to_owned(), ns).ok();

    rt.schema_registry()
        .merge_schema(schema, "test-provider")
        .expect("merge failed");

    let func_ref = rt
        .schema_registry()
        .lookup_function("health.ping")
        .expect("lookup failed");

    assert_eq!(func_ref.function, "ping");
    assert_eq!(func_ref.provider_id, "test-provider");
}
