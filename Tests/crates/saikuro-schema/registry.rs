use saikuro_core::schema::{PrimitiveType, Schema, TypeDefinition, TypeDescriptor};
use saikuro_core::RegistrationToken;
use saikuro_event::SaikuroError;
use saikuro_schema::registry::SchemaRegistry;

#[test]
fn frozen_registry_rejects_type_only_merge() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::from_frozen_schema(Schema::new());
        let mut update = Schema::new();
        update.types.insert(
            "UserId".into(),
            TypeDefinition::Alias {
                inner: TypeDescriptor::primitive(PrimitiveType::String),
            },
        );

        assert!(matches!(
            registry.merge_schema(update, "provider").await,
            Err(SaikuroError::FrozenSchema(_))
        ));
        assert!(registry
            .snapshot()
            .await
            .expect("snapshot")
            .types
            .is_empty());
    });
}

#[test]
fn stale_same_id_deregistration_preserves_new_schema() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let old_token = RegistrationToken::new();
        let new_token = RegistrationToken::new();
        let mut old_schema = Schema::new();
        old_schema
            .namespaces
            .insert("service".into(), empty_namespace());
        let mut new_schema = Schema::new();
        new_schema
            .namespaces
            .insert("service".into(), empty_namespace());

        registry
            .merge_schema_with_token(old_schema, "provider", old_token)
            .await
            .expect("old schema registers");
        registry
            .merge_schema_with_token(new_schema, "provider", new_token)
            .await
            .expect("new schema registers");
        registry.deregister_provider("provider", old_token).await;

        assert!(registry.has_namespace("service").await);
    });
}

fn empty_namespace() -> saikuro_core::schema::NamespaceSchema {
    saikuro_core::schema::NamespaceSchema {
        functions: Box::default(),
        doc: None,
    }
}
