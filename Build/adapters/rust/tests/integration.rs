//! Rust adapter integration tests.
//!
//! All tests use [`InMemoryTransport`] so no network is required.

use saikuro::{Client, InMemoryTransport, Provider};
use saikuro_core::{
    envelope::{Envelope, InvocationType, StreamControl},
    ResponseEnvelope,
};
use serde_json::json;

// Helpers

/// Spin up a provider on one side of an in-memory pair and return a connected
/// client on the other side.  The provider task is detached; it will stop when
/// the client drops its side of the channel.
async fn make_pair(provider: Provider) -> Client {
    let (provider_t, client_t) = InMemoryTransport::pair();

    saikuro_exec::spawn(async move {
        // Ignore the error; the test ends when the client drops its side.
        let _ = provider.serve_on(Box::new(provider_t)).await;
    });

    // Yield to give the provider task a scheduling opportunity.  The provider
    // sends its announce frame synchronously during `serve_on` startup; by
    // yielding here we ensure the frame is in the transport buffer before
    // the client task starts draining it.
    saikuro_exec::yield_now().await;

    Client::from_transport(Box::new(client_t), None).unwrap()
}

// Value conversion

#[test]
fn value_null_roundtrips() {
    saikuro_exec::block_on(async {
        use saikuro::value::{core_to_json, json_to_core};
        let v = json!(null);
        let core = json_to_core(v.clone());
        assert_eq!(core_to_json(core), v);
    })
}

#[test]
fn value_integer_roundtrips() {
    saikuro_exec::block_on(async {
        use saikuro::value::{core_to_json, json_to_core};
        let v = json!(42i64);
        let core = json_to_core(v.clone());
        assert_eq!(core_to_json(core), v);
    })
}

#[test]
fn value_string_roundtrips() {
    saikuro_exec::block_on(async {
        use saikuro::value::{core_to_json, json_to_core};
        let v = json!("hello world");
        let core = json_to_core(v.clone());
        assert_eq!(core_to_json(core), v);
    })
}

#[test]
fn value_array_roundtrips() {
    saikuro_exec::block_on(async {
        use saikuro::value::{core_to_json, json_to_core};
        let v = json!([1, 2, 3]);
        let core = json_to_core(v.clone());
        assert_eq!(core_to_json(core), v);
    })
}

#[test]
fn value_object_roundtrips() {
    saikuro_exec::block_on(async {
        use saikuro::value::{core_to_json, json_to_core};
        let v = json!({"key": "value", "n": 7});
        let core = json_to_core(v.clone());
        assert_eq!(core_to_json(core), v);
    })
}

// Error types

#[test]
fn error_display_transport() {
    saikuro_exec::block_on(async {
        let e = saikuro::Error::Transport("connection refused".into());
        assert!(e.to_string().contains("connection refused"));
    })
}

#[test]
fn error_display_timeout() {
    saikuro_exec::block_on(async {
        let e = saikuro::Error::Timeout {
            target: "math.add".into(),
            ms: 5000,
        };
        let s = e.to_string();
        assert!(s.contains("math.add"));
        assert!(s.contains("5000"));
    })
}

#[test]
fn error_display_remote() {
    saikuro_exec::block_on(async {
        let e = saikuro::Error::remote("CapabilityDenied", "not allowed", None);
        let s = e.to_string();
        assert!(s.contains("CapabilityDenied"));
        assert!(s.contains("not allowed"));
    })
}

// Schema builder

#[test]
fn schema_build_basic() {
    saikuro_exec::block_on(async {
        use saikuro::{FunctionSchema, NamespaceSchema};
        let mut ns = NamespaceSchema::new();
        ns.insert(
            "add",
            FunctionSchema {
                doc: Some("Add two numbers.".into()),
                idempotent: true,
                ..Default::default()
            },
        );
        // to_core should not panic
        let _ = ns.to_core();
    })
}

#[test]
fn schema_capabilities_convert() {
    saikuro_exec::block_on(async {
        use saikuro::{FunctionSchema, NamespaceSchema};
        let mut ns = NamespaceSchema::new();
        ns.insert(
            "op",
            FunctionSchema {
                capabilities: vec!["admin.write".into(), "math.advanced".into()],
                ..Default::default()
            },
        );
        let core_ns = ns.to_core();
        let fn_schema = core_ns.functions.get("op").expect("op function missing");
        let cap_strs: Vec<String> = fn_schema
            .capabilities
            .iter()
            .map(|c| c.to_string())
            .collect();
        assert!(cap_strs.contains(&"admin.write".to_string()));
        assert!(cap_strs.contains(&"math.advanced".to_string()));
    })
}

// In-memory end-to-end: call

