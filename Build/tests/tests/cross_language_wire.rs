//! Cross-language wire-protocol integration tests

use bytes::Bytes;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::{Envelope, InvocationType},
    error::ErrorCode,
    schema::{FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility},
    value::Value,
    InvocationId, ResponseEnvelope, PROTOCOL_VERSION,
};
use saikuro_runtime::runtime::SaikuroRuntime;
use saikuro_transport::{
    memory::MemoryTransport,
    traits::{Transport, TransportReceiver, TransportSender},
};
use std::collections::HashMap;

//  Shared helpers

/// Encode an [`Envelope`] to raw MessagePack bytes (as any adapter would).
fn encode_envelope(env: &Envelope) -> Bytes {
    Bytes::from(env.to_msgpack().expect("encode envelope"))
}

/// Decode a raw MessagePack frame into a [`ResponseEnvelope`].
fn decode_response(frame: Bytes) -> ResponseEnvelope {
    ResponseEnvelope::from_msgpack(&frame).expect("decode response")
}

/// Decode a raw MessagePack frame into an outbound [`Envelope`]
/// (used to receive frames pushed by the runtime, e.g. provider calls).
fn decode_envelope(frame: Bytes) -> Envelope {
    rmp_serde::from_slice(&frame).expect("decode envelope")
}

/// Build a minimal one-function schema with N variadic `Any`-typed args.
///
/// Using `Any`-typed args passes the validator regardless of what values the
/// caller sends, which keeps the helper broadly reusable across tests that
/// focus on routing / wire fidelity rather than argument validation.
fn make_schema_with_args(namespace: &str, function: &str, n_args: usize) -> Schema {
    use saikuro_core::schema::ArgumentDescriptor;
    let args = (0..n_args)
        .map(|i| ArgumentDescriptor {
            name: format!("arg{i}"),
            r#type: TypeDescriptor::primitive(PrimitiveType::Any),
            optional: false,
            default: None,
            doc: None,
        })
        .collect();
    let mut functions = HashMap::new();
    functions.insert(
        function.to_owned(),
        FunctionSchema {
            args,
            returns: TypeDescriptor::primitive(PrimitiveType::Any),
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

/// Build a minimal one-function schema with no required arguments.
fn make_schema(namespace: &str, function: &str) -> Schema {
    make_schema_with_args(namespace, function, 0)
}

/// Serialise a [`Schema`] into a [`Value`] suitable for `Envelope::announce`.
fn schema_to_value(schema: &Schema) -> Value {
    let bytes = rmp_serde::to_vec_named(schema).expect("serialize schema");
    rmp_serde::from_slice::<Value>(&bytes).expect("re-decode schema as Value")
}

/// Wire the "simulated adapter" side: returns `(sender, receiver)` for the
/// test to drive, while the runtime's `ConnectionHandler` is spawned in the
/// background.
///
/// The returned `(sender, receiver)` are the **test** side of the pair.
fn connect_simulated_peer(
    handle: &saikuro_runtime::handle::RuntimeHandle,
    peer_id: &str,
) -> (
    impl TransportSender + 'static,
    impl TransportReceiver + 'static,
) {
    let (test_transport, runtime_transport) =
        MemoryTransport::pair(peer_id, format!("{peer_id}-runtime"));
    let (test_sender, test_receiver) = test_transport.split();
    handle.accept_transport(
        runtime_transport,
        peer_id.to_owned(),
        CapabilitySet::empty(),
    );
    (test_sender, test_receiver)
}

//  A. Rust provider / simulated-Python client

/// A Rust in-process provider answers `math.add(a, b)` by summing two `Int`
/// args.  A simulated Python client sends raw `Call` frames and verifies the
/// computed result arrives back on the wire.
#[test]
fn a_rust_provider_simulated_client_call() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // 1:  Register a Rust in-process provider for `math`.
        let schema = make_schema_with_args("math", "add", 2);
        handle
            .register_schema(schema, "math-provider")
            .expect("register schema");

        handle.register_fn_provider("math-provider", vec!["math".to_owned()], |env| async move {
            let a = match env.args.first() {
                Some(Value::Int(n)) => *n,
                _ => 0,
            };
            let b = match env.args.get(1) {
                Some(Value::Int(n)) => *n,
                _ => 0,
            };
            ResponseEnvelope::ok(env.id, Value::Int(a + b))
        });

        // 2:  Connect a simulated adapter peer.
        let (mut tx, mut rx) = connect_simulated_peer(&handle, "py-client");

        // 3:  Send a raw `Call` envelope (exactly what Python SaikuroClient does).
        let call_env = Envelope::call("math.add", vec![Value::Int(3), Value::Int(7)]);
        let call_id = call_env.id;
        tx.send(encode_envelope(&call_env))
            .await
            .expect("send call");

        // 4:  Read response.
        let frame = rx.recv().await.expect("recv").expect("frame");
        let resp = decode_response(frame);

        assert!(resp.ok, "math.add call must succeed: {:?}", resp.error);
        assert_eq!(resp.id, call_id, "response ID must match request ID");
        assert_eq!(resp.result, Some(Value::Int(10)), "3 + 7 = 10");

        drop(tx);
    })
}

