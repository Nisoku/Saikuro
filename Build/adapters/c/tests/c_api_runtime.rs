use std::ffi::{CStr, CString};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use saikuro_c::{
    saikuro_client_batch_json, saikuro_client_call_json, saikuro_client_cast_json,
    saikuro_client_close, saikuro_client_connect, saikuro_client_free, saikuro_last_error_message,
    saikuro_string_free,
};
use saikuro_core::{
    capability::CapabilitySet,
    envelope::{Envelope, InvocationType},
    schema::{FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility},
    value::Value,
    ResponseEnvelope,
};
use saikuro_runtime::runtime::SaikuroRuntime;
use saikuro_transport::tcp::TcpTransportListener;
use saikuro_transport::traits::TransportListener;

fn c(text: &str) -> CString {
    CString::new(text).expect("CString should be created")
}

fn take_c_string(ptr: *mut std::ffi::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
    unsafe { saikuro_string_free(ptr) };
    text
}

fn take_last_error() -> String {
    take_c_string(saikuro_last_error_message())
}

fn make_schema(namespace: &str, function: &str, n_args: usize) -> Schema {
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

    let mut functions = std::collections::HashMap::new();
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

    let mut namespaces = std::collections::HashMap::new();
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
        types: std::collections::HashMap::new(),
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
            let rt = saikuro_exec::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("create test runtime");

            rt.block_on(async move {
                let socket = SocketAddr::from(([127, 0, 0, 1], 0));
                let runtime = Arc::new(SaikuroRuntime::builder().build());
                let handle = runtime.handle();

                let mut listener = TcpTransportListener::bind(socket)
                    .await
                    .expect("bind TCP listener");

                let schema = make_schema("math", "add", 2);
                runtime
                    .handle()
                    .register_schema(schema, "c-test-provider")
                    .expect("register schema");

                runtime.handle().register_fn_provider(
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
                );

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
                                        CapabilitySet::default(),
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

#[test]
fn c_client_call_cast_batch_roundtrip_with_runtime() {
    let runtime = RuntimeHarness::start();

    let address = c(&runtime.address);
    let handle = saikuro_client_connect(address.as_ptr());
    assert!(
        !handle.is_null(),
        "connect should succeed: {}",
        take_last_error()
    );

    let call_result =
        saikuro_client_call_json(handle, c("math.add").as_ptr(), c("[2, 40]").as_ptr());
    assert!(!call_result.is_null(), "call failed: {}", take_last_error());
    let call_json = take_c_string(call_result);
    assert_eq!(call_json, "42");

    let cast_rc = saikuro_client_cast_json(handle, c("math.add").as_ptr(), c("[5, 6]").as_ptr());
    assert_eq!(cast_rc, 0, "cast should succeed: {}", take_last_error());

    let batch_calls = c(r#"[
            {"target": "math.add", "args": [1, 2]},
            {"target": "math.add", "args": [3, 4]}
        ]"#);
    let batch_result = saikuro_client_batch_json(handle, batch_calls.as_ptr());
    assert!(
        !batch_result.is_null(),
        "batch failed: {}",
        take_last_error()
    );
    let batch_json = take_c_string(batch_result);
    assert_eq!(batch_json, "[3,7]");

    let close_rc = saikuro_client_close(handle);
    assert_eq!(close_rc, 0, "close should succeed: {}", take_last_error());
    saikuro_client_free(handle);
}

#[test]
fn c_client_reports_transport_error_when_namespace_missing() {
    let runtime = RuntimeHarness::start();

    let address = c(&runtime.address);
    let handle = saikuro_client_connect(address.as_ptr());
    assert!(!handle.is_null());

    let missing = saikuro_client_call_json(handle, c("missing.add").as_ptr(), c("[1, 1]").as_ptr());
    assert!(missing.is_null(), "unknown namespace call should fail");

    let message = take_last_error();
    assert!(
        message.contains("call failed") || message.contains("NoProvider"),
        "unexpected error message: {message}"
    );

    let close_rc = saikuro_client_close(handle);
    assert_eq!(close_rc, 0, "close should succeed: {}", take_last_error());
    saikuro_client_free(handle);
}
