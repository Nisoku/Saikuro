//! Sandbox-mode `ConnectionHandler` integration tests

use bytes::Bytes;
use saikuro_core::{
    capability::{CapabilitySet, CapabilityToken},
    envelope::{Envelope, InvocationType},
    schema::{FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility},
    value::Value,
    InvocationId, ResponseEnvelope, PROTOCOL_VERSION,
};
use saikuro_router::{
    provider::ProviderRegistry,
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

//  Helpers

fn build_schema() -> Schema {
    let mut functions = HashMap::new();
    functions.insert(
        "public_fn".to_owned(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );
    functions.insert(
        "internal_fn".to_owned(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Internal,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );
    functions.insert(
        "private_fn".to_owned(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Private,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );
    functions.insert(
        "guarded_fn".to_owned(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Public,
            capabilities: vec![CapabilityToken::new("special.cap")],
            idempotent: false,
            doc: None,
        },
    );
    let mut namespaces = HashMap::new();
    namespaces.insert(
        "svc".to_owned(),
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

fn schema_to_value(schema: &Schema) -> Value {
    let bytes = rmp_serde::to_vec_named(schema).expect("serialize schema");
    rmp_serde::from_slice::<Value>(&bytes).expect("deserialize schema to Value")
}

fn make_announce(schema: &Schema) -> Envelope {
    Envelope::announce(schema_to_value(schema))
}

/// Send `envelope` through a `ConnectionHandler` (optionally sandboxed) and
/// collect all frames the handler pushes back.
///
/// The test side sends the frame then drops its sender to signal EOF.  After
/// the handler finishes its loop all buffered response frames are returned.
async fn run_and_collect(
    schema_registry: SchemaRegistry,
    peer_capabilities: CapabilitySet,
    sandbox: bool,
    envelope: Envelope,
) -> Vec<Bytes> {
    let (test_transport, handler_transport) = MemoryTransport::pair("test", "handler");
    let (handler_sender, handler_receiver) = handler_transport.split();
    let (mut test_sender, mut test_receiver) = test_transport.split();

    let providers = ProviderRegistry::new();
    let router = InvocationRouter::new(providers.clone(), RouterConfig::default());
    let validator = InvocationValidator::new(schema_registry.clone());
    let capability_engine = if sandbox {
        CapabilityEngine::sandboxed()
    } else {
        CapabilityEngine::new()
    };

    let handler = ConnectionHandler {
        peer_id: "sandbox-peer".to_owned(),
        sender: handler_sender,
        receiver: handler_receiver,
        validator,
        capability_engine,
        router,
        peer_capabilities,
        max_message_size: 4 * 1024 * 1024,
        schema_registry,
        provider_registry: providers,
    };

    let frame = Bytes::from(envelope.to_msgpack().expect("encode envelope"));
    test_sender.send(frame).await.expect("send frame");
    drop(test_sender);

    handler.run().await;

    let mut frames = Vec::new();
    while let Ok(Some(f)) = test_receiver.recv().await {
        frames.push(f);
    }
    frames
}

//  Tests

/// In sandbox mode, announcing a schema causes the handler to push back a
/// second frame: an Announce envelope with the capability-filtered schema.
#[test]
fn sandbox_announce_pushes_filtered_schema_frame() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = make_announce(&schema);

        let frames = run_and_collect(registry, CapabilitySet::empty(), true, env).await;

        // Frame 0: ok response to the peer's Announce.
        // Frame 1: unsolicited Announce with the filtered schema.
        assert_eq!(frames.len(), 2, "sandbox mode must produce 2 frames");

        let resp = ResponseEnvelope::from_msgpack(&frames[0]).expect("decode ok response");
        assert!(resp.ok, "announce response must be ok: {:?}", resp.error);

        // Second frame is an Announce envelope.
        let push: Envelope = rmp_serde::from_slice(&frames[1]).expect("decode pushed announce");
        assert_eq!(
            push.invocation_type,
            InvocationType::Announce,
            "second frame must be an Announce"
        );
    })
}

/// The pushed schema excludes Internal-visibility functions.
#[test]
fn sandbox_filtered_schema_excludes_internal_functions() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = make_announce(&schema);

        let frames = run_and_collect(registry, CapabilitySet::empty(), true, env).await;
        assert_eq!(frames.len(), 2);

        let push: Envelope = rmp_serde::from_slice(&frames[1]).expect("decode pushed announce");
        let schema_value = push.args.into_iter().next().expect("args[0] must exist");
        let schema_bytes = rmp_serde::to_vec_named(&schema_value).expect("re-encode");
        let filtered: Schema =
            rmp_serde::from_slice(&schema_bytes).expect("decode filtered schema");

        let svc = filtered
            .namespaces
            .get("svc")
            .expect("svc namespace must be present");
        assert!(
            !svc.functions.contains_key("internal_fn"),
            "Internal functions must be excluded from sandbox schema"
        );
    })
}

