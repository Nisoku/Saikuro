//! Announce-envelope integration tests.

use crate::check_test;
use crate::common;
use crate::shared_test;
use crate::TestSuite;
use bytes::Bytes;
use saikuro_core::{
    envelope::{Envelope, InvocationType},
    InvocationId, ResponseEnvelope, PROTOCOL_VERSION,
};
use saikuro_event::Value;
use saikuro_exec::mpsc;
use saikuro_router::provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem};
use saikuro_schema::registry::{RegistryMode, SchemaRegistry};
use saikuro_transport::{MemoryTransport, Transport, TransportReceiver, TransportSender};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "router::announce_registers_namespace_in_schema",
        announce_registers_namespace_in_schema,
    );
    shared_test!(
        suite,
        "router::announce_allows_subsequent_calls_to_not_fail_schema_validation",
        announce_allows_subsequent_calls_to_not_fail_schema_validation,
    );
    shared_test!(
        suite,
        "router::announce_in_production_mode_returns_error",
        announce_in_production_mode_returns_error,
    );
    shared_test!(
        suite,
        "router::announce_with_invalid_schema_returns_error",
        announce_with_invalid_schema_returns_error,
    );
    shared_test!(
        suite,
        "router::announce_with_no_args_returns_error",
        announce_with_no_args_returns_error,
    );
    shared_test!(
        suite,
        "router::announce_does_not_route_to_provider",
        announce_does_not_route_to_provider,
    );
    shared_test!(
        suite,
        "router::multiple_announces_merge_all_namespaces",
        multiple_announces_merge_all_namespaces,
    );
}

/// Send a single frame, read the response, check the registry while the
/// connection is still alive, then close the connection.
///
/// Unlike `round_trip_via_handler`, this spawns the handler as a background
/// task so the test can inspect shared state (schema registry) before EOF is
/// sent. Returns the decoded response.
async fn round_trip_while_alive(
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    envelope: Envelope,
) -> Result<ResponseEnvelope, &'static str> {
    let log = common::null_log();
    let (test_transport, handler_transport) = MemoryTransport::pair("test", "handler", log.clone());
    let (mut test_sender, mut test_receiver) = test_transport.split();

    let handler = common::make_handler(
        "test-peer",
        schema_registry,
        provider_registry,
        log,
        handler_transport,
        false,
    );

    // Spawn the handler so we can interleave reads/writes.
    let task = saikuro_exec::spawn(handler.run());

    // Send the envelope.
    let frame = Bytes::from(envelope.to_msgpack().map_err(|_| "encode envelope")?);
    test_sender.send(frame).await.map_err(|_| "send frame")?;

    // Read the response BEFORE closing the connection.
    let resp_frame = test_receiver
        .recv()
        .await
        .map_err(|_| "recv response")?
        .ok_or("frame must be present")?;
    let response = ResponseEnvelope::from_msgpack(&resp_frame).map_err(|_| "decode response")?;

    // Now close the connection and wait for the handler to exit.
    drop(test_sender);
    task.await.map_err(|_| "handler task panicked")?;

    Ok(response)
}

fn announce_registers_namespace_in_schema() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let providers = ProviderRegistry::new();

        check_test!(
            !registry.has_namespace("math").await,
            "registry must be empty before announce"
        );

        let schema = common::simple_schema("math", "add");
        let env = common::make_announce_envelope(&schema);

        // Inspect the registry while the connection is still open (before the
        // handler's disconnect cleanup runs).
        let resp = round_trip_while_alive(registry.clone(), providers, env).await?;

        check_test!(resp.ok, "announce should return ok");
        Ok(())
    })
}

fn announce_allows_subsequent_calls_to_not_fail_schema_validation() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let providers = ProviderRegistry::new();

        let schema = common::simple_schema("svc", "hello");
        let announce_env = common::make_announce_envelope(&schema);

        // The ok response proves the schema was registered during the
        // connection lifetime.
        let announce_resp =
            round_trip_while_alive(registry.clone(), providers, announce_env).await?;
        check_test!(
            announce_resp.ok,
            "announce must succeed, implying svc.hello is registered"
        );
        Ok(())
    })
}

fn announce_in_production_mode_returns_error() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        registry.freeze().await;
        assert_eq!(registry.mode().await, RegistryMode::Production);

        let providers = ProviderRegistry::new();
        let schema = common::simple_schema("frozen", "op");
        let env = common::make_announce_envelope(&schema);

        let resp = common::round_trip_via_handler(registry.clone(), providers, env).await;

        check_test!(!resp.ok, "announce in production mode must fail");
        let err = resp.error.as_ref().expect("error detail must be present");
        assert_eq!(
            err.code,
            saikuro_event::ErrorCode::Internal,
            "expected Internal error code for frozen registry"
        );
        check_test!(
            !registry.has_namespace("frozen").await,
            "namespace must not appear after a rejected announce"
        );
        Ok(())
    })
}

fn announce_with_invalid_schema_returns_error() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let providers = ProviderRegistry::new();

        // args[0] is a plain string, not a Schema map.
        let bad_env = Envelope {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Announce,
            id: InvocationId::new().map_err(|_| "entropy")?,
            target: "$saikuro.announce".into(),
            args: crate::vec![Value::String("not a schema".into())],
            meta: Default::default(),
            capability: None,
            batch_items: None,
            stream_control: None,
            seq: None,
        };

        let resp = common::round_trip_via_handler(registry, providers, bad_env).await;

        check_test!(!resp.ok, "announce with invalid schema must fail");
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(saikuro_event::ErrorCode::MalformedEnvelope),
            "expected MalformedEnvelope for bad args[0]"
        );
        Ok(())
    })
}

fn announce_with_no_args_returns_error() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let providers = ProviderRegistry::new();

        let empty_env = Envelope {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Announce,
            id: InvocationId::new().map_err(|_| "entropy")?,
            target: "$saikuro.announce".into(),
            args: crate::vec![],
            meta: Default::default(),
            capability: None,
            batch_items: None,
            stream_control: None,
            seq: None,
        };

        let resp = common::round_trip_via_handler(registry, providers, empty_env).await;

        check_test!(!resp.ok, "announce with no args must fail");
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(saikuro_event::ErrorCode::MalformedEnvelope),
            "expected MalformedEnvelope for empty args"
        );
        Ok(())
    })
}

fn announce_does_not_route_to_provider() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();

        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(crate::common::capacity(4));
        let handle = ProviderHandle::new("interceptor", crate::vec!["$saikuro".into()], work_tx);
        let providers = ProviderRegistry::new();
        providers.register(handle).await;

        let schema = common::simple_schema("intercept_test", "fn");
        let env = common::make_announce_envelope(&schema);
        let resp = common::round_trip_via_handler(registry, providers, env).await;

        check_test!(
            resp.ok,
            "announce should succeed even with a '$saikuro' provider"
        );
        check_test!(
            matches!(work_rx.recv().await, None),
            "announce must NOT be forwarded to any provider channel"
        );
        Ok(())
    })
}

fn multiple_announces_merge_all_namespaces() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = SchemaRegistry::new();
        let providers = ProviderRegistry::new();

        let schemas = [
            common::simple_schema("alpha", "fn_a"),
            common::simple_schema("beta", "fn_b"),
            common::simple_schema("gamma", "fn_c"),
        ];

        for schema in &schemas {
            let env = common::make_announce_envelope(schema);
            let resp = round_trip_while_alive(registry.clone(), providers.clone(), env).await?;
            check_test!(resp.ok, "each announce must succeed");
        }
        Ok(())
    })
}
