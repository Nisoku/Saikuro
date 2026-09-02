use crate::common;
use crate::Box;
use crate::TestSuite;
use crate::ToString;
use bytes::Bytes;
use saikuro_core::capability::CapabilitySet;
use saikuro_core::envelope::Envelope;
use saikuro_core::schema::{
    ArgumentDescriptor, FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema, PrimitiveType,
    Schema, TypeDescriptor, TypeMap, Visibility,
};
use saikuro_core::ResponseEnvelope;
use saikuro_event::Value;
use saikuro_exec::mpsc;
use saikuro_router::provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem};
use saikuro_router::router::InvocationRouter;
use saikuro_runtime::{RuntimeConfig, SaikuroRuntime};
use saikuro_transport::{MemoryTransport, Transport, TransportReceiver, TransportSender};

pub fn register(suite: &mut TestSuite) {
    suite.register("runtime::schema_round_trip", schema_round_trip);
    suite.register("runtime::router_dispatch_direct", router_dispatch_direct);
    suite.register("runtime::transport_round_trip", transport_round_trip);
    suite.register("runtime::runtime_full_stack", runtime_full_stack);
    config_capacity::register(suite);
    schema_registration::register(suite);
}

pub mod config_capacity;
pub mod schema_registration;

fn schema_round_trip() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut functions = FunctionMap::new();
        let _ = functions.insert(
            "add".into(),
            FunctionSchema {
                args: vec![
                    ArgumentDescriptor {
                        name: "a".into(),
                        r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                        optional: false,
                        default: None,
                        doc: None,
                    },
                    ArgumentDescriptor {
                        name: "b".into(),
                        r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                        optional: false,
                        default: None,
                        doc: None,
                    },
                ],
                returns: TypeDescriptor::primitive(PrimitiveType::I64),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        );
        let mut namespaces = NamespaceMap::new();
        let _ = namespaces.insert(
            "math".into(),
            NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        );
        let schema = Schema {
            version: 1,
            namespaces: Box::new(namespaces),
            types: Box::new(TypeMap::new()),
        };

        assert_eq!(schema.version, 1);
        let ns = schema.namespaces.get("math").ok_or("missing math ns")?;
        let fns = &ns.functions;
        assert!(fns.get("add").is_some());
        let add = fns.get("add").unwrap();
        assert_eq!(add.args.len(), 2);
        assert_eq!(add.args[0].name, "a");
        assert_eq!(add.args[1].name, "b");
        Ok(())
    })
}

fn router_dispatch_direct() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) =
            saikuro_exec::mpsc::channel::<ProviderWorkItem>(common::capacity(64));
        let handle = ProviderHandle::new("math-provider", vec!["math".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        common::spawn_responder(work_rx, Value::Int(7));

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::call("math.compute", vec![Value::Int(3)]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;
        assert!(resp.ok);
        assert_eq!(resp.result, Some(Value::Int(7)));
        Ok(())
    })
}

/// Transport round-trip: client sends an envelope through `MemoryTransport`;
/// a bridge task decodes it, dispatches through the router, and returns the
/// response over the same transport.
fn transport_round_trip() -> Result<(), &'static str> {
    crate::block_on(async {
        let log = common::null_log();

        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(common::capacity(64));
        let registry = ProviderRegistry::new();
        registry
            .register(ProviderHandle::new(
                "math-provider".to_string(),
                vec!["math".to_string()],
                work_tx,
            ))
            .await;

        let router = InvocationRouter::with_providers(registry);

        let provider_join = saikuro_exec::spawn(async move {
            while let Some(item) = work_rx.recv().await {
                if let Some(tx) = item.response_tx {
                    let _ = tx.send(ResponseEnvelope::ok(item.envelope.id, Value::Int(42)));
                }
            }
        });

        let (client_transport, server_transport) =
            MemoryTransport::pair("client", "server", log.clone());

        let (mut client_tx, mut client_rx) = client_transport.split();
        let (mut server_tx, mut server_rx) = server_transport.split();

        let bridge_router = router.clone();
        let bridge_join = saikuro_exec::spawn(async move {
            while let Ok(Some(frame)) = server_rx.recv().await {
                if let Ok(env) = Envelope::from_msgpack(&frame) {
                    let resp = bridge_router.dispatch(env).await;
                    if let Ok(bytes) = resp.to_msgpack() {
                        let _ = server_tx.send(Bytes::from(bytes)).await;
                    }
                }
            }
        });

        let env = Envelope::call("math.add", vec![Value::Int(10), Value::Int(32)])
            .map_err(|_| "Envelope::call")?;
        let frame = env.to_msgpack().map_err(|_| "to_msgpack")?;
        client_tx
            .send(Bytes::from(frame))
            .await
            .map_err(|_| "send")?;

        let resp_frame = client_rx
            .recv()
            .await
            .map_err(|_| "recv")?
            .ok_or("no response frame")?;
        let resp = ResponseEnvelope::from_msgpack(&resp_frame).map_err(|_| "from_msgpack")?;
        assert!(resp.ok);
        assert_eq!(resp.result, Some(Value::Int(42)));

        drop(client_tx);
        drop(client_rx);
        drop(router);
        let _ = bridge_join;
        let _ = provider_join;
        Ok(())
    })
}

/// Full saikuro-runtime integration: build a `SaikuroRuntime` via
/// `RuntimeBuilder`, register a schema and an in-process fn_provider through
/// `RuntimeHandle`, and dispatch through the full stack (validation +
/// capability check + routing).
fn runtime_full_stack() -> Result<(), &'static str> {
    crate::block_on(async {
        let config = RuntimeConfig::default();
        let runtime = SaikuroRuntime::builder().config(config).build().await;
        let handle = runtime.handle();

        let mut functions = FunctionMap::new();
        let _ = functions.insert(
            "add".into(),
            FunctionSchema {
                args: vec![
                    ArgumentDescriptor {
                        name: "a".into(),
                        r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                        optional: false,
                        default: None,
                        doc: None,
                    },
                    ArgumentDescriptor {
                        name: "b".into(),
                        r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                        optional: false,
                        default: None,
                        doc: None,
                    },
                ],
                returns: TypeDescriptor::primitive(PrimitiveType::I64),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        );
        let mut namespaces = NamespaceMap::new();
        let _ = namespaces.insert(
            "math".into(),
            NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        );
        let schema = Schema {
            version: 1,
            namespaces: Box::new(namespaces),
            types: Box::new(TypeMap::new()),
        };

        handle
            .register_schema(schema, "test-provider")
            .await
            .map_err(|_| "register_schema")?;

        let _provider = handle
            .register_fn_provider(
                "test-provider".to_string(),
                vec!["math".to_string()],
                |env| async move {
                    let a = env.args.get(0).cloned().unwrap_or(Value::Int(0));
                    let b = env.args.get(1).cloned().unwrap_or(Value::Int(0));
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            ResponseEnvelope::ok(env.id, Value::Int(x + y))
                        }
                        _ => ResponseEnvelope::err(
                            env.id,
                            saikuro_event::ErrorDetail::new(
                                saikuro_event::ErrorCode::InvalidArguments,
                                "expected two integers",
                            ),
                        ),
                    }
                },
            )
            .await;

        let env = Envelope::call("math.add", vec![Value::Int(17), Value::Int(25)])
            .map_err(|_| "Envelope::call")?;
        let resp = handle.dispatch(env, &CapabilitySet::default()).await;
        assert!(resp.ok);
        assert_eq!(resp.result, Some(Value::Int(42)));
        Ok(())
    })
}
