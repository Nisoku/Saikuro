use crate::common;
use crate::format;
use crate::shared_test;
use crate::vec;
use crate::Box;
use crate::String;
use crate::TestSuite;
use crate::ToOwned;
use crate::Vec;
use bytes::Bytes;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::{Envelope, InvocationType},
    schema::{
        ArgumentDescriptor, FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema,
        PrimitiveType, Schema, TypeDescriptor, TypeMap, Visibility,
    },
    InvocationId, ResponseEnvelope, PROTOCOL_VERSION,
};
use saikuro_event::{ErrorCode, Value};
use saikuro_runtime::SaikuroRuntime;
use saikuro_transport::{MemoryTransport, Transport, TransportReceiver, TransportSender};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "wire::a_rust_provider_simulated_client_call",
        a_rust_provider_simulated_client_call,
    );
    shared_test!(
        suite,
        "wire::l_csharp_style_client_wire_fidelity",
        l_csharp_style_client_wire_fidelity,
    );
    shared_test!(
        suite,
        "wire::b_simulated_provider_rust_client_dispatch",
        b_simulated_provider_rust_client_dispatch,
    );
    shared_test!(
        suite,
        "wire::c_rust_and_simulated_providers_coexist",
        c_rust_and_simulated_providers_coexist,
    );
    shared_test!(
        suite,
        "wire::d_batch_call_from_simulated_client",
        d_batch_call_from_simulated_client,
    );
    shared_test!(
        suite,
        "wire::e_call_unknown_namespace_returns_error_on_wire",
        e_call_unknown_namespace_returns_error_on_wire,
    );
    shared_test!(
        suite,
        "wire::e_malformed_frame_returns_error_on_wire",
        e_malformed_frame_returns_error_on_wire,
    );
    shared_test!(
        suite,
        "wire::f_announce_then_client_call_round_trip",
        f_announce_then_client_call_round_trip,
    );
    shared_test!(
        suite,
        "wire::g_concurrent_simulated_clients",
        g_concurrent_simulated_clients,
    );
    shared_test!(
        suite,
        "wire::h_cast_fire_and_forget_returns_ok_empty",
        h_cast_fire_and_forget_returns_ok_empty,
    );
    shared_test!(
        suite,
        "wire::i_provider_reconnect_and_reannounce",
        i_provider_reconnect_and_reannounce,
    );
    shared_test!(
        suite,
        "wire::j_typescript_style_client_wire_fidelity",
        j_typescript_style_client_wire_fidelity,
    );
    shared_test!(
        suite,
        "wire::k_response_id_always_matches_request_id",
        k_response_id_always_matches_request_id,
    );
    shared_test!(
        suite,
        "wire::raw_frame_relay_byte_faithful",
        raw_frame_relay_byte_faithful,
    );
}

pub fn encode_envelope(env: &Envelope) -> Bytes {
    Bytes::from(env.to_msgpack().expect("encode envelope"))
}

pub fn decode_response(frame: Bytes) -> ResponseEnvelope {
    ResponseEnvelope::from_msgpack(&frame).expect("decode response")
}

pub fn decode_envelope(frame: Bytes) -> Envelope {
    Envelope::from_msgpack(&frame).expect("decode envelope")
}

