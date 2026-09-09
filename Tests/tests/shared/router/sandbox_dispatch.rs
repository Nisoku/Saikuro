//! Sandbox-mode `ConnectionHandler` integration tests.

use crate::check_test;
use crate::common;
use crate::shared_test;
use crate::TestSuite;
use bytes::Bytes;
use saikuro_core::{
    capability::{CapabilitySet, CapabilityToken},
    envelope::{Envelope, InvocationType},
    schema::{
        FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema, PrimitiveType, Schema,
        TypeDescriptor, TypeMap, Visibility,
    },
    ResponseEnvelope, PROTOCOL_VERSION,
};
use saikuro_router::provider::ProviderRegistry;
use saikuro_schema::registry::SchemaRegistry;
use saikuro_transport::{MemoryTransport, Transport, TransportReceiver, TransportSender};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "router::sandbox_announce_pushes_filtered_schema_frame",
        sandbox_announce_pushes_filtered_schema_frame,
    );
    shared_test!(
        suite,
        "router::sandbox_filtered_schema_excludes_internal_functions",
        sandbox_filtered_schema_excludes_internal_functions,
    );
    shared_test!(
        suite,
        "router::sandbox_filtered_schema_excludes_private_functions",
        sandbox_filtered_schema_excludes_private_functions,
    );
    shared_test!(
        suite,
        "router::sandbox_filtered_schema_includes_public_no_cap_functions",
        sandbox_filtered_schema_includes_public_no_cap_functions,
    );
    shared_test!(
        suite,
        "router::sandbox_filtered_schema_excludes_functions_peer_lacks_caps_for",
        sandbox_filtered_schema_excludes_functions_peer_lacks_caps_for,
    );
    shared_test!(
        suite,
        "router::sandbox_filtered_schema_includes_functions_peer_has_caps_for",
        sandbox_filtered_schema_includes_functions_peer_has_caps_for,
    );
    shared_test!(
        suite,
        "router::non_sandbox_announce_produces_single_response_frame",
        non_sandbox_announce_produces_single_response_frame,
    );
    shared_test!(
        suite,
        "router::sandbox_handler_denies_internal_function_invocation",
        sandbox_handler_denies_internal_function_invocation,
    );
}

fn build_schema() -> Schema {
    let mut functions = FunctionMap::new();
    functions.insert(
        "public_fn".into(),
        FunctionSchema {
            args: crate::vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Public,
            capabilities: crate::vec![],
            idempotent: false,
            doc: None,
        },
    );
    functions.insert(
        "internal_fn".into(),
        FunctionSchema {
            args: crate::vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Internal,
            capabilities: crate::vec![],
            idempotent: false,
            doc: None,
        },
    );
    functions.insert(
        "private_fn".into(),
        FunctionSchema {
            args: crate::vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Private,
            capabilities: crate::vec![],
            idempotent: false,
            doc: None,
        },
    );
    functions.insert(
        "guarded_fn".into(),
        FunctionSchema {
            args: crate::vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::Unit),
            visibility: Visibility::Public,
            capabilities: crate::vec![CapabilityToken::new("special.cap")],
            idempotent: false,
            doc: None,
        },
    );
    let mut namespaces = NamespaceMap::new();
    namespaces.insert(
        "svc".into(),
        NamespaceSchema {
            functions: crate::Box::new(functions),
            doc: None,
        },
    );
    Schema {
        version: 1,
        namespaces: crate::Box::new(namespaces),
        types: crate::Box::new(TypeMap::new()),
    }
}

/// Send `envelope` through a `ConnectionHandler` (optionally sandboxed) and
/// collect all frames the handler pushes back.
///
/// The test side sends the frame then drops its sender to signal EOF. After
/// the handler finishes its loop all buffered response frames are returned.
async fn run_and_collect(
    schema_registry: SchemaRegistry,
    peer_capabilities: CapabilitySet,
    sandbox: bool,
    envelope: Envelope,
) -> crate::Vec<Bytes> {
    let log = common::null_log();
    let (test_transport, handler_transport) = MemoryTransport::pair("test", "handler", log.clone());
    let (mut test_sender, mut test_receiver) = test_transport.split();

    let providers = ProviderRegistry::new();
    let mut handler = common::make_handler(
        "sandbox-peer",
        schema_registry,
        providers,
        log,
        handler_transport,
    );
    if sandbox {
        handler = handler.sandboxed();
    }
    handler.peer_capabilities = peer_capabilities;

    let frame = Bytes::from(envelope.to_msgpack().expect("encode envelope"));
    test_sender.send(frame).await.expect("send frame");
    drop(test_sender);

    handler.run().await;

    let mut frames = crate::Vec::new();
    while let Ok(Some(f)) = test_receiver.recv().await {
        frames.push(f);
    }
    frames
}

fn sandbox_announce_pushes_filtered_schema_frame() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = common::make_announce_envelope(&schema);

        let frames = run_and_collect(registry, CapabilitySet::empty(), true, env).await;

        // Frame 0: ok response to the peer's Announce.
        // Frame 1: unsolicited Announce with the filtered schema.
        assert_eq!(frames.len(), 2, "sandbox mode must produce 2 frames");

        let resp = ResponseEnvelope::from_msgpack(&frames[0]).map_err(|_| "decode ok response")?;
        check_test!(resp.ok, "announce response must be ok");

        // Second frame is an Announce envelope.
        let push: Envelope =
            saikuro_core::from_slice(&frames[1]).map_err(|_| "decode pushed announce")?;
        assert_eq!(
            push.invocation_type,
            InvocationType::Announce,
            "second frame must be an Announce"
        );
        Ok(())
    })
}

