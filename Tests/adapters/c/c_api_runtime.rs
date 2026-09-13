use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use saikuro_c::{
    saikuro_client_batch_json_async, saikuro_client_call_json_async,
    saikuro_client_cast_json_async, saikuro_client_close_async, saikuro_client_connect_async,
    saikuro_client_free,
};
use saikuro_core::{
    envelope::{Envelope, InvocationType},
    schema::{
        FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema, PrimitiveType, Schema,
        TypeDescriptor, TypeMap, Visibility,
    },
    ResponseEnvelope,
};
use saikuro_event::Value;
use saikuro_runtime::SaikuroRuntime;
use saikuro_transport::tcp::TcpTransportListener;
use saikuro_transport::TransportListener;

mod common;

fn make_schema(namespace: &str, function: &str, n_args: usize) -> Schema {
    use saikuro_core::ArgumentDescriptor;

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
    functions
        .insert(
            function.to_owned(),
            FunctionSchema {
                args,
                returns: TypeDescriptor::primitive(PrimitiveType::Any),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        )
        .ok();

    let mut namespaces = NamespaceMap::new();
    namespaces
        .insert(
            namespace.to_owned(),
            NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        )
        .ok();

    Schema {
        version: 1,
        namespaces: Box::new(namespaces),
        types: Box::new(TypeMap::new()),
    }
}

struct RuntimeHarness {
    address: String,
    stop_flag: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl RuntimeHarness {
    fn start() -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_bg = stop_flag.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();

        let worker = thread::spawn(move || {
            let rt = saikuro_exec::RuntimeBuilder::new_current_thread()
                .enable_all()
                .build();

            rt.block_on(async move {
                let socket = SocketAddr::from(([127, 0, 0, 1], 0));
                let runtime = Arc::new(SaikuroRuntime::builder().build().await);
                let handle = runtime.handle();

                let mut listener = TcpTransportListener::bind(
                    socket,
                    std::sync::Arc::new(saikuro_event::NullSink),
                )
                .await
                .expect("bind TCP listener");

                let schema = make_schema("math", "add", 2);
                runtime
                    .handle()
                    .register_schema(schema, "c-test-provider")
                    .await
                    .expect("register schema");

                runtime
                    .handle()
                    .register_fn_provider(
                        "c-test-provider",
                        vec!["math".to_owned()],
                        |env: Envelope| async move {
                            match env.invocation_type {
                                InvocationType::Call | InvocationType::Cast => {
                                    let a = match env.args.first() {
                                        Some(Value::Int(v)) => *v,
                                        _ => 0,
                                    };
                                    let b = match env.args.get(1) {
                                        Some(Value::Int(v)) => *v,
                                        _ => 0,
                                    };
                                    ResponseEnvelope::ok(env.id, Value::Int(a + b))
                                }
                                _ => ResponseEnvelope::ok_empty(env.id),
                            }
                        },
                    )
                    .await;

                let _ = ready_tx.send(format!("tcp://{}", listener.local_addr()));
                let mut peer_counter: u64 = 0;
                loop {
                    saikuro_exec::select! {
                        result = listener.accept() => {
                            match result {
                                Ok(Some(transport)) => {
                                    peer_counter += 1;
                                    handle.accept_transport(
                                        transport,
                                        format!("c-test-peer-{peer_counter}"),
                                        saikuro_core::CapabilitySet::default(),
                                    );
                                }
                                Ok(None) => break,
                                Err(_) => break,
                            }
                        }
                        _ = saikuro_exec::sleep(Duration::from_millis(25)) => {
                            if stop_flag_bg.load(Ordering::Relaxed) {
                                break;
                            }
                        }
                    }
                }
            });
        });

        let address = ready_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("runtime did not become ready in time");

        Self {
            address,
            stop_flag,
            worker: Some(worker),
        }
    }
}

impl Drop for RuntimeHarness {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn shared_runtime() -> &'static RuntimeHarness {
    static RT: OnceLock<RuntimeHarness> = OnceLock::new();
    RT.get_or_init(|| RuntimeHarness::start())
}

#[test]
fn c_client_call_cast_batch_roundtrip_with_runtime() {
    let _lock = common::LAST_ERROR_LOCK.lock().expect("lock poisoned");
    let runtime = shared_runtime();

    // Connect.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_void>();
    saikuro_client_connect_async(
        common::c(&runtime.address).as_ptr(),
        Some(common::connect_cb),
        user_data,
    );
    let handle = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(
        !handle.is_null(),
        "connect should succeed: {}",
        common::take_error()
    );

    // Call.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_call_json_async(
        handle,
        common::c("math.add").as_ptr(),
        common::c("[2, 40]").as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let call_result = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(
        !call_result.is_null(),
        "call failed: {}",
        common::take_error()
    );
    let call_json = common::take_c_string(call_result);
    assert_eq!(call_json, "42");

    // Cast.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_client_cast_json_async(
        handle,
        common::c("math.add").as_ptr(),
        common::c("[5, 6]").as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let cast_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(cast_rc, 0, "cast should succeed: {}", common::take_error());

    // Batch.
    let batch_calls = common::c(
        r#"[
            {"target": "math.add", "args": [1, 2]},
            {"target": "math.add", "args": [3, 4]}
        ]"#,
    );
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_batch_json_async(
        handle,
        batch_calls.as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let batch_result = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(
        !batch_result.is_null(),
        "batch failed: {}",
        common::take_error()
    );
    let batch_json = common::take_c_string(batch_result);
    assert_eq!(batch_json, "[3,7]");

    // Close.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_client_close_async(handle, Some(common::status_cb), user_data);
    let close_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(
        close_rc,
        0,
        "close should succeed: {}",
        common::take_error()
    );
    saikuro_client_free(handle);
}

#[test]
fn c_client_reports_transport_error_when_namespace_missing() {
    let _lock = common::LAST_ERROR_LOCK.lock().expect("lock poisoned");
    let runtime = shared_runtime();

    // Connect.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_void>();
    saikuro_client_connect_async(
        common::c(&runtime.address).as_ptr(),
        Some(common::connect_cb),
        user_data,
    );
    let handle = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(!handle.is_null());

    // Call missing namespace.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_call_json_async(
        handle,
        common::c("missing.add").as_ptr(),
        common::c("[1, 1]").as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let missing = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(missing.is_null(), "unknown namespace call should fail");

    let message = common::take_error();
    assert!(
        message.contains("call failed") || message.contains("NoProvider"),
        "unexpected error message: {message}"
    );

    // Close.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_client_close_async(handle, Some(common::status_cb), user_data);
    let close_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(
        close_rc,
        0,
        "close should succeed: {}",
        common::take_error()
    );
    saikuro_client_free(handle);
}
