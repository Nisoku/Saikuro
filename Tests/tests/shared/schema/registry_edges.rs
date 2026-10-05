//! `SchemaRegistry` lifecycle edge cases: freeze, registration, discovery,
//! snapshot filtering, and capacity limits.

use crate::check_test;
use crate::shared_test;
use crate::Box;
use crate::TestSuite;
use saikuro_core::schema::{
    FunctionMap, NamespaceSchema, PrimitiveType, Schema, TypeDefinition, TypeDescriptor,
    SCHEMA_NAMESPACES_CAPACITY,
};
use saikuro_core::RegistrationToken;
use saikuro_event::SaikuroError;
use saikuro_schema::registry::{NamespaceRegistration, RegistryMode, SchemaRegistry};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "schema::frozen_schema_preloads_namespaces_and_types",
        frozen_schema_preloads_namespaces_and_types,
    );
    shared_test!(
        suite,
        "schema::register_replaces_existing_namespace",
        register_replaces_existing_namespace,
    );
    shared_test!(
        suite,
        "schema::register_rejected_after_freeze",
        register_rejected_after_freeze,
    );
    shared_test!(
        suite,
        "schema::merge_schema_with_token_roundtrip",
        merge_schema_with_token_roundtrip,
    );
    shared_test!(
        suite,
        "schema::deregister_removes_only_matching_token",
        deregister_removes_only_matching_token,
    );
    shared_test!(
        suite,
        "schema::lookup_reports_malformed_target",
        lookup_reports_malformed_target,
    );
    shared_test!(
        suite,
        "schema::snapshot_filtered_drops_empty_namespaces",
        snapshot_filtered_drops_empty_namespaces,
    );
    shared_test!(
        suite,
        "schema::provider_for_namespace_missing_returns_none",
        provider_for_namespace_missing_returns_none,
    );
    shared_test!(
        suite,
        "schema::namespace_register_respects_capacity",
        namespace_register_respects_capacity,
    );
}

fn simple_namespace(name: &str) -> NamespaceSchema {
    let mut functions = FunctionMap::new();
    functions.insert(
        name.into(),
        saikuro_core::schema::FunctionSchema {
            args: crate::vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: saikuro_core::schema::Visibility::Public,
            capabilities: crate::vec![],
            idempotent: false,
            doc: None,
        },
    );
    NamespaceSchema {
        functions: Box::new(functions),
        doc: None,
    }
}

fn schema_with(namespace: &str, function: &str) -> Schema {
    let mut schema = Schema::new();
    schema
        .namespaces
        .insert(namespace.into(), simple_namespace(function));
    schema
}

fn frozen_schema_preloads_namespaces_and_types() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut schema = Schema::new();
        schema
            .namespaces
            .insert("math".into(), simple_namespace("add"));
        schema.types.insert(
            "UserId".into(),
            TypeDefinition::Alias {
                inner: TypeDescriptor::primitive(PrimitiveType::String),
            },
        );

        let registry = SchemaRegistry::from_frozen_schema(schema);
        check_test!(
            registry.mode().await == RegistryMode::Production,
            "frozen schema must start in production mode"
        );
        check_test!(
            registry.has_namespace("math").await,
            "preloaded namespace must be visible"
        );
        let func_ref = registry
            .lookup_function("math.add")
            .await
            .map_err(|_| "lookup preloaded function")?;
        check_test!(func_ref.provider_id == "frozen", "frozen entries are owned");
        let snapshot = registry.snapshot().await.map_err(|_| "snapshot")?;
        check_test!(
            snapshot.namespaces.get("math").is_some(),
            "snapshot keeps preloaded namespace"
        );
        check_test!(
            snapshot.types.get("UserId").is_some(),
            "snapshot keeps preloaded types"
        );
        Ok(())
    })
}

fn register_replaces_existing_namespace() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let first_token = RegistrationToken::new();
        let second_token = RegistrationToken::new();

        let first_registration = NamespaceRegistration {
            namespace: "svc".into(),
            schema: simple_namespace("old"),
            provider_id: "p1".into(),
            registration_token: first_token,
        };
        registry
            .register(first_registration)
            .await
            .map_err(|_| "first register")?;

        let second_registration = NamespaceRegistration {
            namespace: "svc".into(),
            schema: simple_namespace("new"),
            provider_id: "p2".into(),
            registration_token: second_token,
        };
        registry
            .register(second_registration)
            .await
            .map_err(|_| "second register")?;

        check_test!(
            registry.lookup_function("svc.new").await.is_ok(),
            "replacement schema must be live"
        );
        check_test!(
            registry.lookup_function("svc.old").await.is_err(),
            "replaced schema must be gone"
        );
        check_test!(
            registry.provider_for_namespace("svc").await.as_deref() == Some("p2"),
            "provider id must track the latest registration"
        );
        Ok(())
    })
}