pub fn make_schema_with_args(namespace: &str, function: &str, n_args: usize) -> Schema {
    let args = (0..n_args)
        .map(|i| ArgumentDescriptor {
            name: format!("arg{i}"),
            r#type: TypeDescriptor::primitive(PrimitiveType::Any),
            optional: false,
            default: None,
            doc: None,
        })
        .collect();
    let mut functions = FunctionMap::new();
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
    let mut namespaces = NamespaceMap::new();
    namespaces.insert(
        namespace.to_owned(),
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

fn make_schema(namespace: &str, function: &str) -> Schema {
    make_schema_with_args(namespace, function, 0)
}

pub fn connect_simulated_peer(
    handle: &saikuro_runtime::handle::RuntimeHandle,
    peer_id: &str,
) -> (
    impl TransportSender + 'static,
    impl TransportReceiver + 'static,
) {
    let log = common::null_log();
    let (test_transport, runtime_transport) =
        MemoryTransport::pair(peer_id, format!("{peer_id}-runtime"), log);
    let (test_sender, test_receiver) = test_transport.split();
    handle.accept_transport(
        runtime_transport,
        peer_id.to_owned(),
        CapabilitySet::empty(),
    );
    (test_sender, test_receiver)
}

fn a_rust_provider_simulated_client_call() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let schema = make_schema_with_args("math", "add", 2);
        handle
            .register_schema(schema, "math-provider")
            .await
            .map_err(|_| "register schema")?;

        let _ = handle
            .register_fn_provider("math-provider", vec!["math".to_owned()], |env| async move {
                let a = match env.args.first() {
                    Some(Value::Int(n)) => *n,
                    _ => 0,
                };
                let b = match env.args.get(1) {
                    Some(Value::Int(n)) => *n,
                    _ => 0,
                };
                ResponseEnvelope::ok(env.id, Value::Int(a + b))
            })
            .await;

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "py-client");

        let call_env = Envelope::call("math.add", vec![Value::Int(3), Value::Int(7)])
            .map_err(|_| "entropy available")?;
        let call_id = call_env.id;
        tx.send(encode_envelope(&call_env))
            .await
            .map_err(|_| "send call")?;

        let frame = rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let resp = decode_response(frame);

        assert!(resp.ok, "math.add call must succeed: {:?}", resp.error);
        assert_eq!(resp.id, call_id, "response ID must match request ID");
        assert_eq!(resp.result, Some(Value::Int(10)), "3 + 7 = 10");

        drop(tx);
        Ok(())
    })
}

fn l_csharp_style_client_wire_fidelity() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let schema = make_schema_with_args("buf", "len", 1);
        handle
            .register_schema(schema, "buf-provider")
            .await
            .map_err(|_| "register schema")?;
        let _ = handle
            .register_fn_provider("buf-provider", vec!["buf".to_owned()], |env| async move {
                let n = match env.args.first() {
                    Some(Value::Bytes(b)) => b.len() as i64,
                    Some(Value::String(s)) => s.len() as i64,
                    _ => 0,
                };
                ResponseEnvelope::ok(env.id, Value::Int(n))
            })
            .await;

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "cs-client");

        let env = Envelope::call("buf.len", vec![Value::Bytes(b"hello".to_vec())])
            .map_err(|_| "entropy available")?;
        let id = env.id;
        let raw = env.to_msgpack().map_err(|_| "csharp-style encode")?;
        tx.send(Bytes::from(raw)).await.map_err(|_| "send")?;

        let resp_frame = rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let resp: ResponseEnvelope =
            ResponseEnvelope::from_msgpack(&resp_frame).map_err(|_| "csharp-style decode")?;

        assert!(resp.ok, "buf.len must succeed: {:?}", resp.error);
        assert_eq!(resp.id, id);
        assert_eq!(resp.result, Some(Value::Int(5)), "len(b\"hello\") == 5");

        drop(tx);
        Ok(())
    })
}

fn b_simulated_provider_rust_client_dispatch() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let (mut provider_tx, mut provider_rx) = connect_simulated_peer(&handle, "py-provider");

        let schema = make_schema("greeter", "hello");
        let announce = Envelope::announce(common::schema_to_value(&schema))
            .map_err(|_| "entropy available")?;
        provider_tx
            .send(encode_envelope(&announce))
            .await
            .map_err(|_| "send announce")?;

        let frame = provider_rx
            .recv()
            .await
            .map_err(|_| "recv")?
            .ok_or("frame")?;
        let ok = decode_response(frame);
        assert!(ok.ok, "announce must return ok: {:?}", ok.error);

        let handle_clone = handle.clone();
        let provider_loop = saikuro_exec::spawn(async move {
            if let Ok(Some(call_frame)) = provider_rx.recv().await {
                let call: Envelope = decode_envelope(call_frame);
                assert_eq!(call.invocation_type, InvocationType::Call);
                assert_eq!(call.target, "greeter.hello");

                let resp = ResponseEnvelope::ok(call.id, Value::String("Hello, Saikuro!".into()));
                let frame = Bytes::from(resp.to_msgpack().expect("encode response"));
                provider_tx.send(frame).await.expect("send response");
            }
            drop(handle_clone);
        });

        saikuro_exec::sleep(core::time::Duration::from_millis(20)).await;

        let call = Envelope::call("greeter.hello", vec![]).map_err(|_| "entropy available")?;
        let resp = handle.dispatch(call, &CapabilitySet::empty()).await;

        assert!(resp.ok, "greeter.hello call must succeed: {:?}", resp.error);
        assert_eq!(
            resp.result,
            Some(Value::String("Hello, Saikuro!".into())),
            "response must carry the greeting"
        );

        provider_loop.await.map_err(|_| "provider loop")?;
        Ok(())
    })
}