#[test]
fn call_add_returns_sum() {
    saikuro_exec::block_on(async {
        let mut provider = Provider::new("math");
        provider.register("add", |args: Vec<serde_json::Value>| async move {
            let a = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
            let b = args.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
            Ok(json!(a + b))
        });

        let client = make_pair(provider).await;
        let result = client
            .call("math.add", vec![json!(3), json!(4)])
            .await
            .unwrap();
        assert_eq!(result, json!(7));
        client.close().await.unwrap();
    })
}

#[test]
fn call_unknown_function_returns_error() {
    saikuro_exec::block_on(async {
        let provider = Provider::new("math");
        let client = make_pair(provider).await;
        let err = client.call("math.nonexistent", vec![]).await.unwrap_err();
        // Should get a Remote error with FunctionNotFound-style message
        assert!(matches!(err, saikuro::Error::Remote { .. }));
        client.close().await.unwrap();
    })
}

#[test]
fn call_handler_error_propagates() {
    saikuro_exec::block_on(async {
        let mut provider = Provider::new("math");
        provider.register("divide", |args: Vec<serde_json::Value>| async move {
            let b = args.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
            if b == 0.0 {
                return Err(saikuro::Error::remote(
                    "DivisionByZero",
                    "divisor is zero",
                    None,
                ));
            }
            Ok(json!(args[0].as_f64().unwrap_or(0.0) / b))
        });

        let client = make_pair(provider).await;
        let err = client
            .call("math.divide", vec![json!(10), json!(0)])
            .await
            .unwrap_err();
        assert!(matches!(err, saikuro::Error::Remote { .. }));
        client.close().await.unwrap();
    })
}

// In-memory end-to-end: cast

#[test]
fn cast_does_not_block() {
    saikuro_exec::block_on(async {
        use std::sync::{Arc, Mutex};

        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
        let log_clone = log.clone();

        let mut provider = Provider::new("log");
        provider.register("write", move |args: Vec<serde_json::Value>| {
            let log = log_clone.clone();
            async move {
                if let Some(msg) = args.first().and_then(|v| v.as_str()) {
                    log.lock().unwrap().push(msg.to_owned());
                }
                Ok(json!(null))
            }
        });

        let client = make_pair(provider).await;
        client
            .cast("log.write", vec![json!("hello")])
            .await
            .unwrap();

        // Give the provider a moment to process the cast.
        saikuro_exec::sleep(std::time::Duration::from_millis(20)).await;
        client.close().await.unwrap();
    })
}

// In-memory end-to-end: batch

#[test]
fn batch_returns_all_results() {
    saikuro_exec::block_on(async {
        let mut provider = Provider::new("math");
        provider.register("double", |args: Vec<serde_json::Value>| async move {
            let n = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
            Ok(json!(n * 2))
        });

        let client = make_pair(provider).await;
        let results = client
            .batch(vec![
                ("math.double".into(), vec![json!(1)]),
                ("math.double".into(), vec![json!(2)]),
                ("math.double".into(), vec![json!(3)]),
            ])
            .await
            .unwrap();

        // Batch returns all results as a JSON array.
        assert!(!results.is_empty());
        client.close().await.unwrap();
    })
}

// InMemoryTransport pair sanity check

#[test]
fn in_memory_transport_sends_and_receives() {
    saikuro_exec::block_on(async {
        use bytes::Bytes;
        use saikuro::transport::AdapterTransport;

        let (mut a, mut b) = InMemoryTransport::pair();
        let frame = Bytes::from_static(b"hello");
        a.send(frame.clone()).await.unwrap();
        let received = b.recv().await.unwrap();
        assert_eq!(received, Some(frame));
    })
}

#[test]
fn resource_roundtrip_with_simulated_runtime() {
    saikuro_exec::block_on(async {
        use saikuro::transport::AdapterTransport;

        let (client_side, mut runtime_side) = InMemoryTransport::pair();

        let runtime_task = saikuro_exec::spawn(async move {
            let frame = runtime_side
                .recv()
                .await
                .expect("runtime recv")
                .expect("resource request frame");
            let env = Envelope::from_msgpack(&frame).expect("decode request envelope");
            assert_eq!(env.invocation_type, InvocationType::Resource);
            assert_eq!(env.target, "files.open");

            let response =
                ResponseEnvelope::ok(env.id, saikuro_core::value::Value::String("ok".into()));
            runtime_side
                .send(bytes::Bytes::from(
                    response.to_msgpack().expect("encode response"),
                ))
                .await
                .expect("send response");
        });

        let client = Client::from_transport(Box::new(client_side), None).expect("build client");
        let result = client
            .resource("files.open", vec![json!("/tmp/file")])
            .await
            .expect("resource should succeed");
        assert_eq!(result, json!("ok"));

        runtime_task.await.expect("runtime task");
        client.close().await.expect("close client");
    })
}