/// Multiple sequential calls from the same simulated peer all succeed and
/// return correct results.
#[test]
fn l_csharp_style_client_wire_fidelity() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Register a provider that returns the length of a byte slice.
        let schema = make_schema_with_args("buf", "len", 1);
        handle
            .register_schema(schema, "buf-provider")
            .expect("register schema");
        handle.register_fn_provider("buf-provider", vec!["buf".to_owned()], |env| async move {
            let n = match env.args.first() {
                Some(Value::Bytes(b)) => b.len() as i64,
                Some(Value::String(s)) => s.len() as i64,
                _ => 0,
            };
            ResponseEnvelope::ok(env.id, Value::Int(n))
        });

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "cs-client");

        // Encode exactly as a C# adapter would: named-field MessagePack.
        // C# adapters use the same rmp_serde::to_vec_named encoding as TypeScript.
        let env = Envelope::call("buf.len", vec![Value::Bytes(b"hello".to_vec())]);
        let id = env.id;
        let raw = rmp_serde::to_vec_named(&env).expect("csharp-style encode");
        tx.send(Bytes::from(raw)).await.expect("send");

        let resp_frame = rx.recv().await.expect("recv").expect("frame");
        let resp: ResponseEnvelope =
            rmp_serde::from_slice(&resp_frame).expect("csharp-style decode");

        assert!(resp.ok, "buf.len must succeed: {:?}", resp.error);
        assert_eq!(resp.id, id);
        assert_eq!(resp.result, Some(Value::Int(5)), "len(b\"hello\") == 5");

        drop(tx);
    })
}

/// Connects the Rust adapter `Client` to a live runtime via `InMemoryTransport`
/// and performs a call, verifying the full adapter stack end-to-end.
#[test]
fn m_rust_adapter_client_calls_runtime_provider() {
    saikuro_exec::block_on(async {
        use saikuro::transport::InMemoryTransport;
        use saikuro::Client;

        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Register a Rust in-process provider for `nums.negate`.
        let schema = make_schema_with_args("nums", "negate", 1);
        handle
            .register_schema(schema, "nums-provider")
            .expect("register schema");
        handle.register_fn_provider("nums-provider", vec!["nums".to_owned()], |env| async move {
            let n = match env.args.first() {
                Some(Value::Int(n)) => *n,
                _ => 0,
            };
            ResponseEnvelope::ok(env.id, Value::Int(-n))
        });

        // Create an InMemoryTransport pair and bridge to the runtime.
        let (client_side, bridge_side) = InMemoryTransport::pair();

        let (mut bridge_sender, mut bridge_receiver) = {
            let (ts, tr) =
                saikuro_transport::memory::MemoryTransport::pair("m-bridge", "m-bridge-rt");
            handle.accept_transport(
                tr,
                "m-rust-client".to_owned(),
                saikuro_core::capability::CapabilitySet::empty(),
            );
            ts.split()
        };

        let bridge = saikuro_exec::spawn(async move {
            use saikuro::transport::AdapterTransport;
            let mut adapter = bridge_side;
            loop {
                saikuro_exec::select! {
                    result = adapter.recv() => {
                        match result {
                            Ok(Some(frame)) => {
                                if bridge_sender.send(frame).await.is_err() { break; }
                            }
                            _ => break,
                        }
                    }
                    result = bridge_receiver.recv() => {
                        match result {
                            Ok(Some(frame)) => {
                                if adapter.send(frame).await.is_err() { break; }
                            }
                            _ => break,
                        }
                    }
                }
            }
        });

        // Build the Client from the adapter side.
        let client = Client::from_transport(Box::new(client_side), None).expect("build client");

        // Call `nums.negate(42)`.
        let result = client
            .call("nums.negate", vec![serde_json::json!(42)])
            .await
            .expect("call must succeed");

        assert_eq!(result, serde_json::json!(-42), "negate(42) must return -42");

        client.close().await.expect("close");
        bridge.abort();
    })
}