fn c_rust_and_simulated_providers_coexist() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let svc_schema = make_schema("svc", "ping");
        handle
            .register_schema(svc_schema, "svc-provider")
            .await
            .map_err(|_| "register svc schema")?;
        let _ = handle
            .register_fn_provider("svc-provider", vec!["svc".to_owned()], |env| async move {
                ResponseEnvelope::ok(env.id, Value::String("pong".into()))
            })
            .await;

        let (mut ext_tx, mut ext_rx) = connect_simulated_peer(&handle, "ext-provider");

        let ext_schema = make_schema_with_args("ext", "echo", 1);
        let announce = Envelope::announce(common::schema_to_value(&ext_schema))
            .map_err(|_| "entropy available")?;
        ext_tx
            .send(encode_envelope(&announce))
            .await
            .map_err(|_| "announce")?;
        let ack_frame = ext_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let ack = decode_response(ack_frame);
        assert!(ack.ok, "ext announce must succeed");

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

        saikuro_exec::sleep(core::time::Duration::from_millis(20)).await;

        let (mut client_tx, mut client_rx) = connect_simulated_peer(&handle, "shared-client");

        let ping = Envelope::call("svc.ping", vec![]).map_err(|_| "entropy available")?;
        let ping_id = ping.id;
        client_tx
            .send(encode_envelope(&ping))
            .await
            .map_err(|_| "send ping")?;
        let frame = client_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let ping_resp = decode_response(frame);
        assert!(ping_resp.ok, "svc.ping must succeed");
        assert_eq!(ping_resp.id, ping_id);
        assert_eq!(ping_resp.result, Some(Value::String("pong".into())));

        let echo = Envelope::call("ext.echo", vec![Value::String("hello".into())])
            .map_err(|_| "entropy available")?;
        let echo_id = echo.id;
        client_tx
            .send(encode_envelope(&echo))
            .await
            .map_err(|_| "send echo")?;
        let frame = client_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let echo_resp = decode_response(frame);
        assert!(echo_resp.ok, "ext.echo must succeed: {:?}", echo_resp.error);
        assert_eq!(echo_resp.id, echo_id);
        assert_eq!(echo_resp.result, Some(Value::String("hello".into())));

        drop(client_tx);
        let _ = ext_loop.abort();
        Ok(())
    })
}

fn d_batch_call_from_simulated_client() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let schema = make_schema_with_args("items", "get", 1);
        handle
            .register_schema(schema, "items-provider")
            .await
            .map_err(|_| "register schema")?;
        let _ = handle
            .register_fn_provider(
                "items-provider",
                vec!["items".to_owned()],
                |env| async move {
                    let val = env.args.first().cloned().unwrap_or(Value::Null);
                    ResponseEnvelope::ok(env.id, val)
                },
            )
            .await;

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "batch-client");

        let item_a =
            Envelope::call("items.get", vec![Value::Int(1)]).map_err(|_| "entropy available")?;
        let item_b =
            Envelope::call("items.get", vec![Value::Int(2)]).map_err(|_| "entropy available")?;
        let batch_id = InvocationId::new().map_err(|_| "entropy available")?;

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
            .map_err(|_| "send batch")?;

        let frame = rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let resp = decode_response(frame);

        assert!(resp.ok, "batch must succeed: {:?}", resp.error);
        assert_eq!(resp.id, batch_id);

        let results = match resp.result {
            Some(Value::Array(arr)) => arr,
            other => panic!("expected Array result, got {other:?}"),
        };
        assert_eq!(results.len(), 2, "batch of 2 items must return 2 results");

        drop(tx);
        Ok(())
    })
}

fn e_call_unknown_namespace_returns_error_on_wire() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "err-client");

        let env = Envelope::call("nope.fn", vec![]).map_err(|_| "entropy available")?;
        let id = env.id;
        tx.send(encode_envelope(&env)).await.map_err(|_| "send")?;

        let frame = rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let resp = decode_response(frame);

        assert!(!resp.ok, "call to unknown namespace must fail");
        assert_eq!(resp.id, id, "error response must echo the request ID");
        let err = resp.error.ok_or("error detail must be present")?;
        assert!(
            err.code == ErrorCode::NamespaceNotFound
                || err.code == ErrorCode::NoProvider
                || err.code == ErrorCode::FunctionNotFound,
            "expected a not-found error code, got {:?}",
            err.code
        );

        drop(tx);
        Ok(())
    })
}