#[test]
fn stream_roundtrip_with_simulated_runtime() {
    saikuro_exec::block_on(async {
        use saikuro::transport::AdapterTransport;

        let (client_side, mut runtime_side) = InMemoryTransport::pair();

        let runtime_task = saikuro_exec::spawn(async move {
            let frame = runtime_side
                .recv()
                .await
                .expect("runtime recv")
                .expect("stream open frame");
            let env = Envelope::from_msgpack(&frame).expect("decode stream envelope");
            assert_eq!(env.invocation_type, InvocationType::Stream);
            assert_eq!(env.target, "events.watch");

            let item1 =
                ResponseEnvelope::stream_item(env.id, 0, saikuro_core::value::Value::Int(1));
            runtime_side
                .send(bytes::Bytes::from(
                    item1.to_msgpack().expect("encode item1"),
                ))
                .await
                .expect("send item1");

            let item2 =
                ResponseEnvelope::stream_item(env.id, 1, saikuro_core::value::Value::Int(2));
            runtime_side
                .send(bytes::Bytes::from(
                    item2.to_msgpack().expect("encode item2"),
                ))
                .await
                .expect("send item2");

            let end = ResponseEnvelope::stream_end(env.id, 2);
            runtime_side
                .send(bytes::Bytes::from(end.to_msgpack().expect("encode end")))
                .await
                .expect("send end");
        });

        let client = Client::from_transport(Box::new(client_side), None).expect("build client");
        let mut stream = client
            .stream("events.watch", vec![])
            .await
            .expect("open stream");

        assert_eq!(
            stream.next().await.expect("item1").expect("ok item1"),
            json!(1)
        );
        assert_eq!(
            stream.next().await.expect("item2").expect("ok item2"),
            json!(2)
        );
        assert!(stream.next().await.is_none(), "stream should end");

        runtime_task.await.expect("runtime task");
        client.close().await.expect("close client");
    })
}

#[test]
fn channel_send_receive_and_close_with_simulated_runtime() {
    saikuro_exec::block_on(async {
        use saikuro::transport::AdapterTransport;

        let (client_side, mut runtime_side) = InMemoryTransport::pair();

        let runtime_task = saikuro_exec::spawn(async move {
            let open_frame = runtime_side
                .recv()
                .await
                .expect("runtime recv")
                .expect("channel open frame");
            let open_env = Envelope::from_msgpack(&open_frame).expect("decode channel open");
            assert_eq!(open_env.invocation_type, InvocationType::Channel);
            assert_eq!(open_env.target, "chat.room");

            let send_frame = runtime_side
                .recv()
                .await
                .expect("runtime recv send")
                .expect("channel send frame");
            let send_env = Envelope::from_msgpack(&send_frame).expect("decode channel send");
            assert_eq!(send_env.invocation_type, InvocationType::Channel);
            assert_eq!(send_env.id, open_env.id);
            assert_eq!(send_env.args.len(), 1);

            let outbound = ResponseEnvelope::stream_item(
                open_env.id,
                0,
                saikuro_core::value::Value::String("pong".into()),
            );
            runtime_side
                .send(bytes::Bytes::from(
                    outbound.to_msgpack().expect("encode outbound item"),
                ))
                .await
                .expect("send outbound item");

            let close_frame = runtime_side
                .recv()
                .await
                .expect("runtime recv close")
                .expect("channel close frame");
            let close_env = Envelope::from_msgpack(&close_frame).expect("decode close frame");
            assert_eq!(close_env.id, open_env.id);
            assert!(
                close_env.stream_control.is_some(),
                "close should set stream control"
            );

            let end = ResponseEnvelope::stream_end(open_env.id, 1);
            runtime_side
                .send(bytes::Bytes::from(end.to_msgpack().expect("encode end")))
                .await
                .expect("send end");
        });

        let client = Client::from_transport(Box::new(client_side), None).expect("build client");
        let mut channel = client
            .channel("chat.room", vec![json!("room-1")])
            .await
            .expect("open channel");

        channel
            .send(json!("ping"))
            .await
            .expect("send channel item");
        assert_eq!(
            channel
                .next()
                .await
                .expect("channel inbound")
                .expect("channel ok"),
            json!("pong")
        );
        channel.close().await.expect("close channel");
        assert!(channel.next().await.is_none(), "channel should end");

        runtime_task.await.expect("runtime task");
        client.close().await.expect("close client");
    })
}