fn sandbox_filtered_schema_excludes_internal_functions() -> Result<(), &'static str> {
    sandbox_filtered_schema_asserts("svc", |svc| {
        check_test!(
            !svc.functions.contains_key("internal_fn"),
            "Internal functions must be excluded from sandbox schema"
        );
        Ok(())
    })
}

fn sandbox_filtered_schema_excludes_private_functions() -> Result<(), &'static str> {
    sandbox_filtered_schema_asserts("svc", |svc| {
        check_test!(
            !svc.functions.contains_key("private_fn"),
            "Private functions must be excluded from sandbox schema"
        );
        Ok(())
    })
}

fn sandbox_filtered_schema_includes_public_no_cap_functions() -> Result<(), &'static str> {
    sandbox_filtered_schema_asserts("svc", |svc| {
        check_test!(
            svc.functions.contains_key("public_fn"),
            "public_fn (no caps required) must be included"
        );
        Ok(())
    })
}

fn sandbox_filtered_schema_excludes_functions_peer_lacks_caps_for() -> Result<(), &'static str> {
    sandbox_filtered_schema_asserts("svc", |svc| {
        check_test!(
            !svc.functions.contains_key("guarded_fn"),
            "guarded_fn requires 'special.cap': peer with no caps must not see it"
        );
        Ok(())
    })
}

fn sandbox_filtered_schema_includes_functions_peer_has_caps_for() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = common::make_announce_envelope(&schema);

        let caps = CapabilitySet::from_tokens([CapabilityToken::new("special.cap")])
            .map_err(|_| "build caps")?;
        let frames = run_and_collect(registry, caps, true, env).await;
        assert_eq!(frames.len(), 2);

        let push: Envelope =
            saikuro_core::from_slice(&frames[1]).map_err(|_| "decode pushed announce")?;
        let schema_value = push.args.into_iter().next().ok_or("args[0]")?;
        let schema_bytes = saikuro_core::to_vec(&schema_value).map_err(|_| "re-encode")?;
        let filtered: Schema =
            saikuro_core::from_slice(&schema_bytes).map_err(|_| "decode filtered schema")?;

        let svc = filtered.namespaces.get("svc").ok_or("svc namespace")?;
        check_test!(
            svc.functions.contains_key("guarded_fn"),
            "guarded_fn must be visible to a peer holding 'special.cap'"
        );
        Ok(())
    })
}

/// Shared body for the sandbox-filtered-schema tests: send an announce for
/// `build_schema` through an empty-capability sandboxed handler and assert on
/// the resulting filtered schema.
fn sandbox_filtered_schema_asserts(
    namespace: &'static str,
    assert: impl FnOnce(&saikuro_core::schema::NamespaceSchema) -> Result<(), &'static str> + 'static,
) -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = common::make_announce_envelope(&schema);

        let frames = run_and_collect(registry, CapabilitySet::empty(), true, env).await;
        assert_eq!(frames.len(), 2);

        let push: Envelope =
            saikuro_core::from_slice(&frames[1]).map_err(|_| "decode pushed announce")?;
        let schema_value = push.args.into_iter().next().ok_or("args[0]")?;
        let schema_bytes = saikuro_core::to_vec(&schema_value).map_err(|_| "re-encode")?;
        let filtered: Schema =
            saikuro_core::from_slice(&schema_bytes).map_err(|_| "decode filtered schema")?;

        let svc = filtered
            .namespaces
            .get(namespace)
            .ok_or("namespace present")?;
        assert(svc)
    })
}

fn non_sandbox_announce_produces_single_response_frame() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();
        let env = common::make_announce_envelope(&schema);

        let frames = run_and_collect(registry, CapabilitySet::empty(), false, env).await;

        assert_eq!(
            frames.len(),
            1,
            "non-sandbox announce must produce exactly 1 frame (the ok response)"
        );
        let resp = ResponseEnvelope::from_msgpack(&frames[0]).map_err(|_| "decode response")?;
        check_test!(resp.ok, "non-sandbox announce must be ok");
        Ok(())
    })
}

fn sandbox_handler_denies_internal_function_invocation() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let schema = build_schema();

        // Pre-register the schema so the validator can find it.
        registry
            .merge_schema(schema.clone(), "test-provider")
            .await
            .map_err(|_| "merge schema")?;

        // Build the Invoke envelope for the internal function.
        let invoke_env = Envelope {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Call,
            id: saikuro_core::InvocationId::new().map_err(|_| "entropy")?,
            target: "svc.internal_fn".into(),
            args: crate::vec![],
            meta: Default::default(),
            capability: None,
            batch_items: None,
            stream_control: None,
            seq: None,
        };

        let frames = run_and_collect(registry, CapabilitySet::empty(), true, invoke_env).await;

        assert_eq!(frames.len(), 1);
        let resp = ResponseEnvelope::from_msgpack(&frames[0]).map_err(|_| "decode response")?;
        check_test!(
            !resp.ok,
            "internal function invocation must be denied in sandbox mode"
        );
        let err = resp.error.as_ref().ok_or("error detail must be present")?;
        assert_eq!(
            err.code,
            saikuro_event::ErrorCode::CapabilityDenied,
            "expected CapabilityDenied"
        );
        Ok(())
    })
}