fn e_malformed_frame_returns_error_on_wire() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "bad-client");

        tx.send(Bytes::from_static(b"\xff\xfe\xfd\x00invalid"))
            .await
            .map_err(|_| "send garbage")?;

        let frame = rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let resp = decode_response(frame);

        assert!(!resp.ok, "garbage frame must return an error");
        let err = resp.error.ok_or("error detail")?;
        assert_eq!(
            err.code,
            ErrorCode::MalformedEnvelope,
            "expected MalformedEnvelope, got {:?}",
            err.code
        );

        drop(tx);
        Ok(())
    })
}

fn f_announce_then_client_call_round_trip() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let (mut prov_tx, mut prov_rx) = connect_simulated_peer(&handle, "prov-f");

        let schema = make_schema_with_args("calc", "square", 1);
        let announce = Envelope::announce(common::schema_to_value(&schema))
            .map_err(|_| "entropy available")?;
        prov_tx
            .send(encode_envelope(&announce))
            .await
            .map_err(|_| "announce")?;
        let ack = decode_response(prov_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?);
        assert!(ack.ok, "announce must succeed");

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

        saikuro_exec::sleep(core::time::Duration::from_millis(20)).await;

        let (mut cli_tx, mut cli_rx) = connect_simulated_peer(&handle, "cli-f");

        let call =
            Envelope::call("calc.square", vec![Value::Int(9)]).map_err(|_| "entropy available")?;
        let call_id = call.id;
        cli_tx
            .send(encode_envelope(&call))
            .await
            .map_err(|_| "send call")?;

        let frame = cli_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let resp = decode_response(frame);

        assert!(resp.ok, "calc.square must succeed: {:?}", resp.error);
        assert_eq!(resp.id, call_id);
        assert_eq!(resp.result, Some(Value::Int(81)), "9*9 = 81");

        drop(cli_tx);
        let _ = prov_loop.abort();
        Ok(())
    })
}

fn g_concurrent_simulated_clients() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let schema = make_schema_with_args("echo", "run", 1);
        handle
            .register_schema(schema, "echo-provider")
            .await
            .map_err(|_| "register schema")?;
        let _ = handle
            .register_fn_provider("echo-provider", vec!["echo".to_owned()], |env| async move {
                let val = env.args.first().cloned().unwrap_or(Value::Null);
                ResponseEnvelope::ok(env.id, val)
            })
            .await;

        const N: usize = 10;
        let mut tasks = Vec::with_capacity(N);

        for i in 0..N {
            let handle_clone = handle.clone();
            tasks.push(saikuro_exec::spawn(async move {
                let (mut tx, mut rx) =
                    connect_simulated_peer(&handle_clone, &format!("concurrent-client-{i}"));
                let env = Envelope::call("echo.run", vec![Value::Int(i as i64)])
                    .expect("entropy available");
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
            t.await.map_err(|_| "client task panicked")?;
        }
        Ok(())
    })
}

fn h_cast_fire_and_forget_returns_ok_empty() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let schema = make_schema_with_args("logger", "info", 1);
        handle
            .register_schema(schema, "logger-provider")
            .await
            .map_err(|_| "register schema")?;
        let _ = handle
            .register_fn_provider(
                "logger-provider",
                vec!["logger".to_owned()],
                |env| async move { ResponseEnvelope::ok_empty(env.id) },
            )
            .await;

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "cast-client");

        let cast = Envelope::cast("logger.info", vec![Value::String("fire!".into())])
            .map_err(|_| "entropy available")?;
        let cast_id = cast.id;
        tx.send(encode_envelope(&cast))
            .await
            .map_err(|_| "send cast")?;

        let frame = rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let resp = decode_response(frame);

        assert!(resp.ok, "cast must return ok: {:?}", resp.error);
        assert_eq!(resp.id, cast_id);
        assert!(resp.result.is_none(), "cast response must have no result");

        drop(tx);
        Ok(())
    })
}

