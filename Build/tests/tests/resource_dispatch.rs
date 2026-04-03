//! Resource-envelope dispatch integration tests

use bytes::Bytes;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::{Envelope, InvocationType},
    error::ErrorCode,
    resource::ResourceHandle,
    schema::{FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility},
    value::Value,
    ResponseEnvelope,
};
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::{InvocationRouter, RouterConfig},
};
use saikuro_runtime::connection::ConnectionHandler;
use saikuro_schema::{
    capability_engine::CapabilityEngine, registry::SchemaRegistry, validator::InvocationValidator,
};
use saikuro_transport::{
    memory::MemoryTransport,
    traits::{Transport, TransportReceiver, TransportSender},
};
use tokio::sync::mpsc;

//  Helpers

/// Build a minimal [`Schema`] with one namespace and one zero-argument function
/// accepting any number of arguments (uses `any` type for the first arg if needed).
fn minimal_schema(namespace: &str, function: &str) -> Schema {
    let mut functions = std::collections::HashMap::new();
    functions.insert(
        function.to_owned(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Any),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );
    let mut namespaces = std::collections::HashMap::new();
    namespaces.insert(
        namespace.to_owned(),
        NamespaceSchema {
            functions,
            doc: None,
        },
    );
    Schema {
        version: 1,
        namespaces,
        types: std::collections::HashMap::new(),
    }
}

/// Register a namespace schema into a `SchemaRegistry` so the validator accepts
/// invocations to that namespace from a `ConnectionHandler`.
fn register_namespace(registry: &SchemaRegistry, namespace: &str, function: &str) {
    registry
        .merge_schema(minimal_schema(namespace, function), "test-provider")
        .expect("merge_schema must succeed");
}

/// Build a `ProviderRegistry` with a single provider subscribed to `namespace`.
fn make_provider(namespace: &str) -> (ProviderRegistry, mpsc::Receiver<ProviderWorkItem>) {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(64);
    let handle = ProviderHandle::new(
        format!("{namespace}-provider"),
        vec![namespace.to_owned()],
        work_tx,
    );
    let registry = ProviderRegistry::new();
    registry.register(handle);
    (registry, work_rx)
}

/// Spawn a background task that answers every work item with `result_value`.
fn spawn_responder(
    mut work_rx: mpsc::Receiver<ProviderWorkItem>,
    result_value: Value,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(item) = work_rx.recv().await {
            if let Some(tx) = item.response_tx {
                let _ = tx.send(ResponseEnvelope::ok(item.envelope.id, result_value.clone()));
            }
        }
    })
}

/// Build a `Value` that encodes a given `ResourceHandle` (same as the wire format).
fn handle_to_value(handle: &ResourceHandle) -> Value {
    handle.to_value()
}

/// Run a single envelope through a full `ConnectionHandler` (the same plumbing
/// used by the real runtime).  Mirrors the helper in `announce_dispatch.rs`.
async fn round_trip_via_handler(
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    envelope: Envelope,
) -> ResponseEnvelope {
    let (test_transport, handler_transport) = MemoryTransport::pair("test", "handler");
    let (handler_sender, handler_receiver) = handler_transport.split();
    let (mut test_sender, mut test_receiver) = test_transport.split();

    let router = InvocationRouter::new(provider_registry, RouterConfig::default());
    let validator = InvocationValidator::new(schema_registry.clone());
    let capability_engine = CapabilityEngine::default();

    let handler = ConnectionHandler {
        peer_id: "test-peer".to_owned(),
        sender: handler_sender,
        receiver: handler_receiver,
        validator,
        capability_engine,
        router,
        peer_capabilities: CapabilitySet::empty(),
        max_message_size: 4 * 1024 * 1024,
        schema_registry,
        provider_registry: ProviderRegistry::new(),
    };

    let frame = Bytes::from(envelope.to_msgpack().expect("encode envelope"));
    test_sender.send(frame).await.expect("send frame");
    drop(test_sender);

    handler.run().await;

    let resp_frame = test_receiver
        .recv()
        .await
        .expect("recv response")
        .expect("frame must be present");
    ResponseEnvelope::from_msgpack(&resp_frame).expect("decode response")
}

//  Tests

/// `Envelope::resource()` sets `InvocationType::Resource`:  a basic sanity
/// check before testing the full dispatch path.
#[test]
fn resource_envelope_constructor_sets_correct_type() {
    let env = Envelope::resource("files.open", vec![Value::String("/tmp/data.csv".into())]);
    assert_eq!(env.invocation_type, InvocationType::Resource);
    assert_eq!(env.target, "files.open");
    assert_eq!(env.args.len(), 1);
}

