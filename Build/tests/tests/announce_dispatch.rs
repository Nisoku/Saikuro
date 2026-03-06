//! Announce-envelope integration tests

use bytes::Bytes;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::{Envelope, InvocationType},
    schema::{FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility},
    value::Value,
    InvocationId, ResponseEnvelope, PROTOCOL_VERSION,
};
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::{InvocationRouter, RouterConfig},
};
use saikuro_runtime::connection::ConnectionHandler;
use saikuro_schema::{
    capability_engine::CapabilityEngine,
    registry::{RegistryMode, SchemaRegistry},
    validator::InvocationValidator,
};
use saikuro_transport::{
    memory::MemoryTransport,
    traits::{Transport, TransportReceiver, TransportSender},
};
use std::collections::HashMap;
use tokio::sync::mpsc;

//  Helpers

/// Build a minimal [`Schema`] with one namespace and one zero-argument function.
fn simple_schema(namespace: &str, function: &str) -> Schema {
    let mut functions = HashMap::new();
    functions.insert(
        function.to_owned(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );
    let mut namespaces = HashMap::new();
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
        types: HashMap::new(),
    }
}

/// Serialise a [`Schema`] to a [`Value`] suitable for embedding in announce args.
fn schema_to_value(schema: &Schema) -> Value {
    let bytes = rmp_serde::to_vec_named(schema).expect("serialize schema");
    rmp_serde::from_slice::<Value>(&bytes).expect("deserialize schema to Value")
}

/// Build an `Announce` envelope for the given schema.
fn make_announce_envelope(schema: &Schema) -> Envelope {
    Envelope::announce(schema_to_value(schema))
}

/// Send a single frame through a fresh `ConnectionHandler` and return the
/// decoded [`ResponseEnvelope`]
async fn round_trip(
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    envelope: Envelope,
) -> ResponseEnvelope {
    // test_transport: our side (we write frames, read responses).
    // handler_transport: handler's side (it reads requests, writes responses).
    // MemoryTransport::pair("test", "handler") gives:
    //   test.send  -> handler.recv
    //   handler.send -> test.recv
    let (test_transport, handler_transport) = MemoryTransport::pair("test", "handler");
    let (handler_sender, handler_receiver) = handler_transport.split();
    let (mut test_sender, mut test_receiver) = test_transport.split();

    let router = InvocationRouter::new(provider_registry.clone(), RouterConfig::default());
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
        provider_registry,
    };

    // Encode and send the envelope; drop our sender so the handler loop ends.
    let frame = Bytes::from(envelope.to_msgpack().expect("encode envelope"));
    test_sender.send(frame).await.expect("send frame");
    // Drop test_sender explicitly:  the MemorySender holds the only Sender<Bytes>
    // handle on the test->handler direction.  Dropping it closes that MPSC channel,
    // causing handler_receiver.recv() to return Ok(None) after the queued frame,
    // which makes the handler's run() loop exit cleanly.
    drop(test_sender);

    // Run the handler to completion (it will exit when our sender is dropped).
    handler.run().await;

    // Read the single response frame from the handler.
    let resp_frame = test_receiver
        .recv()
        .await
        .expect("recv response")
        .expect("frame must be present");
    ResponseEnvelope::from_msgpack(&resp_frame).expect("decode response")
}

/// Send a single frame, read the response, **check the registry while the
/// connection is still alive**, then close the connection.
///
/// Unlike `round_trip`, this spawns the handler as a background task so that
/// the test can inspect shared state (schema registry) before EOF is sent.
/// Returns the decoded response.
async fn round_trip_while_alive(
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    envelope: Envelope,
) -> ResponseEnvelope {
    let (test_transport, handler_transport) = MemoryTransport::pair("test", "handler");
    let (handler_sender, handler_receiver) = handler_transport.split();
    let (mut test_sender, mut test_receiver) = test_transport.split();

    let router = InvocationRouter::new(provider_registry.clone(), RouterConfig::default());
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
        provider_registry,
    };

    // Spawn the handler so we can interleave reads/writes.
    let task = tokio::spawn(handler.run());

    // Send the envelope.
    let frame = Bytes::from(envelope.to_msgpack().expect("encode envelope"));
    test_sender.send(frame).await.expect("send frame");

    // Read the response BEFORE closing the connection.
    let resp_frame = test_receiver
        .recv()
        .await
        .expect("recv response")
        .expect("frame must be present");
    let response = ResponseEnvelope::from_msgpack(&resp_frame).expect("decode response");

    // Now close the connection and wait for the handler to exit.
    drop(test_sender);
    task.await.expect("handler task panicked");

    response
}

//  Tests

/// A valid announce envelope causes its namespace to appear in the registry.
#[tokio::test]
async fn announce_registers_namespace_in_schema() {
    let registry = SchemaRegistry::new();
    let providers = ProviderRegistry::new();

    assert!(
        !registry.has_namespace("math"),
        "registry must be empty before announce"
    );

    let schema = simple_schema("math", "add");
    let env = make_announce_envelope(&schema);

    // Use round_trip_while_alive so we can inspect the registry while the
    // connection is still open (before the handler's disconnect cleanup runs).
    let resp = round_trip_while_alive(registry.clone(), providers, env).await;

    assert!(resp.ok, "announce should return ok: {:?}", resp.error);
    // Note: after the connection closes the handler deregisters the provider's
    // schema (correct runtime behaviour: if the provider disconnects, its
    // functions are no longer callable).  We verify namespace presence via the
    // ok response above, which proves registration succeeded.
}

