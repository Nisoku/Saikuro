use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, Once};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use saikuro_core::{
    capability::CapabilitySet,
    envelope::{Envelope, InvocationType},
    schema::{
        ArgumentDescriptor, FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor,
        Visibility,
    },
    value::Value,
    ResponseEnvelope,
};
use saikuro_runtime::runtime::SaikuroRuntime;
use saikuro_transport::tcp::TcpTransportListener;
use saikuro_transport::traits::{Transport, TransportListener, TransportReceiver, TransportSender};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root")
}

fn build_target_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("resolve current test executable path");
    let exe_dir = exe.parent().expect("test executable directory");

    if exe_dir.file_name().and_then(|s| s.to_str()) == Some("deps") {
        return exe_dir
            .parent()
            .expect("target profile directory")
            .to_path_buf();
    }

    exe_dir.to_path_buf()
}

fn compile_cpp(source: &str, output: &Path) {
    ensure_saikuro_c_built();
    assert!(
        check_gpp_available(),
        "g++ is required for cpp_wrapper_runtime tests. Install g++ and ensure it is available on PATH"
    );

    let root = repo_root();
    let target = build_target_dir();

    let src_path = output.with_extension("cpp");
    fs::write(&src_path, source).expect("write cpp source");

    let mut cmd = Command::new("g++");
    cmd.arg("-std=c++17")
        .arg(&src_path)
        .arg("-I")
        .arg(root.join("Build/adapters/cpp/include"))
        .arg("-I")
        .arg(root.join("Build/adapters/c/include"))
        .arg("-L")
        .arg(&target);

    if cfg!(target_os = "linux") {
        cmd.arg(format!("-Wl,-rpath,{}", target.display()));
    } else if cfg!(target_os = "macos") {
        cmd.arg("-Wl,-rpath,@loader_path")
            .arg(format!("-Wl,-rpath,{}", target.display()));
    }

    let output = cmd
        .arg("-lsaikuro_c")
        .arg("-o")
        .arg(output)
        .output()
        .expect("invoke g++");

    assert!(
        output.status.success(),
        "g++ compile failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn check_gpp_available() -> bool {
    Command::new("g++")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn ensure_saikuro_c_built() {
    static BUILD_ONCE: Once = Once::new();
    BUILD_ONCE.call_once(|| {
        let build_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let status = Command::new("cargo")
            .current_dir(build_root)
            .arg("build")
            .arg("-p")
            .arg("saikuro-c")
            .status()
            .expect("run cargo build -p saikuro-c");
        assert!(
            status.success(),
            "failed to build saikuro-c library for C++ runtime tests"
        );
    });
}

fn run_cpp(exe: &Path, args: &[&str]) {
    let target = build_target_dir();
    let mut cmd = Command::new(exe);
    cmd.args(args);

    let lib_path_var = if cfg!(windows) {
        "PATH"
    } else if cfg!(target_os = "macos") {
        "DYLD_LIBRARY_PATH"
    } else {
        "LD_LIBRARY_PATH"
    };
    let sep = if cfg!(windows) { ';' } else { ':' };

    let existing = std::env::var(lib_path_var).unwrap_or_default();
    let joined = if existing.is_empty() {
        target.display().to_string()
    } else {
        format!("{}{}{}", target.display(), sep, existing)
    };
    cmd.env(lib_path_var, joined);

    let mut child = cmd.spawn().expect("spawn compiled cpp test");
    let timeout = Duration::from_secs(30);
    let poll_interval = Duration::from_millis(20);
    let start = std::time::Instant::now();

    let timed_out = loop {
        if child.try_wait().expect("poll compiled cpp test").is_some() {
            break false;
        }
        if start.elapsed() >= timeout {
            break true;
        }
        thread::sleep(poll_interval);
    };

    if timed_out {
        let _ = child.kill();
    }
    let output = child
        .wait_with_output()
        .expect("collect compiled cpp test output");

    assert!(
        !timed_out && output.status.success(),
        "cpp program failed{}:\nstdout:\n{}\nstderr:\n{}",
        if timed_out { " (timed out)" } else { "" },
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_probe_path(stem: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("saikuro_{stem}_{}_{}", std::process::id(), nanos))
}

fn spawn_runtime_for_cpp_client() -> (String, thread::JoinHandle<()>) {
    let (ready_tx, ready_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        let rt = saikuro_exec::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        rt.block_on(async move {
            let socket = SocketAddr::from(([127, 0, 0, 1], 0));
            let runtime = SaikuroRuntime::builder().build();
            let handle = runtime.handle();
            let (done_tx, done_rx) = saikuro_exec::oneshot::channel::<()>();
            let done_tx = Arc::new(Mutex::new(Some(done_tx)));
            let call_count = Arc::new(AtomicUsize::new(0));

            let mut functions = std::collections::HashMap::new();
            functions.insert(
                "add".to_owned(),
                FunctionSchema {
                    args: vec![
                        ArgumentDescriptor {
                            name: "a".to_owned(),
                            r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                            optional: false,
                            default: None,
                            doc: None,
                        },
                        ArgumentDescriptor {
                            name: "b".to_owned(),
                            r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                            optional: false,
                            default: None,
                            doc: None,
                        },
                    ],
                    returns: TypeDescriptor::primitive(PrimitiveType::Any),
                    visibility: Visibility::Public,
                    capabilities: vec![],
                    idempotent: false,
                    doc: None,
                },
            );
            functions.insert(
                "watch".to_owned(),
                FunctionSchema {
                    args: vec![],
                    returns: TypeDescriptor::Stream {
                        item: Box::new(TypeDescriptor::primitive(PrimitiveType::Any)),
                    },
                    visibility: Visibility::Public,
                    capabilities: vec![],
                    idempotent: false,
                    doc: None,
                },
            );
            let mut namespaces = std::collections::HashMap::new();
            namespaces.insert(
                "math".to_owned(),
                NamespaceSchema {
                    functions,
                    doc: None,
                },
            );
            let schema = Schema {
                version: 1,
                namespaces,
                types: std::collections::HashMap::new(),
            };
            handle
                .register_schema(schema, "cpp-runtime-provider")
                .expect("register schema");

            let done_tx_closure = done_tx.clone();
            let call_count_closure = call_count.clone();
            handle.register_fn_provider(
                "cpp-runtime-provider",
                vec!["math".to_owned()],
                move |env: Envelope| {
                    let done_tx_closure = done_tx_closure.clone();
                    let call_count_closure = call_count_closure.clone();
                    async move {
                        match env.target.as_str() {
                            "math.add" => {
                                let seen = call_count_closure.fetch_add(1, Ordering::Relaxed) + 1;
                                if seen >= 3 {
                                    if let Some(tx) = done_tx_closure
                                        .lock()
                                        .expect("done sender mutex poisoned")
                                        .take()
                                    {
                                        let _ = tx.send(());
                                    }
                                }
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
                    }
                },
            );

            let mut listener = TcpTransportListener::bind(socket)
                .await
                .expect("bind listener");
            let _ = ready_tx.send(format!("tcp://{}", listener.local_addr()));

            let transport = saikuro_exec::timeout(Duration::from_secs(60), listener.accept())
                .await
                .expect("timed out waiting for cpp client connection")
                .expect("accept")
                .expect("transport");
            handle.accept_transport(transport, "cpp-client".to_owned(), CapabilitySet::default());
            match saikuro_exec::timeout(Duration::from_secs(5), done_rx).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => panic!("oneshot was canceled before completion"),
                Err(_) => panic!("timed out waiting for cpp client to issue call"),
            }
        });
    });

    let address = ready_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("cpp runtime listener did not become ready");

    (address, handle)
}

fn spawn_scripted_runtime_for_cpp_provider() -> (String, thread::JoinHandle<bool>) {
    let (ready_tx, ready_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        let rt = saikuro_exec::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        rt.block_on(async move {
            let socket = SocketAddr::from(([127, 0, 0, 1], 0));
            let mut listener = TcpTransportListener::bind(socket)
                .await
                .expect("bind listener");
            let _ = ready_tx.send(format!("tcp://{}", listener.local_addr()));
            let transport =
                match saikuro_exec::timeout(Duration::from_secs(60), listener.accept()).await {
                    Ok(Ok(Some(transport))) => transport,
                    Ok(Ok(None)) => return false,
                    Ok(Err(_)) => return false,
                    Err(_) => return false,
                };
            let (mut tx, mut rx) = transport.split();

            let frame = match saikuro_exec::timeout(Duration::from_secs(5), rx.recv()).await {
                Ok(Ok(Some(frame))) => frame,
                _ => return false,
            };
            let announce = Envelope::from_msgpack(&frame).expect("decode announce");
            if announce.invocation_type != InvocationType::Announce {
                return false;
            }

            let ack = ResponseEnvelope::ok_empty(announce.id);
            tx.send(ack.to_msgpack().expect("encode ack").into())
                .await
                .expect("send ack");

            let call = Envelope::call("math.add", vec![Value::Int(20), Value::Int(22)]);
            tx.send(call.to_msgpack().expect("encode call").into())
                .await
                .expect("send call");

            let response = match saikuro_exec::timeout(Duration::from_secs(5), rx.recv()).await {
                Ok(Ok(Some(response))) => response,
                _ => return false,
            };
            let decoded = ResponseEnvelope::from_msgpack(&response).expect("decode response");

            decoded.ok && decoded.result == Some(Value::Int(42))
        })
    });

    let address = ready_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("scripted runtime listener did not become ready");

    (address, handle)
}

#[test]
fn cpp_wrapper_client_integrates_with_runtime() {
    let (address, runtime_thread) = spawn_runtime_for_cpp_client();

    let exe = temp_probe_path("cpp_client_runtime_test");

    let source = r#"
#include <saikuro/saikuro.hpp>
#include <cassert>
#include <string>

int main(int argc, char** argv) {
    if (argc < 2) return 2;
    std::string addr = argv[1];
    saikuro::Client client(addr);

    std::string v = client.call_json("math.add", "[20,22]");
    assert(v == "42");

    client.cast_json("math.add", "[1,2]");
    std::string b = client.batch_json("[{\"target\":\"math.add\",\"args\":[1,2]}]");
    assert(!b.empty());

    return 0;
}
"#;

    compile_cpp(source, &exe);
    run_cpp(&exe, &[&address]);

    let src_path = exe.with_extension("cpp");
    let _ = fs::remove_file(&src_path);
    let _ = fs::remove_file(&exe);

    runtime_thread.join().expect("runtime thread");
}

#[test]
fn cpp_wrapper_provider_announce_and_dispatch_with_runtime() {
    let (address, scripted_runtime) = spawn_scripted_runtime_for_cpp_provider();

    let exe = temp_probe_path("cpp_provider_runtime_test");

    let source = r#"
#include <saikuro/saikuro.hpp>
#include <cassert>
#include <cstdio>
#include <string>

extern "C" char* add_cb(void*, const char* args_json) {
    long long a = 0;
    long long b = 0;
    const char* raw = args_json == nullptr ? "[]" : args_json;
    if (std::sscanf(raw, " [ %lld , %lld ] ", &a, &b) != 2) {
        return saikuro_string_dup("0");
    }
    std::string sum = std::to_string(a + b);
    return saikuro_string_dup(sum.c_str());
}

int main(int argc, char** argv) {
    if (argc < 2) return 2;
    saikuro::Provider provider("math");
    provider.register_handler("add", add_cb, nullptr);
    provider.serve(argv[1]);
    return 0;
}
"#;

    compile_cpp(source, &exe);
    run_cpp(&exe, &[&address]);

    let src_path = exe.with_extension("cpp");
    let _ = fs::remove_file(&src_path);
    let _ = fs::remove_file(&exe);

    let ok = scripted_runtime.join().expect("scripted runtime");
    assert!(ok, "provider should announce and answer runtime call");
}