/// Connects the Rust adapter `Provider` to a live runtime via an in-process
/// bridge and verifies that a simulated client can call the provider through
/// the runtime.
#[test]
fn n_rust_adapter_provider_serves_simulated_client() {
    saikuro_exec::block_on(async {
        use saikuro::transport::InMemoryTransport;
        use saikuro::{ArgDescriptor, FunctionSchema, Provider, RegisterOptions};
        use saikuro_core::schema::{PrimitiveType, TypeDescriptor, Visibility};

        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Create an InMemoryTransport pair for provider <-> runtime communication.
        let (provider_side, bridge_side) = InMemoryTransport::pair();

        // Bridge the InMemoryTransport to the runtime's MemoryTransport.
        let (mut bridge_sender, mut bridge_receiver) = {
            let (ts, tr) =
                saikuro_transport::memory::MemoryTransport::pair("n-bridge", "n-bridge-rt");
            handle.accept_transport(
                tr,
                "n-rust-provider".to_owned(),
                saikuro_core::capability::CapabilitySet::empty(),
            );
            ts.split()
        };

        let bridge = saikuro_exec::spawn(async move {
            use saikuro::transport::AdapterTransport;
            let mut adapter = bridge_side;
            loop {
                saikuro_exec::select! {
                    result = adapter.recv() => {
                        match result {
                            Ok(Some(frame)) => {
                                if bridge_sender.send(frame).await.is_err() { break; }
                            }
                            _ => break,
                        }
                    }
                    result = bridge_receiver.recv() => {
                        match result {
                            Ok(Some(frame)) => {
                                if adapter.send(frame).await.is_err() { break; }
                            }
                            _ => break,
                        }
                    }
                }
            }
        });

        // Build the Provider for namespace `words`.
        let mut provider = Provider::new("words");
        provider.register_with_options(
            "reverse",
            |args: Vec<serde_json::Value>| async move {
                let s = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let reversed: String = s.chars().rev().collect();
                Ok(serde_json::Value::String(reversed))
            },
            RegisterOptions {
                schema: Some(FunctionSchema {
                    doc: None,
                    idempotent: false,
                    capabilities: vec![],
                    args: vec![ArgDescriptor {
                        name: "s".to_owned(),
                        r#type: TypeDescriptor::primitive(PrimitiveType::String),
                        optional: false,
                        doc: None,
                    }],
                    returns: Some(TypeDescriptor::primitive(PrimitiveType::String)),
                    visibility: Visibility::Public,
                }),
            },
        );

        // Serve on the provider side of the InMemoryTransport in a background task.
        let serve_task = saikuro_exec::spawn(async move {
            let _ = provider.serve_on(Box::new(provider_side)).await;
        });

        // Give the provider a moment to announce and for the runtime to register it.
        saikuro_exec::sleep(std::time::Duration::from_millis(50)).await;

        // Connect a simulated client and call `words.reverse`.
        let (mut client_tx, mut client_rx) = connect_simulated_peer(&handle, "n-sim-client");

        let call = Envelope::call("words.reverse", vec![Value::String("saikuro".into())]);
        let call_id = call.id;
        client_tx
            .send(encode_envelope(&call))
            .await
            .expect("send call");

        let frame = client_rx.recv().await.expect("recv").expect("frame");
        let resp = decode_response(frame);

        assert!(resp.ok, "words.reverse must succeed: {:?}", resp.error);
        assert_eq!(resp.id, call_id);
        assert_eq!(
            resp.result,
            Some(Value::String("orukias".into())),
            "reverse(\"saikuro\") == \"orukias\""
        );

        drop(client_tx);
        serve_task.abort();
        bridge.abort();
    })
}

