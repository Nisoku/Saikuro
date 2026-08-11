use saikuro_core::schema::{PrimitiveType, Schema, TypeDefinition, TypeDescriptor};
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