fn register_rejected_after_freeze() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        registry
            .merge_schema(schema_with("svc", "run"), "p")
            .await
            .map_err(|_| "merge before freeze")?;
        registry.freeze().await;
        check_test!(
            registry.mode().await == RegistryMode::Production,
            "freeze must flip mode"
        );

        let result = registry
            .register(NamespaceRegistration {
                namespace: "late".into(),
                schema: simple_namespace("f"),
                provider_id: "p".into(),
                registration_token: RegistrationToken::new(),
            })
            .await;
        check_test!(
            matches!(result, Err(SaikuroError::FrozenSchema(_))),
            "register must be rejected in production mode"
        );
        check_test!(
            !registry.has_namespace("late").await,
            "rejected register must not mutate state"
        );
        Ok(())
    })
}

fn merge_schema_with_token_roundtrip() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let token = RegistrationToken::new();
        registry
            .merge_schema_with_token(schema_with("svc", "run"), "provider", token)
            .await
            .map_err(|_| "merge with token")?;

        let func_ref = registry
            .lookup_function("svc.run")
            .await
            .map_err(|_| "lookup")?;
        check_test!(func_ref.namespace == "svc", "namespace resolved");
        check_test!(func_ref.function == "run", "function resolved");
        check_test!(func_ref.provider_id == "provider", "provider resolved");
        check_test!(
            func_ref.schema.returns == TypeDescriptor::primitive(PrimitiveType::Unit),
            "schema content resolved"
        );
        Ok(())
    })
}

fn deregister_removes_only_matching_token() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let old_token = RegistrationToken::new();
        let new_token = RegistrationToken::new();

        registry
            .merge_schema_with_token(schema_with("a", "f"), "provider", old_token)
            .await
            .map_err(|_| "register a")?;
        registry
            .merge_schema_with_token(schema_with("b", "g"), "provider", new_token)
            .await
            .map_err(|_| "register b")?;

        registry.deregister_provider("provider", old_token).await;
        check_test!(
            !registry.has_namespace("a").await,
            "namespace under the stale token must be removed"
        );
        check_test!(
            registry.has_namespace("b").await,
            "namespace under the live token must survive"
        );
        Ok(())
    })
}

fn lookup_reports_malformed_target() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let result = registry.lookup_function("no-dot-here").await;
        check_test!(
            matches!(result, Err(SaikuroError::MalformedTarget(_))),
            "malformed target must map to MalformedTarget"
        );
        Ok(())
    })
}

fn snapshot_filtered_drops_empty_namespaces() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let mut keep = schema_with("keep", "run");
        keep.namespaces
            .insert("junk".into(), simple_namespace("discard"));
        registry
            .merge_schema(keep, "provider")
            .await
            .map_err(|_| "merge")?;

        let snapshot = registry
            .snapshot_filtered(|_ns, fn_name, _schema| fn_name == "run")
            .await
            .map_err(|_| "snapshot_filtered")?;
        check_test!(
            snapshot.namespaces.get("keep").is_some(),
            "namespace with a kept function must remain"
        );
        check_test!(
            snapshot.namespaces.get("junk").is_none(),
            "namespace with no kept functions must be dropped"
        );
        Ok(())
    })
}

fn provider_for_namespace_missing_returns_none() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        check_test!(
            registry.provider_for_namespace("absent").await.is_none(),
            "missing namespace must yield None"
        );
        Ok(())
    })
}

fn namespace_register_respects_capacity() -> Result<(), &'static str> {
    const FOOTPRINT: usize = 140 * 1024;
    crate::capacity::require_capacity(FOOTPRINT)?;

    crate::block_on(async {
        let registry = SchemaRegistry::new();
        for i in 0..SCHEMA_NAMESPACES_CAPACITY {
            let name = crate::format!("ns{i}");
            let schema = {
                let mut s = Schema::new();
                s.namespaces.insert(name.clone(), simple_namespace("f"));
                s
            };
            registry
                .merge_schema(schema, "provider")
                .await
                .map_err(|_| "namespace at capacity must register")?;
        }

        let overflow = {
            let mut s = Schema::new();
            s.namespaces
                .insert("ns_overflow".into(), simple_namespace("f"));
            s
        };
        let result = registry.merge_schema(overflow, "provider").await;
        check_test!(
            matches!(result, Err(SaikuroError::SchemaCapacity)),
            "a namespace beyond the fixed capacity must be rejected"
        );
        check_test!(
            !registry.has_namespace("ns_overflow").await,
            "rejected namespace must not appear"
        );
        Ok(())
    })
}
