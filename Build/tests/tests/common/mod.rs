use bytes::Bytes;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::Envelope,
    schema::{FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility},
    value::Value,
    ResponseEnvelope,
};
use saikuro_exec::mpsc;
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
use std::collections::HashMap;

pub fn make_provider(namespace: &str) -> (ProviderRegistry, mpsc::Receiver<ProviderWorkItem>) {
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

pub fn simple_schema(namespace: &str, function: &str) -> Schema {
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

pub fn schema_to_value(schema: &Schema) -> Value {
    let bytes = rmp_serde::to_vec_named(schema).expect("serialize schema");
    rmp_serde::from_slice::<Value>(&bytes).expect("deserialize schema to Value")
}

pub fn make_announce_envelope(schema: &Schema) -> Envelope {
    Envelope::announce(schema_to_value(schema))
}

pub fn register_namespace(registry: &SchemaRegistry, namespace: &str, function: &str) {
    registry
        .merge_schema(simple_schema(namespace, function), "test-provider")
        .expect("merge_schema must succeed");
}

pub async fn round_trip_via_handler(
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
