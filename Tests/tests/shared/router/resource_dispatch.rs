//! Resource-envelope dispatch integration tests.

use crate::check_test;
use crate::common;
use crate::shared_test;
use crate::TestSuite;
use saikuro_core::{
    envelope::{Envelope, InvocationType},
    resource::ResourceHandle,
};
use saikuro_event::{ErrorCode, Value, ValueMap};
use saikuro_exec::mpsc;
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::InvocationRouter,
};
use saikuro_schema::registry::SchemaRegistry;

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "router::resource_envelope_constructor_sets_correct_type",
        resource_envelope_constructor_sets_correct_type,
    );
    shared_test!(
        suite,
        "router::resource_envelope_routes_as_call",
        resource_envelope_routes_as_call,
    );
    shared_test!(
        suite,
        "router::resource_envelope_returns_handle_from_provider",
        resource_envelope_returns_handle_from_provider,
    );
    shared_test!(
        suite,
        "router::resource_to_unknown_namespace_returns_no_provider",
        resource_to_unknown_namespace_returns_no_provider,
    );
    shared_test!(
        suite,
        "router::resource_to_dropped_provider_returns_unavailable",
        resource_to_dropped_provider_returns_unavailable,
    );
    shared_test!(
        suite,
        "router::resource_handle_minimal_roundtrips_through_value",
        resource_handle_minimal_roundtrips_through_value,
    );
    shared_test!(
        suite,
        "router::resource_handle_full_roundtrips_through_value",
        resource_handle_full_roundtrips_through_value,
    );
    shared_test!(
        suite,
        "router::resource_dispatch_through_connection_handler",
        resource_dispatch_through_connection_handler,
    );
    shared_test!(
        suite,
        "router::resource_to_unknown_namespace_via_handler_returns_namespace_not_found",
        resource_to_unknown_namespace_via_handler_returns_namespace_not_found,
    );
    shared_test!(
        suite,
        "router::resource_response_id_matches_request_id",
        resource_response_id_matches_request_id,
    );
    shared_test!(
        suite,
        "router::concurrent_resource_invocations_all_succeed",
        concurrent_resource_invocations_all_succeed,
    );
    shared_test!(
        suite,
        "router::resource_handle_from_value_rejects_non_map",
        resource_handle_from_value_rejects_non_map,
    );
    shared_test!(
        suite,
        "router::resource_handle_from_value_rejects_missing_id",
        resource_handle_from_value_rejects_missing_id,
    );
}

/// Spawn a background task that answers every work item with `result_value`.
fn spawn_responder(
    mut work_rx: mpsc::Receiver<ProviderWorkItem>,
    result_value: Value,
) -> saikuro_exec::JoinHandle<()> {
    saikuro_exec::spawn(async move {
        while let Some(item) = work_rx.recv().await {
            if let Some(tx) = item.response_tx {
                let _ = tx.send(saikuro_core::ResponseEnvelope::ok(
                    item.envelope.id,
                    result_value.clone(),
                ));
            }
        }
    })
}

/// Build a `Value` that encodes a given `ResourceHandle` (same as the wire format).
fn handle_to_value(handle: &ResourceHandle) -> Value {
    handle.to_value()
}

fn resource_envelope_constructor_sets_correct_type() -> Result<(), &'static str> {
    let env = Envelope::resource(
        "files.open",
        crate::vec![Value::String("/tmp/data.csv".into())],
    )
    .map_err(|_| "create")?;
    assert_eq!(env.invocation_type, InvocationType::Resource);
    assert_eq!(env.target, "files.open");
    assert_eq!(env.args.len(), 1);
    Ok(())
}

fn resource_envelope_routes_as_call() -> Result<(), &'static str> {
    crate::block_on(async {
        let handle = ResourceHandle::new("abc-001")
            .with_mime_type("text/csv")
            .with_size(8192)
            .with_uri("saikuro://res/abc-001");
        let result_value = handle_to_value(&handle);

        let (registry, work_rx) = common::make_provider("files").await;
        let _responder = spawn_responder(work_rx, result_value);

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::resource(
            "files.open",
            crate::vec![Value::String("/tmp/data.csv".into())],
        )
        .map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(resp.ok, "resource dispatch should succeed");
        check_test!(resp.result.is_some(), "result must be present");
        Ok(())
    })
}

fn resource_envelope_returns_handle_from_provider() -> Result<(), &'static str> {
    crate::block_on(async {
        let original_handle = ResourceHandle::new("xyz-999")
            .with_mime_type("application/octet-stream")
            .with_size(65536)
            .with_uri("https://storage.example.com/blobs/xyz-999");
        let result_value = handle_to_value(&original_handle);

        let (registry, work_rx) = common::make_provider("storage").await;
        let _responder = spawn_responder(work_rx, result_value);

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::resource("storage.get", crate::vec![Value::String("xyz-999".into())])
            .map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(resp.ok, "resource dispatch should succeed");

        let raw_result = resp.result.ok_or("result must be present")?;
        let decoded_handle =
            ResourceHandle::from_value(&raw_result).ok_or("result must decode into a handle")?;

        assert_eq!(decoded_handle.id, original_handle.id);
        assert_eq!(decoded_handle.mime_type, original_handle.mime_type);
        assert_eq!(decoded_handle.size, original_handle.size);
        assert_eq!(decoded_handle.uri, original_handle.uri);
        Ok(())
    })
}

fn resource_to_unknown_namespace_returns_no_provider() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let env = Envelope::resource("missing.open", crate::vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(!resp.ok, "should fail for unknown namespace");
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(ErrorCode::NoProvider)
        );
        Ok(())
    })
}