/// A simulated Python provider connects, announces its schema, then the Rust
/// runtime's `handle.dispatch()` calls through to that provider.
#[test]
fn b_simulated_provider_rust_client_dispatch() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // 1:  Connect a simulated Python provider.
        let (mut provider_tx, mut provider_rx) = connect_simulated_peer(&handle, "py-provider");

        // 2:  Send an Announce so the runtime learns about `greeter.hello`.
        let schema = make_schema("greeter", "hello");
        let announce = Envelope::announce(schema_to_value(&schema));
        provider_tx
            .send(encode_envelope(&announce))
            .await
            .expect("send announce");

        // 3:  Read the ok response to the Announce.
        let frame = provider_rx.recv().await.expect("recv").expect("frame");
        let ok = decode_response(frame);
        assert!(ok.ok, "announce must return ok: {:?}", ok.error);

        // 4:  In a background task, simulate the Python provider's dispatch loop:
        //     read inbound Call frames, send back ResponseEnvelope frames.
        let handle_clone = handle.clone();
        let provider_loop = saikuro_exec::spawn(async move {
            // The runtime will route the Rust call to this provider over the wire.
            // We receive the forwarded Call frame.
            if let Ok(Some(call_frame)) = provider_rx.recv().await {
                let call: Envelope = decode_envelope(call_frame);
                assert_eq!(call.invocation_type, InvocationType::Call);
                assert_eq!(call.target, "greeter.hello");

                // Respond with a greeting.
                let resp = ResponseEnvelope::ok(call.id, Value::String("Hello, Saikuro!".into()));
                let frame = Bytes::from(resp.to_msgpack().expect("encode response"));
                provider_tx.send(frame).await.expect("send response");
            }
            drop(handle_clone); // keep handle alive until here
        });

        // Give the announce a moment to be processed before dispatching.
        saikuro_exec::sleep(std::time::Duration::from_millis(20)).await;

        // 5:  Rust-side dispatch through handle (simulates any Rust caller).
        let call = Envelope::call("greeter.hello", vec![]);
        let resp = handle.dispatch(call, &CapabilitySet::empty()).await;

        assert!(resp.ok, "greeter.hello call must succeed: {:?}", resp.error);
        assert_eq!(
            resp.result,
            Some(Value::String("Hello, Saikuro!".into())),
            "response must carry the greeting"
        );

        provider_loop.await.expect("provider loop");
    })
}

/// Both a Rust in-process provider (namespace `svc`) and a simulated adapter
/// provider (namespace `ext`) are registered in the same runtime.  Calls to
/// both namespaces succeed from a shared simulated client connection.
#[test]
fn c_rust_and_simulated_providers_coexist() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Rust provider for `svc`
        let svc_schema = make_schema("svc", "ping");
        handle
            .register_schema(svc_schema, "svc-provider")
            .expect("register svc schema");
        handle.register_fn_provider("svc-provider", vec!["svc".to_owned()], |env| async move {
            ResponseEnvelope::ok(env.id, Value::String("pong".into()))
        });

        // Simulated external provider for `ext`
        let (mut ext_tx, mut ext_rx) = connect_simulated_peer(&handle, "ext-provider");

        let ext_schema = make_schema_with_args("ext", "echo", 1);
        let announce = Envelope::announce(schema_to_value(&ext_schema));
        ext_tx
            .send(encode_envelope(&announce))
            .await
            .expect("announce");
        let ack_frame = ext_rx.recv().await.expect("recv").expect("frame");
        let ack = decode_response(ack_frame);
        assert!(ack.ok, "ext announce must succeed");

        // Run the external provider loop in the background.
        let ext_loop = saikuro_exec::spawn(async move {
            while let Ok(Some(frame)) = ext_rx.recv().await {
                let call: Envelope = decode_envelope(frame);
                let result = call.args.first().cloned().unwrap_or(Value::Null);
                let resp = ResponseEnvelope::ok(call.id, result);
                ext_tx
                    .send(Bytes::from(resp.to_msgpack().unwrap()))
                    .await
                    .expect("send response");
            }
        });

        saikuro_exec::sleep(std::time::Duration::from_millis(20)).await;

        // Shared simulated client
        let (mut client_tx, mut client_rx) = connect_simulated_peer(&handle, "shared-client");

        // Call the Rust provider.
        let ping = Envelope::call("svc.ping", vec![]);
        let ping_id = ping.id;
        client_tx
            .send(encode_envelope(&ping))
            .await
            .expect("send ping");
        let frame = client_rx.recv().await.expect("recv").expect("frame");
        let ping_resp = decode_response(frame);
        assert!(ping_resp.ok, "svc.ping must succeed");
        assert_eq!(ping_resp.id, ping_id);
        assert_eq!(ping_resp.result, Some(Value::String("pong".into())));

        // Call the simulated external provider.
        let echo = Envelope::call("ext.echo", vec![Value::String("hello".into())]);
        let echo_id = echo.id;
        client_tx
            .send(encode_envelope(&echo))
            .await
            .expect("send echo");
        let frame = client_rx.recv().await.expect("recv").expect("frame");
        let echo_resp = decode_response(frame);
        assert!(echo_resp.ok, "ext.echo must succeed: {:?}", echo_resp.error);
        assert_eq!(echo_resp.id, echo_id);
        assert_eq!(echo_resp.result, Some(Value::String("hello".into())));

        drop(client_tx);
        ext_loop.abort();
    })
}

