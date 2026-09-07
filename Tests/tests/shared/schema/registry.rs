//! Schema registry lifecycle tests.

use crate::check_test;
use crate::shared_test;
use crate::TestSuite;
use crate::Box;
use saikuro_core::schema::{NamespaceSchema, PrimitiveType, Schema, TypeDefinition, TypeDescriptor};
use saikuro_core::RegistrationToken;
use saikuro_event::SaikuroError;
use saikuro_schema::registry::SchemaRegistry;

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite,
        "schema::frozen_registry_rejects_type_only_merge",
        frozen_registry_rejects_type_only_merge,
    );
    shared_test!(suite,
        "schema::stale_same_id_deregistration_preserves_new_schema",
        stale_same_id_deregistration_preserves_new_schema,
    );
}

fn empty_namespace() -> NamespaceSchema {
    NamespaceSchema {
        functions: Box::default(),
        doc: None,
    }
}

fn frozen_registry_rejects_type_only_merge() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::from_frozen_schema(Schema::new());
        let mut update = Schema::new();
        update.types.insert(
            "UserId".into(),
            TypeDefinition::Alias {
                inner: TypeDescriptor::primitive(PrimitiveType::String),
            },
        );

        check_test!(
            matches!(
                registry.merge_schema(update, "provider").await,
                Err(SaikuroError::FrozenSchema(_))
            ),
            "frozen registry must reject any merge"
        );
        let snapshot = registry
            .snapshot()
            .await
            .map_err(|_| "snapshot")?;
        check_test!(
            snapshot.types.is_empty(),
            "rejected merge must not mutate the frozen schema"
        );
        Ok(())
    })
}

fn stale_same_id_deregistration_preserves_new_schema() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let old_token = RegistrationToken::new();
        let new_token = RegistrationToken::new();
        let mut old_schema = Schema::new();
        old_schema.namespaces.insert("service".into(), empty_namespace());
        let mut new_schema = Schema::new();
        new_schema.namespaces.insert("service".into(), empty_namespace());

        registry
            .merge_schema_with_token(old_schema, "provider", old_token)
            .await
            .map_err(|_| "old schema registers")?;
        registry
            .merge_schema_with_token(new_schema, "provider", new_token)
            .await
            .map_err(|_| "new schema registers")?;
        registry.deregister_provider("provider", old_token).await;

        check_test!(
            registry.has_namespace("service").await,
            "stale token must not remove the schema registered under the new token"
        );
        Ok(())
    })
}