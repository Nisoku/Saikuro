use bytes::Bytes;
use saikuro_core::Arc;
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

use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use core::future::Future;
use core::pin::Pin;

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
        vec![namespace.to_string()],
        work_tx,
    );
    let registry = ProviderRegistry::new();
    registry.register(handle).await;
    (registry, work_rx)
}

pub fn simple_schema(namespace: &str, function: &str) -> Schema {
    let mut functions = FunctionMap::new();
    let _ = functions.insert(
        function.to_string(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );
    let mut namespaces = NamespaceMap::new();
    let _ = namespaces.insert(
        namespace.to_string(),
        NamespaceSchema {
            functions: Box::new(functions),
            doc: None,
        },
    );
    Schema {
        version: 1,
        namespaces: Box::new(namespaces),
        types: Box::new(TypeMap::new()),
    }
}

pub fn schema_to_value(schema: &Schema) -> Value {
    let bytes = saikuro_core::to_vec(schema).expect("serialize schema");
    saikuro_core::from_slice::<Value>(&bytes).expect("deserialize schema to Value")
}

pub fn make_announce_envelope(schema: &Schema) -> Envelope {
    Envelope::announce(schema_to_value(schema)).expect("entropy available")
}

pub fn spawn_responder(
    mut work_rx: mpsc::Receiver<ProviderWorkItem>,
    value: Value,
) -> saikuro_exec::JoinHandle<()> {
    saikuro_exec::spawn(async move {
        while let Some(item) = work_rx.recv().await {
            if let Some(tx) = item.response_tx {
                let _ = tx.send(ResponseEnvelope::ok(item.envelope.id, value.clone()));
            }
        }
    })
}

pub async fn register_namespace(registry: &SchemaRegistry, namespace: &str, function: &str) {
    registry
        .merge_schema(simple_schema(namespace, function), "test-provider")
        .await
        .expect("merge_schema must succeed");
}

/// Build a `ConnectionHandler` over the runtime side of a
/// `MemoryTransport::pair` with the standard test configuration: default
/// router config, optional sandbox capability engine, and empty peer
/// capabilities.
pub fn make_handler(
    peer_id: &str,
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    log: Arc<dyn saikuro_event::LogSink>,
    handler_transport: MemoryTransport,
    sandbox: bool,
) -> crate::Box<ConnectionHandler<MemorySender, MemoryReceiver>> {
    let (handler_sender, handler_receiver) = handler_transport.split();
    crate::Box::new(ConnectionHandler {
        peer_id: peer_id.to_string(),
        registration_token: RegistrationToken::new(),
        sender: handler_sender,
        receiver: handler_receiver,
        validator: InvocationValidator::new(schema_registry.clone()),
        capability_engine: if sandbox {
            CapabilityEngine::sandboxed()
        } else {
            CapabilityEngine::default()
        },
        router: InvocationRouter::new(provider_registry.clone(), RouterConfig::default()),
        peer_capabilities: saikuro_runtime::connection::empty_peer_capabilities(),
        max_message_size: core::cmp::min(4 * 1024 * 1024, crate::capacity::TEST_CAPACITY),
        schema_registry,
        provider_registry,
        log,
    })
}

/// Send `envelope` through a fresh handler and return the response.
///
/// Returns a boxed future so the handler's (large) state machine lives on the
/// heap
pub fn round_trip_via_handler(
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    envelope: Envelope,
) -> Pin<Box<dyn Future<Output = ResponseEnvelope> + 'static>> {
    Box::pin(async move {
        let log = null_log();
        let (test_transport, handler_transport) =
            MemoryTransport::pair("test", "handler", log.clone());
        let (mut test_sender, mut test_receiver) = test_transport.split();

        let handler = make_handler(
            "test-peer",
            schema_registry,
            provider_registry,
            log,
            handler_transport,
            false,
        );

        let frame = Bytes::from(envelope.to_msgpack().expect("encode envelope"));
        test_sender.send(frame).await.expect("send frame");
        drop(test_sender);

        // Box the handler run-loop future so its ~2.4K state machine lives on
        // the heap (as a spawned task would)
        Box::pin(handler.run()).await;

        let resp_frame = test_receiver
            .recv()
            .await
            .expect("recv response")
            .expect("frame must be present");
        ResponseEnvelope::from_msgpack(&resp_frame).expect("decode response")
    })
}

pub fn capacity(value: usize) -> saikuro_exec::ChannelCapacity {
    saikuro_exec::ChannelCapacity::try_from(value).expect("test channel capacity must be valid")
}

pub fn encode_envelope(env: &Envelope) -> Bytes {
    Bytes::from(env.to_msgpack().expect("encode envelope"))
}