#[test]
fn log_envelope_is_forwarded_to_runtime() {
    saikuro_exec::block_on(async {
        use saikuro::transport::AdapterTransport;

        let (client_side, mut runtime_side) = InMemoryTransport::pair();

        let runtime_task = saikuro_exec::spawn(async move {
            let frame = runtime_side
                .recv()
                .await
                .expect("runtime recv")
                .expect("log frame");
            let env = Envelope::from_msgpack(&frame).expect("decode log envelope");
            assert_eq!(env.invocation_type, InvocationType::Log);
            assert_eq!(env.target, "$log");
            assert_eq!(env.args.len(), 1);
        });

        let client = Client::from_transport(Box::new(client_side), None).expect("build client");
        client
            .log("info", "tests", "hello", Some(json!({"k": "v"})))
            .await
            .expect("log should send");

        runtime_task.await.expect("runtime task");
        client.close().await.expect("close client");
    })
}

#[test]
fn call_with_timeout_reports_timeout() {
    saikuro_exec::block_on(async {
        use saikuro::transport::AdapterTransport;

        let (client_side, mut runtime_side) = InMemoryTransport::pair();

        let _runtime_task = saikuro_exec::spawn(async move {
            // Receive one call envelope and intentionally do not respond.
            let _ = runtime_side.recv().await;
            // Keep the transport alive longer than the test timeout so the
            // client's timeout can fire instead of the transport closing.
            saikuro_exec::sleep(std::time::Duration::from_millis(200)).await;
        });

        let client = Client::from_transport(Box::new(client_side), None).expect("build client");
        let err = client
            .call_with_timeout(
                "math.add",
                vec![json!(1), json!(2)],
                Some(std::time::Duration::from_millis(20)),
            )
            .await
            .expect_err("call should time out");

        assert!(matches!(err, saikuro::Error::Timeout { .. }));
        client.close().await.expect("close client");
    })
}

#[test]
fn client_acknowledges_announce_on_connect() {
    saikuro_exec::block_on(async {
        use saikuro::transport::AdapterTransport;

        let (client_side, mut runtime_side) = InMemoryTransport::pair();

        let announce_id = saikuro_core::invocation::InvocationId::new();
        let announce = Envelope {
            version: saikuro_core::PROTOCOL_VERSION,
            invocation_type: InvocationType::Announce,
            id: announce_id,
            target: "$announce".into(),
            args: vec![saikuro_core::value::Value::Null],
            meta: Default::default(),
            capability: None,
            batch_items: None,
            stream_control: None,
            seq: None,
        };

        runtime_side
            .send(bytes::Bytes::from(
                announce.to_msgpack().expect("encode announce"),
            ))
            .await
            .expect("send announce");

        let client = Client::from_transport(Box::new(client_side), None).expect("build client");

        let ack_frame = runtime_side
            .recv()
            .await
            .expect("runtime recv ack")
            .expect("ack frame");
        let ack = ResponseEnvelope::from_msgpack(&ack_frame).expect("decode ack");
        assert_eq!(ack.id, announce_id);
        assert!(ack.ok, "announce ack should be ok");

        client.close().await.expect("close client");
    })
}

#[test]
fn envelope_roundtrip_msgpack_preserves_fields() {
    saikuro_exec::block_on(async {
        let original = Envelope::call(
            "math.add",
            vec![
                saikuro_core::value::Value::Int(1),
                saikuro_core::value::Value::Int(2),
            ],
        );

        let bytes = original.to_msgpack().expect("encode envelope");
        let decoded = Envelope::from_msgpack(&bytes).expect("decode envelope");

        assert_eq!(decoded.invocation_type, original.invocation_type);
        assert_eq!(decoded.id, original.id);
        assert_eq!(decoded.target, original.target);
        assert_eq!(decoded.args, original.args);
    })
}

#[test]
fn channel_abort_sends_abort_control_frame() {
    saikuro_exec::block_on(async {
        use saikuro::transport::AdapterTransport;

        let (client_side, mut runtime_side) = InMemoryTransport::pair();

        let runtime_task = saikuro_exec::spawn(async move {
            let open_frame = runtime_side
                .recv()
                .await
                .expect("runtime recv")
                .expect("channel open frame");
            let open_env = Envelope::from_msgpack(&open_frame).expect("decode channel open");
            assert_eq!(open_env.invocation_type, InvocationType::Channel);

            let abort_frame = runtime_side
                .recv()
                .await
                .expect("runtime recv abort")
                .expect("channel abort frame");
            let abort_env = Envelope::from_msgpack(&abort_frame).expect("decode abort frame");
            assert_eq!(abort_env.id, open_env.id);
            assert_eq!(abort_env.stream_control, Some(StreamControl::Abort));
        });

        let client = Client::from_transport(Box::new(client_side), None).expect("build client");
        let channel = client
            .channel("chat.room", vec![json!("room-1")])
            .await
            .expect("open channel");
        channel.abort().await.expect("abort channel");

        runtime_task.await.expect("runtime task");
        client.close().await.expect("close client");
    })
}