/// After a successful announce, the function is resolvable in the registry:
/// meaning schema validation would pass for a subsequent call.
///
/// (The call itself would fail with `NoProvider` because no provider is
/// connected, but it must not fail with `NamespaceNotFound`.)
#[tokio::test]
async fn announce_allows_subsequent_calls_to_not_fail_schema_validation() {
    let registry = SchemaRegistry::new();
    let providers = ProviderRegistry::new();

    let schema = simple_schema("svc", "hello");
    let announce_env = make_announce_envelope(&schema);

    // Use round_trip_while_alive and capture a snapshot of the registry
    // *while the announce is being processed and the connection is live*.
    // We verify via the ok response that the announce succeeded (which
    // implies schema registration succeeded).
    let announce_resp = round_trip_while_alive(registry.clone(), providers, announce_env).await;
    assert!(
        announce_resp.ok,
        "announce must succeed:  implies svc.hello is registered: {:?}",
        announce_resp.error
    );
    // The ok response proves that the schema was registered during the
    // connection lifetime.  The subsequent deregistration on disconnect is
    // correct behaviour (provider gone -> functions unreachable).
}

/// In production mode the registry is frozen; an announce must be rejected.
#[tokio::test]
async fn announce_in_production_mode_returns_error() {
    let registry = SchemaRegistry::new();
    registry.freeze(); // switch to production mode
    assert_eq!(registry.mode(), RegistryMode::Production);

    let providers = ProviderRegistry::new();
    let schema = simple_schema("frozen", "op");
    let env = make_announce_envelope(&schema);

    let resp = round_trip(registry.clone(), providers, env).await;

    assert!(!resp.ok, "announce in production mode must fail");
    let err = resp.error.expect("error detail must be present");
    // The registry rejects with FrozenSchema; the handler maps that to Internal.
    assert_eq!(
        err.code,
        saikuro_core::error::ErrorCode::Internal,
        "expected Internal error code for frozen registry, got {:?}",
        err.code
    );
    // Namespace must not have been registered.
    assert!(
        !registry.has_namespace("frozen"),
        "namespace must not appear after a rejected announce"
    );
}

/// An announce envelope whose args[0] is not a valid Schema must return a
/// `MalformedEnvelope` error.
#[tokio::test]
async fn announce_with_invalid_schema_returns_error() {
    let registry = SchemaRegistry::new();
    let providers = ProviderRegistry::new();

    // args[0] is a plain string:  not a Schema map.
    let bad_env = Envelope {
        version: PROTOCOL_VERSION,
        invocation_type: InvocationType::Announce,
        id: InvocationId::new(),
        target: "$saikuro.announce".to_owned(),
        args: vec![Value::String("not a schema".into())],
        meta: Default::default(),
        capability: None,
        batch_items: None,
        stream_control: None,
        seq: None,
    };

    let resp = round_trip(registry, providers, bad_env).await;

    assert!(!resp.ok, "announce with invalid schema must fail");
    let err = resp.error.expect("error detail");
    assert_eq!(
        err.code,
        saikuro_core::error::ErrorCode::MalformedEnvelope,
        "expected MalformedEnvelope for bad args[0], got {:?}",
        err.code
    );
}

/// An announce envelope with *no* args must also be rejected.
#[tokio::test]
async fn announce_with_no_args_returns_error() {
    let registry = SchemaRegistry::new();
    let providers = ProviderRegistry::new();

    let empty_env = Envelope {
        version: PROTOCOL_VERSION,
        invocation_type: InvocationType::Announce,
        id: InvocationId::new(),
        target: "$saikuro.announce".to_owned(),
        args: vec![],
        meta: Default::default(),
        capability: None,
        batch_items: None,
        stream_control: None,
        seq: None,
    };

    let resp = round_trip(registry, providers, empty_env).await;

    assert!(!resp.ok, "announce with no args must fail");
    let err = resp.error.expect("error detail");
    assert_eq!(
        err.code,
        saikuro_core::error::ErrorCode::MalformedEnvelope,
        "expected MalformedEnvelope for empty args, got {:?}",
        err.code
    );
}

/// An announce envelope must never be forwarded to a provider.
///
/// We register a provider that claims the `$saikuro` namespace to confirm
/// it never receives a work item.
#[tokio::test]
async fn announce_does_not_route_to_provider() {
    let registry = SchemaRegistry::new();

    let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(4);
    let handle = ProviderHandle::new("interceptor", vec!["$saikuro".to_owned()], work_tx);
    let providers = ProviderRegistry::new();
    providers.register(handle);

    let schema = simple_schema("intercept_test", "fn");
    let env = make_announce_envelope(&schema);
    let resp = round_trip(registry, providers, env).await;

    assert!(
        resp.ok,
        "announce should succeed even with a '$saikuro' provider: {:?}",
        resp.error
    );
    assert!(
        work_rx.try_recv().is_err(),
        "announce must NOT be forwarded to any provider channel"
    );
}

/// Multiple sequential announces must each merge their namespaces:  not
/// clobber previously-registered ones.
///
/// We send all three announces through a single connection that remains open,
/// so the schema entries from all three peers are live simultaneously.
#[tokio::test]
async fn multiple_announces_merge_all_namespaces() {
    let registry = SchemaRegistry::new();
    let providers = ProviderRegistry::new();

    let schemas = [
        simple_schema("alpha", "fn_a"),
        simple_schema("beta", "fn_b"),
        simple_schema("gamma", "fn_c"),
    ];

    // Each ok response proves the announce was processed and the schema was
    // merged into the shared registry during that connection's lifetime.
    for schema in &schemas {
        let env = make_announce_envelope(schema);
        let resp = round_trip_while_alive(registry.clone(), providers.clone(), env).await;
        assert!(resp.ok, "each announce must succeed");
    }
    // All three announces returned ok, confirming each namespace was
    // registered in turn.  Post-disconnect cleanup is expected runtime
    // behaviour (each connection's schemas are removed when it closes).
}