/// Simulated client sends a `Batch` envelope containing two `Call` items.
/// Runtime must return one response with `result` as an array of results,
/// each in the same order as the batch items.
#[test]
fn d_batch_call_from_simulated_client() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Register a simple identity provider for `items`.
        let schema = make_schema_with_args("items", "get", 1);
        handle
            .register_schema(schema, "items-provider")
            .expect("register schema");
        handle.register_fn_provider(
            "items-provider",
            vec!["items".to_owned()],
            |env| async move {
                let val = env.args.first().cloned().unwrap_or(Value::Null);
                ResponseEnvelope::ok(env.id, val)
            },
        );

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "batch-client");

        // Build two inner Call envelopes.
        let item_a = Envelope::call("items.get", vec![Value::Int(1)]);
        let item_b = Envelope::call("items.get", vec![Value::Int(2)]);
        let batch_id = InvocationId::new();

        // Construct the batch envelope exactly as TypeScript/Python adapters do.
        let batch_env = Envelope {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Batch,
            id: batch_id,
            target: "$saikuro.batch".to_owned(),
            args: vec![],
            meta: Default::default(),
            capability: None,
            batch_items: Some(vec![item_a, item_b]),
            stream_control: None,
            seq: None,
        };

        tx.send(encode_envelope(&batch_env))
            .await
            .expect("send batch");

        let frame = rx.recv().await.expect("recv").expect("frame");
        let resp = decode_response(frame);

        assert!(resp.ok, "batch must succeed: {:?}", resp.error);
        assert_eq!(resp.id, batch_id);

        // The result must be an ordered array of sub-results.
        let results = match resp.result {
            Some(Value::Array(arr)) => arr,
            other => panic!("expected Array result, got {other:?}"),
        };
        assert_eq!(results.len(), 2, "batch of 2 items must return 2 results");

        drop(tx);
    })
}

/// Simulated client calls a namespace that has never been registered.
/// Runtime must return `NamespaceNotFound` or `NoProvider` on the wire.
#[test]
fn e_call_unknown_namespace_returns_error_on_wire() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "err-client");

        let env = Envelope::call("nope.fn", vec![]);
        let id = env.id;
        tx.send(encode_envelope(&env)).await.expect("send");

        let frame = rx.recv().await.expect("recv").expect("frame");
        let resp = decode_response(frame);

        assert!(!resp.ok, "call to unknown namespace must fail");
        assert_eq!(resp.id, id, "error response must echo the request ID");
        let err = resp.error.expect("error detail must be present");
        assert!(
            err.code == ErrorCode::NamespaceNotFound
                || err.code == ErrorCode::NoProvider
                || err.code == ErrorCode::FunctionNotFound,
            "expected a not-found error code, got {:?}",
            err.code
        );

        drop(tx);
    })
}