/// A `Resource` envelope is routed to the provider and the provider's response
/// is returned to the caller.
#[tokio::test]
async fn resource_envelope_routes_as_call() {
    let handle = ResourceHandle::new("abc-001")
        .with_mime_type("text/csv")
        .with_size(8192)
        .with_uri("saikuro://res/abc-001");
    let result_value = handle_to_value(&handle);

    let (registry, work_rx) = make_provider("files");
    let _responder = spawn_responder(work_rx, result_value);

    let router = InvocationRouter::with_providers(registry);
    let env = Envelope::resource("files.open", vec![Value::String("/tmp/data.csv".into())]);
    let resp = router.dispatch(env).await;

    assert!(
        resp.ok,
        "resource dispatch should succeed: {:?}",
        resp.error
    );
    assert!(resp.result.is_some(), "result must be present");
}

/// The provider's returned value can be decoded back into a `ResourceHandle`.
#[tokio::test]
async fn resource_envelope_returns_handle_from_provider() {
    let original_handle = ResourceHandle::new("xyz-999")
        .with_mime_type("application/octet-stream")
        .with_size(65536)
        .with_uri("https://storage.example.com/blobs/xyz-999");
    let result_value = handle_to_value(&original_handle);

    let (registry, work_rx) = make_provider("storage");
    let _responder = spawn_responder(work_rx, result_value);

    let router = InvocationRouter::with_providers(registry);
    let env = Envelope::resource("storage.get", vec![Value::String("xyz-999".into())]);
    let resp = router.dispatch(env).await;

    assert!(
        resp.ok,
        "resource dispatch should succeed: {:?}",
        resp.error
    );

    let raw_result = resp.result.expect("result must be present");
    let decoded_handle = ResourceHandle::from_value(&raw_result)
        .expect("result must deserialise to a ResourceHandle");

    assert_eq!(decoded_handle.id, original_handle.id);
    assert_eq!(decoded_handle.mime_type, original_handle.mime_type);
    assert_eq!(decoded_handle.size, original_handle.size);
    assert_eq!(decoded_handle.uri, original_handle.uri);
}

/// A `Resource` envelope whose target namespace has no registered provider
/// returns `NoProvider`.
#[tokio::test]
async fn resource_to_unknown_namespace_returns_no_provider() {
    let registry = ProviderRegistry::new(); // empty
    let router = InvocationRouter::with_providers(registry);

    let env = Envelope::resource("missing.open", vec![]);
    let resp = router.dispatch(env).await;

    assert!(!resp.ok, "should fail for unknown namespace");
    let err = resp.error.expect("error detail must be present");
    assert_eq!(
        err.code,
        ErrorCode::NoProvider,
        "expected NoProvider, got {:?}",
        err.code
    );
}

/// A `Resource` envelope sent to a dropped provider returns a provider
/// availability error.
#[tokio::test]
async fn resource_to_dropped_provider_returns_unavailable() {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(1);
    let handle = ProviderHandle::new("gone", vec!["blobs".to_owned()], work_tx);
    let registry = ProviderRegistry::new();
    registry.register(handle);
    drop(work_rx); // provider vanished

    let router = InvocationRouter::with_providers(registry);
    let env = Envelope::resource("blobs.get", vec![]);
    let resp = router.dispatch(env).await;

    assert!(!resp.ok, "should fail for dropped provider");
    let err = resp.error.expect("error detail must be present");
    assert!(
        err.code == ErrorCode::ProviderUnavailable || err.code == ErrorCode::NoProvider,
        "expected ProviderUnavailable or NoProvider, got {:?}",
        err.code
    );
}

/// A minimal `ResourceHandle` (only `id`, no optional fields) round-trips
/// through `Value` serialisation without data loss.
#[test]
fn resource_handle_minimal_roundtrips_through_value() {
    let original = ResourceHandle::new("minimal-id");
    let v = original.to_value();
    let decoded = ResourceHandle::from_value(&v).expect("must decode");

    assert_eq!(decoded.id, "minimal-id");
    assert!(decoded.mime_type.is_none());
    assert!(decoded.size.is_none());
    assert!(decoded.uri.is_none());
}

/// A fully-populated `ResourceHandle` round-trips through `Value` without data
/// loss.
#[test]
fn resource_handle_full_roundtrips_through_value() {
    let original = ResourceHandle::new("full-id")
        .with_mime_type("image/png")
        .with_size(4096)
        .with_uri("file:///var/images/full-id.png");

    let v = original.to_value();
    let decoded = ResourceHandle::from_value(&v).expect("must decode");

    assert_eq!(decoded, original);
}