/// The pushed schema excludes Private-visibility functions.
#[test]
fn sandbox_filtered_schema_excludes_private_functions() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = make_announce(&schema);

        let frames = run_and_collect(registry, CapabilitySet::empty(), true, env).await;
        assert_eq!(frames.len(), 2);

        let push: Envelope = rmp_serde::from_slice(&frames[1]).expect("decode pushed announce");
        let schema_value = push.args.into_iter().next().expect("args[0]");
        let schema_bytes = rmp_serde::to_vec_named(&schema_value).expect("re-encode");
        let filtered: Schema =
            rmp_serde::from_slice(&schema_bytes).expect("decode filtered schema");

        let svc = filtered.namespaces.get("svc").expect("svc namespace");
        assert!(
            !svc.functions.contains_key("private_fn"),
            "Private functions must be excluded from sandbox schema"
        );
    })
}

/// Public functions with no required caps are present in the filtered schema.
#[test]
fn sandbox_filtered_schema_includes_public_no_cap_functions() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = make_announce(&schema);

        let frames = run_and_collect(registry, CapabilitySet::empty(), true, env).await;
        assert_eq!(frames.len(), 2);

        let push: Envelope = rmp_serde::from_slice(&frames[1]).expect("decode pushed announce");
        let schema_value = push.args.into_iter().next().expect("args[0]");
        let schema_bytes = rmp_serde::to_vec_named(&schema_value).expect("re-encode");
        let filtered: Schema =
            rmp_serde::from_slice(&schema_bytes).expect("decode filtered schema");

        let svc = filtered.namespaces.get("svc").expect("svc namespace");
        assert!(
            svc.functions.contains_key("public_fn"),
            "public_fn (no caps required) must be included"
        );
    })
}

/// Functions whose required capabilities the peer doesn't hold are excluded.
#[test]
fn sandbox_filtered_schema_excludes_functions_peer_lacks_caps_for() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = make_announce(&schema);

        // Peer has no capabilities.
        let frames = run_and_collect(registry, CapabilitySet::empty(), true, env).await;
        assert_eq!(frames.len(), 2);

        let push: Envelope = rmp_serde::from_slice(&frames[1]).expect("decode pushed announce");
        let schema_value = push.args.into_iter().next().expect("args[0]");
        let schema_bytes = rmp_serde::to_vec_named(&schema_value).expect("re-encode");
        let filtered: Schema =
            rmp_serde::from_slice(&schema_bytes).expect("decode filtered schema");

        let svc = filtered.namespaces.get("svc").expect("svc namespace");
        assert!(
            !svc.functions.contains_key("guarded_fn"),
            "guarded_fn requires 'special.cap':  peer with no caps must not see it"
        );
    })
}

/// If the peer holds the required capability, the guarded function IS included.
#[test]
fn sandbox_filtered_schema_includes_functions_peer_has_caps_for() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = make_announce(&schema);

        let caps = CapabilitySet::from_tokens([CapabilityToken::new("special.cap")]);
        let frames = run_and_collect(registry, caps, true, env).await;
        assert_eq!(frames.len(), 2);

        let push: Envelope = rmp_serde::from_slice(&frames[1]).expect("decode pushed announce");
        let schema_value = push.args.into_iter().next().expect("args[0]");
        let schema_bytes = rmp_serde::to_vec_named(&schema_value).expect("re-encode");
        let filtered: Schema =
            rmp_serde::from_slice(&schema_bytes).expect("decode filtered schema");

        let svc = filtered.namespaces.get("svc").expect("svc namespace");
        assert!(
            svc.functions.contains_key("guarded_fn"),
            "guarded_fn must be visible to a peer holding 'special.cap'"
        );
    })
}

/// In non-sandbox mode, no extra frame is sent after an Announce.
#[test]
fn non_sandbox_announce_produces_single_response_frame() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = make_announce(&schema);

        let frames = run_and_collect(registry, CapabilitySet::empty(), false, env).await;

        assert_eq!(
            frames.len(),
            1,
            "non-sandbox announce must produce exactly 1 frame (the ok response)"
        );
        let resp = ResponseEnvelope::from_msgpack(&frames[0]).expect("decode response");
        assert!(resp.ok);
    })
}

/// Calling an Internal function through a sandboxed handler returns CapabilityDenied.
#[test]
fn sandbox_handler_denies_internal_function_invocation() {
    saikuro_exec::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();

        // Pre-register the schema so the validator can find it.
        registry
            .merge_schema(schema.clone(), "test-provider")
            .expect("merge schema");

        // Build the Invoke envelope for the internal function.
        let invoke_env = Envelope {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Call,
            id: InvocationId::new(),
            target: "svc.internal_fn".to_owned(),
            args: vec![],
            meta: Default::default(),
            capability: None,
            batch_items: None,
            stream_control: None,
            seq: None,
        };

        let frames = run_and_collect(registry, CapabilitySet::empty(), true, invoke_env).await;

        assert_eq!(frames.len(), 1);
        let resp = ResponseEnvelope::from_msgpack(&frames[0]).expect("decode response");
        assert!(
            !resp.ok,
            "internal function invocation must be denied in sandbox mode"
        );
        let err = resp.error.expect("error detail must be present");
        assert_eq!(
            err.code,
            saikuro_core::error::ErrorCode::CapabilityDenied,
            "expected CapabilityDenied, got {:?}",
            err.code
        );
    })
}