/// Simulated client sends a malformed frame (not valid MessagePack).
/// Runtime must return `MalformedEnvelope`.
#[test]
fn e_malformed_frame_returns_error_on_wire() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "bad-client");

        // Send garbage bytes.
        tx.send(Bytes::from_static(b"\xff\xfe\xfd\x00invalid"))
            .await
            .expect("send garbage");

        let frame = rx.recv().await.expect("recv").expect("frame");
        let resp = decode_response(frame);

        assert!(!resp.ok, "garbage frame must return an error");
        let err = resp.error.expect("error detail");
        assert_eq!(
            err.code,
            ErrorCode::MalformedEnvelope,
            "expected MalformedEnvelope, got {:?}",
            err.code
        );

        drop(tx);
    })
}

/// Simulated provider announces its schema; then a second simulated client
/// immediately calls one of the announced functions through the runtime.
/// The provider handles the forwarded call and sends back the response.
#[test]
fn f_announce_then_client_call_round_trip() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Simulated provider
        let (mut prov_tx, mut prov_rx) = connect_simulated_peer(&handle, "prov-f");

        let schema = make_schema_with_args("calc", "square", 1);
        let announce = Envelope::announce(schema_to_value(&schema));
        prov_tx
            .send(encode_envelope(&announce))
            .await
            .expect("announce");
        let ack = decode_response(prov_rx.recv().await.unwrap().unwrap());
        assert!(ack.ok, "announce must succeed");

        // Provider loop: read inbound Call, return square of first arg.
        let prov_loop = saikuro_exec::spawn(async move {
            while let Ok(Some(frame)) = prov_rx.recv().await {
                let call: Envelope = decode_envelope(frame);
                let n = match call.args.first() {
                    Some(Value::Int(n)) => *n,
                    _ => 0,
                };
                let resp = ResponseEnvelope::ok(call.id, Value::Int(n * n));
                prov_tx
                    .send(Bytes::from(resp.to_msgpack().unwrap()))
                    .await
                    .unwrap();
            }
        });

        saikuro_exec::sleep(std::time::Duration::from_millis(20)).await;

        // Simulated client
        let (mut cli_tx, mut cli_rx) = connect_simulated_peer(&handle, "cli-f");

        let call = Envelope::call("calc.square", vec![Value::Int(9)]);
        let call_id = call.id;
        cli_tx
            .send(encode_envelope(&call))
            .await
            .expect("send call");

        let frame = cli_rx.recv().await.expect("recv").expect("frame");
        let resp = decode_response(frame);

        assert!(resp.ok, "calc.square must succeed: {:?}", resp.error);
        assert_eq!(resp.id, call_id);
        assert_eq!(resp.result, Some(Value::Int(81)), "9² = 81");

        drop(cli_tx);
        prov_loop.abort();
    })
}

/// Ten simulated clients connect concurrently to the same runtime and each
/// make a call to the same Rust in-process provider.  All calls must succeed
/// independently, demonstrating safe concurrent dispatch.
#[test]
fn g_concurrent_simulated_clients() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Register a provider that returns the input value.
        let schema = make_schema_with_args("echo", "run", 1);
        handle
            .register_schema(schema, "echo-provider")
            .expect("register schema");
        handle.register_fn_provider("echo-provider", vec!["echo".to_owned()], |env| async move {
            let val = env.args.first().cloned().unwrap_or(Value::Null);
            ResponseEnvelope::ok(env.id, val)
        });

        const N: usize = 10;
        let mut tasks = Vec::with_capacity(N);

        for i in 0..N {
            let handle_clone = handle.clone();
            tasks.push(saikuro_exec::spawn(async move {
                let (mut tx, mut rx) =
                    connect_simulated_peer(&handle_clone, &format!("concurrent-client-{i}"));
                let env = Envelope::call("echo.run", vec![Value::Int(i as i64)]);
                let id = env.id;
                tx.send(encode_envelope(&env)).await.expect("send");
                let frame = rx.recv().await.expect("recv").expect("frame");
                let resp = decode_response(frame);
                assert!(resp.ok, "client {i} call must succeed: {:?}", resp.error);
                assert_eq!(resp.id, id);
                assert_eq!(resp.result, Some(Value::Int(i as i64)));
                drop(tx);
            }));
        }

        for t in tasks {
            t.await.expect("client task panicked");
        }
    })
}