/// Sending a `Resource` envelope through the full `ConnectionHandler` stack
/// succeeds end-to-end (encode -> frame -> decode -> validate -> route -> response).
///
/// The schema is pre-registered in the schema registry so the validator passes
/// the target function.  Resource invocations share the same call semantics and
/// routing path as `Call` invocations; this test confirms the end-to-end wire
/// path works correctly.
#[tokio::test]
async fn resource_dispatch_through_connection_handler() {
    let handle = ResourceHandle::new("handler-test-001")
        .with_mime_type("text/plain")
        .with_size(128);
    let result_value = handle_to_value(&handle);

    let (provider_registry, work_rx) = make_provider("docs");
    let _responder = spawn_responder(work_rx, result_value.clone());

    let schema_registry = SchemaRegistry::new();
    // Register the target function so the validator passes.
    register_namespace(&schema_registry, "docs", "fetch");

    let env = Envelope::resource("docs.fetch", vec![]);

    let resp = round_trip_via_handler(schema_registry, provider_registry, env).await;

    assert!(
        resp.ok,
        "handler resource dispatch should succeed: {:?}",
        resp.error
    );
    let raw_result = resp.result.expect("result must be present");
    let decoded = ResourceHandle::from_value(&raw_result).expect("must decode ResourceHandle");
    assert_eq!(decoded.id, "handler-test-001");
    assert_eq!(decoded.mime_type.as_deref(), Some("text/plain"));
    assert_eq!(decoded.size, Some(128));
}

/// Sending a `Resource` envelope to a namespace not in the schema registry
/// through the `ConnectionHandler` returns `NamespaceNotFound` (the validator
/// rejects it before it reaches the router).
#[tokio::test]
async fn resource_to_unknown_namespace_via_handler_returns_namespace_not_found() {
    let schema_registry = SchemaRegistry::new(); // empty:  no namespaces registered
    let provider_registry = ProviderRegistry::new();

    let env = Envelope::resource("unknown_ns.open", vec![]);
    let resp = round_trip_via_handler(schema_registry, provider_registry, env).await;

    assert!(!resp.ok, "should fail for unregistered namespace");
    let err = resp.error.expect("error detail must be present");
    assert_eq!(
        err.code,
        ErrorCode::NamespaceNotFound,
        "validator must reject unknown namespace before routing: got {:?}",
        err.code
    );
}

/// The `InvocationId` inside the response correlates to the original envelope.
#[tokio::test]
async fn resource_response_id_matches_request_id() {
    let handle = ResourceHandle::new("corr-001");
    let result_value = handle_to_value(&handle);

    let (registry, work_rx) = make_provider("corr");
    let _responder = spawn_responder(work_rx, result_value);

    let router = InvocationRouter::with_providers(registry);
    let env = Envelope::resource("corr.get", vec![]);
    let request_id = env.id;
    let resp = router.dispatch(env).await;

    assert!(resp.ok);
    assert_eq!(
        resp.id, request_id,
        "response ID must match the request ID for call-semantic correlation"
    );
}

/// Multiple concurrent `Resource` invocations all complete successfully and
/// their results are correctly correlated.
#[tokio::test]
async fn concurrent_resource_invocations_all_succeed() {
    // Each concurrent call gets its own handle with a unique id.
    // The echo-provider returns whatever value is sent:  here we just use a
    // static handle value; the important thing is that all futures complete.
    let handle = ResourceHandle::new("concurrent-test");
    let result_value = handle_to_value(&handle);

    let (registry, work_rx) = make_provider("bulk");
    let _responder = spawn_responder(work_rx, result_value);

    let router = InvocationRouter::with_providers(registry);
    let mut joins = Vec::new();

    for _ in 0..10 {
        let r = router.clone();
        joins.push(tokio::spawn(async move {
            let env = Envelope::resource("bulk.fetch", vec![]);
            r.dispatch(env).await
        }));
    }

    for join in joins {
        let resp = join.await.expect("task must not panic");
        assert!(
            resp.ok,
            "concurrent resource call should succeed: {:?}",
            resp.error
        );
        let raw = resp.result.expect("result must be present");
        let decoded = ResourceHandle::from_value(&raw).expect("must decode");
        assert_eq!(decoded.id, "concurrent-test");
    }
}

/// `ResourceHandle::from_value` returns `None` for a non-map `Value` (e.g. a plain integer).
#[test]
fn resource_handle_from_value_rejects_non_map() {
    let v = Value::Int(42);
    assert!(
        ResourceHandle::from_value(&v).is_none(),
        "from_value must return None for a non-map value"
    );
}

/// `ResourceHandle::from_value` returns `None` for a map that has no `id` field.
#[test]
fn resource_handle_from_value_rejects_missing_id() {
    // Build a Value::Map that lacks the "id" key.
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, Value> = BTreeMap::new();
    map.insert("size".to_owned(), Value::Int(100));
    let v = Value::Map(map);
    assert!(
        ResourceHandle::from_value(&v).is_none(),
        "from_value must return None when 'id' is absent"
    );
}