fn i_provider_reconnect_and_reannounce() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        {
            let (mut prov_tx, mut prov_rx) = connect_simulated_peer(&handle, "reconnect-prov-v1");

            let schema = make_schema("svc2", "op");
            let announce = Envelope::announce(common::schema_to_value(&schema))
                .map_err(|_| "entropy available")?;
            prov_tx
                .send(encode_envelope(&announce))
                .await
                .map_err(|_| "announce v1")?;
            let ack = decode_response(prov_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?);
            assert!(ack.ok, "v1 announce must succeed");

            let prov_loop = saikuro_exec::spawn(async move {
                if let Ok(Some(frame)) = prov_rx.recv().await {
                    let call: Envelope = decode_envelope(frame);
                    let resp = ResponseEnvelope::ok(call.id, Value::Int(1));
                    prov_tx
                        .send(Bytes::from(resp.to_msgpack().unwrap()))
                        .await
                        .unwrap();
                }
            });

            saikuro_exec::sleep(core::time::Duration::from_millis(20)).await;

            let (mut cli_tx, mut cli_rx) = connect_simulated_peer(&handle, "cli-reconnect-v1");
            let call = Envelope::call("svc2.op", vec![]).map_err(|_| "entropy available")?;
            cli_tx
                .send(encode_envelope(&call))
                .await
                .map_err(|_| "send")?;
            let resp = decode_response(cli_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?);
            assert!(resp.ok, "v1 call must succeed");
            assert_eq!(resp.result, Some(Value::Int(1)));
            drop(cli_tx);

            prov_loop.await.map_err(|_| "v1 provider loop")?;
        }

        saikuro_exec::sleep(core::time::Duration::from_millis(40)).await;

        let (mut prov2_tx, mut prov2_rx) = connect_simulated_peer(&handle, "reconnect-prov-v2");

        let schema2 = make_schema("svc2", "op");
        let announce2 = Envelope::announce(common::schema_to_value(&schema2))
            .map_err(|_| "entropy available")?;
        prov2_tx
            .send(encode_envelope(&announce2))
            .await
            .map_err(|_| "announce v2")?;
        let ack2 = decode_response(prov2_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?);
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

        saikuro_exec::sleep(core::time::Duration::from_millis(20)).await;

        let (mut cli2_tx, mut cli2_rx) = connect_simulated_peer(&handle, "cli-reconnect-v2");
        let call2 = Envelope::call("svc2.op", vec![]).map_err(|_| "entropy available")?;
        cli2_tx
            .send(encode_envelope(&call2))
            .await
            .map_err(|_| "send v2")?;
        let resp2 = decode_response(cli2_rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?);
        assert!(resp2.ok, "v2 call must succeed: {:?}", resp2.error);
        assert_eq!(resp2.result, Some(Value::Int(2)), "v2 provider must answer");

        drop(cli2_tx);
        let _ = prov2_loop.abort();
        Ok(())
    })
}

fn j_typescript_style_client_wire_fidelity() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let schema = make_schema_with_args("str", "upper", 1);
        handle
            .register_schema(schema, "str-provider")
            .await
            .map_err(|_| "register schema")?;
        let _ = handle
            .register_fn_provider("str-provider", vec!["str".to_owned()], |env| async move {
                let s = match env.args.first() {
                    Some(Value::String(s)) => s.to_uppercase(),
                    _ => String::new(),
                };
                ResponseEnvelope::ok(env.id, Value::String(s))
            })
            .await;

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "ts-client");

        let env = Envelope::call("str.upper", vec![Value::String("hello".into())])
            .map_err(|_| "entropy available")?;
        let id = env.id;
        let raw = env.to_msgpack().map_err(|_| "ts-style encode")?;
        tx.send(Bytes::from(raw)).await.map_err(|_| "send")?;

        let resp_frame = rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
        let resp: ResponseEnvelope =
            ResponseEnvelope::from_msgpack(&resp_frame).map_err(|_| "ts-style decode")?;

        assert!(resp.ok, "str.upper must succeed: {:?}", resp.error);
        assert_eq!(resp.id, id);
        assert_eq!(resp.result, Some(Value::String("HELLO".into())));

        drop(tx);
        Ok(())
    })
}