/// Simulated client sends a `Cast` frame.  The runtime must immediately return
/// `ok_empty` without waiting for any provider:  fire-and-forget semantics.
#[test]
fn h_cast_fire_and_forget_returns_ok_empty() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Register a no-op provider so the schema validator can find the function.
        let schema = make_schema_with_args("logger", "info", 1);
        handle
            .register_schema(schema, "logger-provider")
            .expect("register schema");
        handle.register_fn_provider(
            "logger-provider",
            vec!["logger".to_owned()],
            |env| async move {
                // Cast providers receive the work item but do not need to respond.
                ResponseEnvelope::ok_empty(env.id)
            },
        );

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "cast-client");

        let cast = Envelope::cast("logger.info", vec![Value::String("fire!".into())]);
        let cast_id = cast.id;
        tx.send(encode_envelope(&cast)).await.expect("send cast");

        let frame = rx.recv().await.expect("recv").expect("frame");
        let resp = decode_response(frame);

        assert!(resp.ok, "cast must return ok: {:?}", resp.error);
        assert_eq!(resp.id, cast_id);
        assert!(resp.result.is_none(), "cast response must have no result");

        drop(tx);
    })
}

/// A simulated provider connects, announces, serves one call, then disconnects.
/// A new provider with the same namespace connects and re-announces.
/// Subsequent calls are routed to the new provider.
#[test]
fn i_provider_reconnect_and_reannounce() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // First provider instance
        {
            let (mut prov_tx, mut prov_rx) = connect_simulated_peer(&handle, "reconnect-prov-v1");

            let schema = make_schema("svc2", "op");
            let announce = Envelope::announce(schema_to_value(&schema));
            prov_tx
                .send(encode_envelope(&announce))
                .await
                .expect("announce v1");
            let ack = decode_response(prov_rx.recv().await.unwrap().unwrap());
            assert!(ack.ok, "v1 announce must succeed");

            // Serve one call.
            let prov_loop = saikuro_exec::spawn(async move {
                if let Ok(Some(frame)) = prov_rx.recv().await {
                    let call: Envelope = decode_envelope(frame);
                    let resp = ResponseEnvelope::ok(call.id, Value::Int(1));
                    prov_tx
                        .send(Bytes::from(resp.to_msgpack().unwrap()))
                        .await
                        .unwrap();
                }
                // drop prov_tx:  signals disconnect to the handler
            });

            saikuro_exec::sleep(std::time::Duration::from_millis(20)).await;

            let (mut cli_tx, mut cli_rx) = connect_simulated_peer(&handle, "cli-reconnect-v1");
            let call = Envelope::call("svc2.op", vec![]);
            cli_tx.send(encode_envelope(&call)).await.expect("send");
            let resp = decode_response(cli_rx.recv().await.unwrap().unwrap());
            assert!(resp.ok, "v1 call must succeed");
            assert_eq!(resp.result, Some(Value::Int(1)));
            drop(cli_tx);

            prov_loop.await.expect("v1 provider loop");
        }

        // Brief wait for the connection handler to notice the drop.
        saikuro_exec::sleep(std::time::Duration::from_millis(40)).await;

        // Second provider instance (same namespace)
        let (mut prov2_tx, mut prov2_rx) = connect_simulated_peer(&handle, "reconnect-prov-v2");

        let schema2 = make_schema("svc2", "op");
        let announce2 = Envelope::announce(schema_to_value(&schema2));
        prov2_tx
            .send(encode_envelope(&announce2))
            .await
            .expect("announce v2");
        let ack2 = decode_response(prov2_rx.recv().await.unwrap().unwrap());
        assert!(ack2.ok, "v2 announce must succeed");

        let prov2_loop = saikuro_exec::spawn(async move {
            while let Ok(Some(frame)) = prov2_rx.recv().await {
                let call: Envelope = decode_envelope(frame);
                let resp = ResponseEnvelope::ok(call.id, Value::Int(2));
                prov2_tx
                    .send(Bytes::from(resp.to_msgpack().unwrap()))
                    .await
                    .unwrap();
            }
        });

        saikuro_exec::sleep(std::time::Duration::from_millis(20)).await;

        let (mut cli2_tx, mut cli2_rx) = connect_simulated_peer(&handle, "cli-reconnect-v2");
        let call2 = Envelope::call("svc2.op", vec![]);
        cli2_tx
            .send(encode_envelope(&call2))
            .await
            .expect("send v2");
        let resp2 = decode_response(cli2_rx.recv().await.unwrap().unwrap());
        assert!(resp2.ok, "v2 call must succeed: {:?}", resp2.error);
        assert_eq!(resp2.result, Some(Value::Int(2)), "v2 provider must answer");

        drop(cli2_tx);
        prov2_loop.abort();
    })
}

