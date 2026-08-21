use bytes::Bytes;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::Envelope,
    schema::{
        FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema, PrimitiveType, Schema,
        TypeDescriptor, TypeMap, Visibility,
    },
    RegistrationToken, ResponseEnvelope,
};
use saikuro_event::Value;
use saikuro_exec::mpsc;
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::{InvocationRouter, RouterConfig},
};
use saikuro_runtime::connection::ConnectionHandler;
use saikuro_schema::{
    capability_engine::CapabilityEngine, registry::SchemaRegistry, validator::InvocationValidator,
};
use saikuro_transport::shared::memory::{MemoryReceiver, MemorySender};
use saikuro_transport::{MemoryTransport, Transport, TransportReceiver, TransportSender};
use std::sync::Arc;

pub fn null_log() -> Arc<dyn saikuro_event::LogSink> {
    Arc::from(Box::new(saikuro_event::NullSink) as Box<dyn saikuro_event::LogSink>)
}

pub async fn make_provider(
    namespace: &str,
) -> (ProviderRegistry, mpsc::Receiver<ProviderWorkItem>) {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(
        saikuro_exec::ChannelCapacity::try_from(64).expect("64 is a valid channel capacity"),
    );
    let handle = ProviderHandle::new(
        format!("{namespace}-provider"),
        vec![namespace.to_owned()],
        work_tx,
    );
    let registry = ProviderRegistry::new();
    registry.register(handle).await;
    (registry, work_rx)
}

pub fn simple_schema(namespace: &str, function: &str) -> Schema {
    let mut functions = FunctionMap::new();
    functions
        .insert(
            function.to_owned(),
            FunctionSchema {
                args: vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::Unit),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        )
        .ok();
    let mut namespaces = NamespaceMap::new();
    namespaces
        .insert(
            namespace.to_owned(),
            NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        )
        .ok();
    Schema {
        version: 1,
        namespaces: Box::new(namespaces),
        types: Box::new(TypeMap::new()),
    }
}

pub fn schema_to_value(schema: &Schema) -> Value {
    let bytes = rmp_serde::to_vec_named(schema).expect("serialize schema");
    rmp_serde::from_slice::<Value>(&bytes).expect("deserialize schema to Value")
}

pub fn make_announce_envelope(schema: &Schema) -> Envelope {
    Envelope::announce(schema_to_value(schema)).expect("entropy available")
}

pub async fn register_namespace(registry: &SchemaRegistry, namespace: &str, function: &str) {
    registry
        .merge_schema(simple_schema(namespace, function), "test-provider")
        .await
        .expect("merge_schema must succeed");
}

/// Build a `ConnectionHandler` over the runtime side of a
/// `MemoryTransport::pair` with the standard test configuration: default
/// router config, non-sandbox capability engine, and empty peer capabilities.
///
/// Tests needing sandbox mode or specific peer capabilities mutate the
/// returned handler's public fields (or call `ConnectionHandler::sandboxed`).
pub fn make_handler(
    peer_id: &str,
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    log: Arc<dyn saikuro_event::LogSink>,
    handler_transport: MemoryTransport,
) -> ConnectionHandler<MemorySender, MemoryReceiver> {
    let (handler_sender, handler_receiver) = handler_transport.split();
    ConnectionHandler {
        peer_id: peer_id.to_owned(),
        registration_token: RegistrationToken::new(),
        sender: handler_sender,
        receiver: handler_receiver,
        validator: InvocationValidator::new(schema_registry.clone()),
        capability_engine: CapabilityEngine::default(),
        router: InvocationRouter::new(provider_registry.clone(), RouterConfig::default()),
        peer_capabilities: CapabilitySet::empty(),
        max_message_size: 4 * 1024 * 1024,
        schema_registry,
        provider_registry,
        log,
    }
}

pub async fn round_trip_via_handler(
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    envelope: Envelope,
) -> ResponseEnvelope {
    let log = null_log();
    let (test_transport, handler_transport) = MemoryTransport::pair("test", "handler", log.clone());
    let (mut test_sender, mut test_receiver) = test_transport.split();

    let handler = make_handler(
        "test-peer",
        schema_registry,
        provider_registry,
        log,
        handler_transport,
    );

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
