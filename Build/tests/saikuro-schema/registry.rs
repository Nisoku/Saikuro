use saikuro_core::schema::{PrimitiveType, Schema, TypeDefinition, TypeDescriptor};
use saikuro_core::RegistrationToken;
use saikuro_schema::registry::{RegistryError, SchemaRegistry};

#[test]
fn frozen_registry_rejects_type_only_merge() {
    let registry = SchemaRegistry::from_frozen_schema(Schema::new());
    let mut update = Schema::new();
    update
        .types
        .insert(
            "UserId".into(),
            TypeDefinition::Alias {
                inner: TypeDescriptor::primitive(PrimitiveType::String),
            },
        )
        .expect("type fits");

    assert!(matches!(
        registry.merge_schema(update, "provider"),
        Err(RegistryError::FrozenSchema(_))
    ));
    assert!(registry.snapshot().expect("snapshot").types.is_empty());
}

#[test]
fn stale_same_id_deregistration_preserves_new_schema() {
    let registry = SchemaRegistry::new();
    let old_token = RegistrationToken::new();
    let new_token = RegistrationToken::new();
    let mut old_schema = Schema::new();
    old_schema
        .namespaces
        .insert("service".into(), empty_namespace())
        .expect("namespace fits");
    let mut new_schema = Schema::new();
    new_schema
        .namespaces
        .insert("service".into(), empty_namespace())
        .expect("namespace fits");

    registry
        .merge_schema_with_token(old_schema, "provider", old_token)
        .expect("old schema registers");
    registry
        .merge_schema_with_token(new_schema, "provider", new_token)
        .expect("new schema registers");
    registry.deregister_provider("provider", old_token);

    assert!(registry.has_namespace("service"));
}

fn empty_namespace() -> saikuro_core::schema::NamespaceSchema {
    saikuro_core::schema::NamespaceSchema {
        functions: Box::default(),
        doc: None,
    }
}
