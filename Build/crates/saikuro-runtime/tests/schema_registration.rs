use saikuro_core::schema::{
    FunctionMap, FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility,
};
use saikuro_core::RegistrationToken;
use saikuro_router::provider::{Provider, ProviderHandle, ProviderWorkItem};
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

#[test]
fn stale_same_id_cleanup_preserves_new_provider_and_schema() {
    let runtime = SaikuroRuntime::builder().build();
    let handle = runtime.handle();
    let old_token = RegistrationToken::new();
    let new_token = RegistrationToken::new();
    let (old_sender, _old_receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(
        saikuro_exec::ChannelCapacity::try_from(4).expect("4 is a valid channel capacity"),
    );
    let (new_sender, _new_receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(
        saikuro_exec::ChannelCapacity::try_from(4).expect("4 is a valid channel capacity"),
    );

    handle.register_provider(ProviderHandle::with_registration_token(
        "provider",
        old_token,
        vec!["service".into()],
        old_sender,
    ));
    handle
        .register_schema_with_token(schema_for("service", "old"), "provider", old_token)
        .expect("old schema registers");
    handle.register_provider(ProviderHandle::with_registration_token(
        "provider",
        new_token,
        vec!["service".into()],
        new_sender,
    ));
    handle
        .register_schema_with_token(schema_for("service", "new"), "provider", new_token)
        .expect("new schema registers");

    handle.deregister_provider("provider", old_token);

    let provider = runtime
        .provider_registry()
        .get("service")
        .expect("new provider remains routed");
    assert_eq!(provider.id(), "provider");
    assert_eq!(provider.registration_token(), new_token);
    assert!(runtime
        .schema_registry()
        .lookup_function("service.new")
        .is_ok());
}

fn schema_for(namespace: &str, function: &str) -> Schema {
    let mut functions = FunctionMap::new();
    functions
        .insert(
            function.to_owned(),
            FunctionSchema {
                args: vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::String),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: true,
                doc: None,
            },
        )
        .expect("function fits");
    let mut schema = Schema::new();
    schema
        .namespaces
        .insert(
            namespace.to_owned(),
            NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        )
        .expect("namespace fits");
    schema
}