fn k_response_id_always_matches_request_id() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let schema = make_schema("id_check", "fn");
        handle
            .register_schema(schema, "idcheck-provider")
            .await
            .map_err(|_| "register schema")?;
        let _ = handle
            .register_fn_provider(
                "idcheck-provider",
                vec!["id_check".to_owned()],
                |env| async move { ResponseEnvelope::ok(env.id, Value::Null) },
            )
            .await;

        let (mut tx, mut rx) = connect_simulated_peer(&handle, "id-client");

        let mut sent_ids: Vec<InvocationId> = Vec::new();
        for _ in 0..20 {
            let env = Envelope::call("id_check.fn", vec![]).map_err(|_| "entropy available")?;
            sent_ids.push(env.id);
            tx.send(encode_envelope(&env)).await.map_err(|_| "send")?;
        }

        let mut received_ids: Vec<InvocationId> = Vec::new();
        for _ in 0..20 {
            let frame = rx.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
            let resp = decode_response(frame);
            assert!(resp.ok, "pipelined call must succeed");
            received_ids.push(resp.id);
        }

        let mut sent_sorted = sent_ids.clone();
        let mut recv_sorted = received_ids.clone();
        sent_sorted.sort();
        recv_sorted.sort();
        assert_eq!(
            sent_sorted, recv_sorted,
            "every response must echo its request ID"
        );

        drop(tx);
        Ok(())
    })
}

fn raw_frame_relay_byte_faithful() -> Result<(), &'static str> {
    crate::block_on(async {
        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();
        let schema = make_schema_with_args("math", "add", 5);
        let (mut provider_tx, mut provider_rx) =
            connect_simulated_peer(&handle, "relay-provider-peer");
        let announce =
            Envelope::announce(common::schema_to_value(&schema)).map_err(|_| "announce id")?;
        provider_tx
            .send(encode_envelope(&announce))
            .await
            .map_err(|_| "send announce")?;
        let ack = decode_response(provider_rx.recv().await.map_err(|_| "recv")?.ok_or("ack")?);
        assert!(ack.ok, "provider announce must succeed: {:?}", ack.error);

        // Client peer sends a call whose frame exercises int widths that are
        // observable in the encoded bytes (fixint vs int64/uint64).
        let (mut client_tx, mut client_rx) = connect_simulated_peer(&handle, "relay-client-peer");
        let call = Envelope::call(
            "math.add",
            vec![
                Value::UInt(7),
                Value::UInt(u64::MAX),
                Value::Int(-7),
                Value::Int(i64::MIN),
                Value::Float(1.5),
            ],
        )
        .map_err(|_| "call id")?;
        let call_id = call.id;
        let raw = encode_envelope(&call);
        client_tx.send(raw.clone()).await.map_err(|_| "send call")?;

        // The relayed frame must be byte-identical to the origin frame
        let relayed = provider_rx
            .recv()
            .await
            .map_err(|_| "recv relay")?
            .ok_or("relay frame")?;
        assert!(
            relayed.as_ref() == raw.as_ref(),
            "relayed frame must be byte-identical to the origin frame (raw forward)"
        );
        // Compare against the *decoded* origin
        let origin_env = decode_envelope(raw);
        let relayed_env = decode_envelope(relayed);
        assert_eq!(
            relayed_env.id, origin_env.id,
            "relayed id must match origin"
        );
        assert_eq!(
            relayed_env.target, origin_env.target,
            "relayed target must match origin"
        );
        assert_eq!(
            relayed_env.args, origin_env.args,
            "relayed args must match the decoded origin (int widths preserved)"
        );
        assert_eq!(
            origin_env.args[1],
            Value::UInt(u64::MAX),
            "uint64 width must survive the relay"
        );
        assert_eq!(
            origin_env.args[3],
            Value::Int(i64::MIN),
            "int64 width must survive the relay"
        );

        // Provider replies so the client call completes through the relay.
        let resp = ResponseEnvelope::ok(call_id, Value::Int(7));
        provider_tx
            .send(Bytes::from(resp.to_msgpack().expect("encode response")))
            .await
            .map_err(|_| "send response")?;
        let resp = decode_response(
            client_rx
                .recv()
                .await
                .map_err(|_| "recv resp")?
                .ok_or("resp")?,
        );
        assert!(
            resp.ok,
            "call must complete through the relay: {:?}",
            resp.error
        );
        assert_eq!(resp.result, Some(Value::Int(7)));

        drop(client_tx);
        drop(provider_tx);
        Ok(())
    })
}
