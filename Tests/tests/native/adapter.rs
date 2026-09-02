//! Host-only adapter-bridge tests.

use saikuro_core::{
    capability::CapabilitySet,
    envelope::Envelope,
    schema::{PrimitiveType, TypeDescriptor, Visibility},
    ResponseEnvelope,
};
use saikuro_event::Value;
use saikuro_runtime::SaikuroRuntime;
use saikuro_transport::{Transport, TransportReceiver, TransportSender};

pub fn register(suite: &mut saikuro_tests::TestSuite) {
    suite.register(
        "adapter::m_rust_adapter_client_calls_runtime_provider",
        m_rust_adapter_client_calls_runtime_provider,
    );
    suite.register(
        "adapter::n_rust_adapter_provider_serves_simulated_client",
        n_rust_adapter_provider_serves_simulated_client,
    );
}

fn m_rust_adapter_client_calls_runtime_provider() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        use saikuro::transport::InMemoryTransport;
        use saikuro::Client;

        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let schema = saikuro_tests::shared::wire::wire::make_schema_with_args("nums", "negate", 1);
        handle
            .register_schema(schema, "nums-provider")
            .await
            .map_err(|_| "register schema")?;
        let _ = handle
            .register_fn_provider("nums-provider", saikuro_tests::vec!["nums".to_owned()], |env| {
                async move {
                    let n = match env.args.first() {
                        Some(Value::Int(n)) => *n,
                        _ => 0,
                    };
                    ResponseEnvelope::ok(env.id, Value::Int(-n))
                }
            })
            .await;

        let (client_side, bridge_side) = InMemoryTransport::pair();

        let (mut bridge_sender, mut bridge_receiver) = {
            let bridge_log = saikuro_tests::shared::common::null_log();
            let (ts, tr) =
                saikuro_transport::MemoryTransport::pair("m-bridge", "m-bridge-rt", bridge_log);
            handle.accept_transport(tr, "m-rust-client".to_owned(), CapabilitySet::empty());
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

        let client = Client::from_transport(Box::new(client_side), None)
            .map_err(|_| "build adapter client")?;

        saikuro_exec::sleep(core::time::Duration::from_millis(50)).await;

        let result = client
            .call("nums.negate", saikuro_tests::vec![serde_json::json!(7)])
            .await
            .map_err(|_| "adapter client call")?;
        assert_eq!(result, serde_json::json!(-7), "negate(7) == -7");

        client.close().await.map_err(|_| "close client")?;
        let _ = bridge.abort();
        Ok(())
    })
}

fn n_rust_adapter_provider_serves_simulated_client() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        use saikuro::transport::InMemoryTransport;
        use saikuro::{ArgDescriptor, FunctionSchema, Provider, RegisterOptions};

        let runtime = SaikuroRuntime::builder().build().await;
        let handle = runtime.handle();

        let (provider_side, bridge_side) = InMemoryTransport::pair();

        let (mut bridge_sender, mut bridge_receiver) = {
            let bridge_log = saikuro_tests::shared::common::null_log();
            let (ts, tr) =
                saikuro_transport::MemoryTransport::pair("n-bridge", "n-bridge-rt", bridge_log);
            handle.accept_transport(tr, "n-rust-provider".to_owned(), CapabilitySet::empty());
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
                    capabilities: saikuro_tests::vec![],
                    args: saikuro_tests::vec![ArgDescriptor {
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

        let serve_task = saikuro_exec::spawn(async move {
            let _ = provider.serve_on(Box::new(provider_side)).await;
        });

        saikuro_exec::sleep(core::time::Duration::from_millis(50)).await;

        let (mut client_tx, mut client_rx) =
            saikuro_tests::shared::wire::wire::connect_simulated_peer(&handle, "n-sim-client");

        let call = Envelope::call("words.reverse", saikuro_tests::vec![Value::String("saikuro".into())])
            .map_err(|_| "entropy available")?;
        let call_id = call.id;
        client_tx
            .send(saikuro_tests::shared::wire::wire::encode_envelope(&call))
            .await
            .map_err(|_| "send call")?;

        let frame = client_rx
            .recv()
            .await
            .map_err(|_| "recv")?
            .ok_or("frame")?;
        let resp = saikuro_tests::shared::wire::wire::decode_response(frame);

        assert!(resp.ok, "words.reverse must succeed: {:?}", resp.error);
        assert_eq!(resp.id, call_id);
        assert_eq!(
            resp.result,
            Some(Value::String("orukias".into())),
            "reverse(\"saikuro\") == \"orukias\""
        );

        drop(client_tx);
        let _ = serve_task.abort();
        let _ = bridge.abort();
        Ok(())
    })
}