/// Simulates what the TypeScript `SaikuroClient.call()` does on the wire:
/// encode a `Call` envelope with `rmp_serde::to_vec_named` (named fields),
/// send raw bytes with no length prefix (MemoryTransport is message-framed),
/// then decode the `ResponseEnvelope` from raw bytes.
#[test]
fn j_typescript_style_client_wire_fidelity() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        // Register a provider that uppercases a string.
        let schema = make_schema_with_args("str", "upper", 1);
        handle
            .register_schema(schema, "str-provider")
            .expect("register schema");
        handle.register_fn_provider("str-provider", vec!["str".to_owned()], |env| async move {
            let s = match env.args.first() {
                Some(Value::String(s)) => s.to_uppercase(),
                _ => String::new(),
            };
            ResponseEnvelope::ok(env.id, Value::String(s))
        });

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "ts-client");

        // Encode exactly as TypeScript does: named field map via rmp_serde::to_vec_named.
        let env = Envelope::call("str.upper", vec![Value::String("hello".into())]);
        let id = env.id;
        let raw = rmp_serde::to_vec_named(&env).expect("ts-style encode");
        tx.send(Bytes::from(raw)).await.expect("send");

        let resp_frame = rx.recv().await.expect("recv").expect("frame");
        let resp: ResponseEnvelope = rmp_serde::from_slice(&resp_frame).expect("ts-style decode");

        assert!(resp.ok, "str.upper must succeed: {:?}", resp.error);
        assert_eq!(resp.id, id);
        assert_eq!(resp.result, Some(Value::String("HELLO".into())));

        drop(tx);
    })
}

/// Every response frame from the runtime must carry the same ID as the
/// request that triggered it:  regardless of concurrency.  This is a critical
/// invariant for all adapters to correctly correlate responses to pending
/// promises/futures.
#[test]
fn k_response_id_always_matches_request_id() {
    saikuro_exec::block_on(async {
        let runtime = SaikuroRuntime::builder().build();
        let handle = runtime.handle();

        let schema = make_schema("id_check", "fn");
        handle
            .register_schema(schema, "idcheck-provider")
            .expect("register schema");
        handle.register_fn_provider(
            "idcheck-provider",
            vec!["id_check".to_owned()],
            |env| async move { ResponseEnvelope::ok(env.id, Value::Null) },
        );

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "id-client");

        // Send 20 calls pipelined:  don't wait for each response.
        let mut sent_ids: Vec<InvocationId> = Vec::new();
        for _ in 0..20 {
            let env = Envelope::call("id_check.fn", vec![]);
            sent_ids.push(env.id);
            tx.send(encode_envelope(&env)).await.expect("send");
        }

        let mut received_ids: Vec<InvocationId> = Vec::new();
        for _ in 0..20 {
            let frame = rx.recv().await.expect("recv").expect("frame");
            let resp = decode_response(frame);
            assert!(resp.ok, "pipelined call must succeed");
            received_ids.push(resp.id);
        }

        // Every sent ID must appear exactly once in the received IDs.
        let mut sent_sorted = sent_ids.clone();
        let mut recv_sorted = received_ids.clone();
        sent_sorted.sort();
        recv_sorted.sort();
        assert_eq!(
            sent_sorted, recv_sorted,
            "every response must echo its request ID"
        );

        drop(tx);
    })
}
