//! Runtime-level schema registration and stale-token cleanup tests.

use crate::shared_test;
use crate::Box;
use crate::TestSuite;
use saikuro_core::schema::{
    FunctionMap, FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility,
};
use saikuro_core::RegistrationToken;
use saikuro_router::provider::{Provider, ProviderHandle, ProviderWorkItem};
use saikuro_runtime::SaikuroRuntime;

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "runtime::schema_registration_roundtrip",
        schema_registration_roundtrip,
    );
    shared_test!(
        suite,
        "runtime::stale_same_id_cleanup_preserves_new_provider_and_schema",
        stale_same_id_cleanup_preserves_new_provider_and_schema,
    );
}

fn schema_for(namespace: &str, function: &str) -> Schema {
    let mut functions = FunctionMap::new();
    functions.insert(
        function.into(),
        FunctionSchema {
            args: crate::vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::String),
            visibility: Visibility::Public,
            capabilities: crate::vec![],
            idempotent: true,
            doc: None,
        },
    );
    let mut schema = Schema::new();
    schema.namespaces.insert(
        namespace.into(),
        NamespaceSchema {
            functions: Box::new(functions),
            doc: None,
        },
    );
    schema
}

fn schema_registration_roundtrip() -> Result<(), &'static str> {
    crate::block_on(async {
        let rt = SaikuroRuntime::builder().build().await;

        let mut functions = FunctionMap::new();
        functions.insert(
            "ping".into(),
            FunctionSchema {
                args: crate::vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::String),
                visibility: Visibility::Public,
                capabilities: crate::vec![],
                idempotent: true,
                doc: Some("Returns 'pong'".into()),
            },
        );
        let mut schema = Schema::new();
        schema.namespaces.insert(
            "health".into(),
            NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        );

        rt.schema_registry()
            .merge_schema(schema, "test-provider")
            .await
            .map_err(|_| "merge failed")?;

        let func_ref = rt
            .schema_registry()
            .lookup_function("health.ping")
            .await
            .map_err(|_| "lookup failed")?;

        assert_eq!(func_ref.function, "ping");
        assert_eq!(func_ref.provider_id, "test-provider");
        Ok(())
    })
}

fn stale_same_id_cleanup_preserves_new_provider_and_schema() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();
        let old_token = RegistrationToken::new();
        let new_token = RegistrationToken::new();
        let capacity = saikuro_exec::ChannelCapacity::try_from(4).map_err(|_| "capacity 4")?;
        let (old_sender, _old_receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(capacity);
        let (new_sender, _new_receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(capacity);

        handle
            .register_provider(ProviderHandle::with_registration_token(
                "provider",
                old_token,
                crate::vec!["service".into()],
                old_sender,
            ))
            .await;
        handle
            .register_schema_with_token(schema_for("service", "old"), "provider", old_token)
            .await
            .map_err(|_| "old schema registers")?;
        handle
            .register_provider(ProviderHandle::with_registration_token(
                "provider",
                new_token,
                crate::vec!["service".into()],
                new_sender,
            ))
            .await;
        handle
            .register_schema_with_token(schema_for("service", "new"), "provider", new_token)
            .await
            .map_err(|_| "new schema registers")?;

        handle.deregister_provider("provider", old_token).await;

        let provider = runtime
            .provider_registry()
            .get("service")
            .await
            .ok_or("new provider remains routed")?;
        assert_eq!(provider.id(), "provider");
        assert_eq!(provider.registration_token(), new_token);
        crate::check_test!(
            runtime
                .schema_registry()
                .lookup_function("service.new")
                .await
                .is_ok(),
            "schema registered under the new token must survive stale cleanup"
        );
        Ok(())
    })
}