fn resource_to_dropped_provider_returns_unavailable() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) =
            mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let handle = ProviderHandle::new("gone", crate::vec!["blobs".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        drop(work_rx); // provider vanished

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::resource("blobs.get", crate::vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(!resp.ok, "should fail for dropped provider");
        let err = resp.error.as_ref().map(|e| e.code.clone()).unwrap();
        assert!(
            err == ErrorCode::ProviderUnavailable || err == ErrorCode::NoProvider,
            "expected ProviderUnavailable or NoProvider, got {:?}",
            err
        );
        Ok(())
    })
}

fn resource_handle_minimal_roundtrips_through_value() -> Result<(), &'static str> {
    let original = ResourceHandle::new("minimal-id");
    let v = original.to_value();
    let decoded = ResourceHandle::from_value(&v).ok_or("must decode")?;

    assert_eq!(decoded.id, "minimal-id");
    assert!(decoded.mime_type.is_none());
    assert!(decoded.size.is_none());
    assert!(decoded.uri.is_none());
    Ok(())
}

fn resource_handle_full_roundtrips_through_value() -> Result<(), &'static str> {
    let original = ResourceHandle::new("full-id")
        .with_mime_type("image/png")
        .with_size(4096)
        .with_uri("file:///var/images/full-id.png");

    let v = original.to_value();
    let decoded = ResourceHandle::from_value(&v).ok_or("must decode")?;

    assert_eq!(decoded, original);
    Ok(())
}

fn resource_dispatch_through_connection_handler() -> Result<(), &'static str> {
    crate::block_on(async {
        let handle = ResourceHandle::new("handler-test-001")
            .with_mime_type("text/plain")
            .with_size(128);
        let result_value = handle_to_value(&handle);

        let (provider_registry, work_rx) = common::make_provider("docs").await;
        let _responder = spawn_responder(work_rx, result_value.clone());

        let schema_registry = SchemaRegistry::new();
        common::register_namespace(&schema_registry, "docs", "fetch").await;

        let env = Envelope::resource("docs.fetch", crate::vec![]).map_err(|_| "create")?;

        let resp = common::round_trip_via_handler(schema_registry, provider_registry, env).await;

        check_test!(resp.ok, "handler resource dispatch should succeed");
        let raw_result = resp.result.ok_or("result must be present")?;
        let decoded = ResourceHandle::from_value(&raw_result).ok_or("must decode handle")?;
        assert_eq!(decoded.id, "handler-test-001");
        assert_eq!(decoded.mime_type.as_deref(), Some("text/plain"));
        assert_eq!(decoded.size, Some(128));
        Ok(())
    })
}

fn resource_to_unknown_namespace_via_handler_returns_namespace_not_found(
) -> Result<(), &'static str> {
    crate::block_on(async {
        let schema_registry = SchemaRegistry::new();
        let provider_registry = ProviderRegistry::new();

        let env = Envelope::resource("unknown_ns.open", crate::vec![]).map_err(|_| "create")?;
        let resp = common::round_trip_via_handler(schema_registry, provider_registry, env).await;

        check_test!(!resp.ok, "should fail for unregistered namespace");
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(ErrorCode::NamespaceNotFound),
            "validator must reject unknown namespace before routing"
        );
        Ok(())
    })
}

fn resource_response_id_matches_request_id() -> Result<(), &'static str> {
    crate::block_on(async {
        let handle = ResourceHandle::new("corr-001");
        let result_value = handle_to_value(&handle);

        let (registry, work_rx) = common::make_provider("corr").await;
        let _responder = spawn_responder(work_rx, result_value);

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::resource("corr.get", crate::vec![]).map_err(|_| "create")?;
        let request_id = env.id;
        let resp = router.dispatch(env).await;

        check_test!(resp.ok, "call must succeed");
        assert_eq!(
            resp.id, request_id,
            "response ID must match the request ID for call-semantic correlation"
        );
        Ok(())
    })
}

fn concurrent_resource_invocations_all_succeed() -> Result<(), &'static str> {
    crate::block_on(async {
        let handle = ResourceHandle::new("concurrent-test");
        let result_value = handle_to_value(&handle);

        let (registry, work_rx) = common::make_provider("bulk").await;
        let _responder = spawn_responder(work_rx, result_value);

        let router = InvocationRouter::with_providers(registry);
        let mut joins = crate::Vec::new();

        for _ in 0..10 {
            let r = router.clone();
            joins.push(saikuro_exec::spawn(async move {
                let env = Envelope::resource("bulk.fetch", crate::vec![]).map_err(|_| "create")?;
                Ok::<_, &'static str>(r.dispatch(env).await)
            }));
        }

        for join in joins {
            let resp = join.await.map_err(|_| "task must not panic")??;
            check_test!(resp.ok, "concurrent resource call should succeed");
            let raw = resp.result.ok_or("result must be present")?;
            let decoded = ResourceHandle::from_value(&raw).ok_or("must decode")?;
            assert_eq!(decoded.id, "concurrent-test");
        }
        Ok(())
    })
}

fn resource_handle_from_value_rejects_non_map() -> Result<(), &'static str> {
    let v = Value::Int(42);
    check_test!(
        ResourceHandle::from_value(&v).is_none(),
        "from_value must return None for a non-map value"
    );
    Ok(())
}

fn resource_handle_from_value_rejects_missing_id() -> Result<(), &'static str> {
    let mut map = ValueMap::new();
    map.insert("size".into(), Value::Int(100));
    let v = Value::Map(map);
    check_test!(
        ResourceHandle::from_value(&v).is_none(),
        "from_value must return None when 'id' is absent"
    );
    Ok(())
}
