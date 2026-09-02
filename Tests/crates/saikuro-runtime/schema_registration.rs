use saikuro_core::schema::{
    FunctionMap, FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility,
};
use saikuro_core::RegistrationToken;
use saikuro_router::provider::{Provider, ProviderHandle, ProviderWorkItem};
use saikuro_runtime::SaikuroRuntime;

/// Smoke test: build a runtime, register a schema, verify lookup works.
#[test]
fn schema_registration_roundtrip() {
    saikuro_exec::block_on(async {
        let rt = SaikuroRuntime::builder().build().await;

        let mut functions = FunctionMap::new();
        functions.insert(
            "ping".to_owned(),
            FunctionSchema {
                args: vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::String),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: true,
                doc: Some("Returns 'pong'".to_owned()),
            },
        );

        let ns = NamespaceSchema {
            functions: Box::new(functions),
            doc: None,
        };

        let mut schema = Schema::new();
        schema.namespaces.insert("health".to_owned(), ns);

        rt.schema_registry()
            .merge_schema(schema, "test-provider")
            .await
            .expect("merge failed");

        let func_ref = rt
            .schema_registry()
            .lookup_function("health.ping")
            .await
            .expect("lookup failed");

        assert_eq!(func_ref.function, "ping");
        assert_eq!(func_ref.provider_id, "test-provider");
    });
}

#[test]
fn stale_same_id_cleanup_preserves_new_provider_and_schema() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();
        let old_token = RegistrationToken::new();
        let new_token = RegistrationToken::new();
        let (old_sender, _old_receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(
            saikuro_exec::ChannelCapacity::try_from(4).expect("4 is a valid channel capacity"),
        );
        let (new_sender, _new_receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(
            saikuro_exec::ChannelCapacity::try_from(4).expect("4 is a valid channel capacity"),
        );

        handle
            .register_provider(ProviderHandle::with_registration_token(
                "provider",
                old_token,
                vec!["service".into()],
                old_sender,
            ))
            .await;
        handle
            .register_schema_with_token(schema_for("service", "old"), "provider", old_token)
            .await
            .expect("old schema registers");
        handle
            .register_provider(ProviderHandle::with_registration_token(
                "provider",
                new_token,
                vec!["service".into()],
                new_sender,
            ))
            .await;
        handle
            .register_schema_with_token(schema_for("service", "new"), "provider", new_token)
            .await
            .expect("new schema registers");

        handle.deregister_provider("provider", old_token).await;

        let provider = runtime
            .provider_registry()
            .get("service")
            .await
            .expect("new provider remains routed");
        assert_eq!(provider.id(), "provider");
        assert_eq!(provider.registration_token(), new_token);
        assert!(runtime
            .schema_registry()
            .lookup_function("service.new")
            .await
            .is_ok());
    });
}

fn schema_for(namespace: &str, function: &str) -> Schema {
    let mut functions = FunctionMap::new();
    functions.insert(
        function.to_owned(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::String),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: true,
            doc: None,
        },
    );
    let mut schema = Schema::new();
    schema.namespaces.insert(
        namespace.to_owned(),
        NamespaceSchema {
            functions: Box::new(functions),
            doc: None,
        },
    );
    schema
}
