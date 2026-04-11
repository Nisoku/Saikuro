use std::ffi::{CStr, CString};
use std::net::SocketAddr;
use std::ptr;
use std::thread;
use std::time::Duration;

use saikuro_c::{
    saikuro_channel_close, saikuro_channel_next_json, saikuro_channel_send_json,
    saikuro_client_call_json, saikuro_client_call_json_timeout, saikuro_client_channel_json,
    saikuro_client_close, saikuro_client_connect, saikuro_client_free, saikuro_client_log,
    saikuro_client_resource_json, saikuro_client_stream_json, saikuro_last_error_message,
    saikuro_provider_free, saikuro_provider_new, saikuro_provider_register, saikuro_provider_serve,
    saikuro_stream_next_json, saikuro_string_dup, saikuro_string_free,
};
use saikuro_core::{
    envelope::{Envelope, InvocationType},
    error::{ErrorCode, ErrorDetail},
    value::Value,
    ResponseEnvelope,
};
use saikuro_transport::tcp::TcpTransportListener;
use saikuro_transport::traits::{Transport, TransportListener, TransportReceiver, TransportSender};

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

fn take_error() -> String {
    take_c_string(saikuro_last_error_message())
}

#[derive(Default)]
struct ScriptReport {
    saw_log: bool,
    saw_channel_close: bool,
    saw_announce: bool,
    saw_provider_response: bool,
}

fn spawn_scripted_server_for_client() -> (String, thread::JoinHandle<ScriptReport>) {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create runtime");

        rt.block_on(async move {
            let socket = SocketAddr::from(([127, 0, 0, 1], 0));
            let mut listener = TcpTransportListener::bind(socket)
                .await
                .expect("bind listener");
            let _ = ready_tx.send(format!("tcp://{}", listener.local_addr()));
            let transport = listener
                .accept()
                .await
                .expect("accept result")
                .expect("accepted transport");
            let (mut tx, mut rx) = transport.split();

            let mut report = ScriptReport::default();
            let mut open_channel_id = None;

            loop {
                let frame = match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
                    Ok(Ok(Some(frame))) => frame,
                    _ => break,
                };

                let env = match Envelope::from_msgpack(&frame) {
                    Ok(env) => env,
                    Err(_) => continue,
                };

                match env.invocation_type {
                    InvocationType::Resource => {
                        let resp = ResponseEnvelope::ok(env.id, Value::String("contents".into()));
                        let payload = resp.to_msgpack().expect("encode resource resp");
                        tx.send(payload.into()).await.expect("send resource resp");
                    }
                    InvocationType::Log => {
                        report.saw_log = true;
                    }
                    InvocationType::Stream => {
                        let a = ResponseEnvelope::stream_item(env.id, 0, Value::Int(1));
                        tx.send(a.to_msgpack().expect("encode stream item 1").into())
                            .await
                            .expect("send stream item 1");
                        let b = ResponseEnvelope::stream_item(env.id, 1, Value::Int(2));
                        tx.send(b.to_msgpack().expect("encode stream item 2").into())
                            .await
                            .expect("send stream item 2");
                        let end = ResponseEnvelope::stream_end(env.id, 2);
                        tx.send(end.to_msgpack().expect("encode stream end").into())
                            .await
                            .expect("send stream end");
                    }
                    InvocationType::Channel => {
                        if matches!(
                            env.stream_control,
                            Some(saikuro_core::envelope::StreamControl::End)
                        ) {
                            report.saw_channel_close = true;
                            continue;
                        }

                        if open_channel_id.is_none() && env.target == "chat.open" {
                            open_channel_id = Some(env.id);
                            let welcome = ResponseEnvelope::stream_item(
                                env.id,
                                0,
                                Value::String("welcome".into()),
                            );
                            tx.send(welcome.to_msgpack().expect("encode welcome").into())
                                .await
                                .expect("send welcome");
                        } else if Some(env.id) == open_channel_id {
                            let item = ResponseEnvelope::stream_item(
                                env.id,
                                1,
                                Value::String("pong".into()),
                            );
                            tx.send(item.to_msgpack().expect("encode pong").into())
                                .await
                                .expect("send pong");
                        }
                    }
                    InvocationType::Call if env.target == "math.fail" => {
                        let err = ErrorDetail::new(ErrorCode::ProviderError, "boom");
                        let resp = ResponseEnvelope::err(env.id, err);
                        tx.send(resp.to_msgpack().expect("encode call err").into())
                            .await
                            .expect("send call err");
                    }
                    InvocationType::Call if env.target == "slow.never" => {
                        // Intentionally do not respond to trigger client timeout.
                    }
                    _ => {}
                }
            }

            report
        })
    });

    let address = ready_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("client scripted server did not become ready");

    (address, handle)
}

#[test]
fn c_client_protocol_paths_cover_stream_channel_resource_log_error_and_timeout() {
    let (address, server) = spawn_scripted_server_for_client();

    let handle = saikuro_client_connect(c(&address).as_ptr());
    assert!(!handle.is_null(), "connect failed: {}", take_error());

    let resource = saikuro_client_resource_json(handle, c("files.read").as_ptr(), c("[]").as_ptr());
    assert!(!resource.is_null(), "resource failed: {}", take_error());
    assert_eq!(take_c_string(resource), "\"contents\"");

    let log_rc = saikuro_client_log(
        handle,
        c("info").as_ptr(),
        c("tests").as_ptr(),
        c("hello").as_ptr(),
        c("{}").as_ptr(),
    );
    assert_eq!(log_rc, 0, "log failed: {}", take_error());

    let stream = saikuro_client_stream_json(handle, c("events.watch").as_ptr(), c("[]").as_ptr());
    assert!(!stream.is_null(), "stream open failed: {}", take_error());

    let mut out_json = ptr::null_mut();
    let mut out_done = 0;
    let rc = unsafe { saikuro_stream_next_json(stream, &mut out_json, &mut out_done) };
    assert_eq!(rc, 0);
    assert_eq!(out_done, 0);
    assert_eq!(take_c_string(out_json), "1");

    let rc = unsafe { saikuro_stream_next_json(stream, &mut out_json, &mut out_done) };
    assert_eq!(rc, 0);
    assert_eq!(out_done, 0);
    assert_eq!(take_c_string(out_json), "2");

    let rc = unsafe { saikuro_stream_next_json(stream, &mut out_json, &mut out_done) };
    assert_eq!(rc, 0);
    assert_eq!(out_done, 1);

    let channel = saikuro_client_channel_json(handle, c("chat.open").as_ptr(), c("[]").as_ptr());
    assert!(!channel.is_null(), "channel open failed: {}", take_error());

    let rc = unsafe { saikuro_channel_next_json(channel, &mut out_json, &mut out_done) };
    assert_eq!(rc, 0);
    assert_eq!(out_done, 0);
    assert_eq!(take_c_string(out_json), "\"welcome\"");

    let send_rc = saikuro_channel_send_json(channel, c("\"ping\"").as_ptr());
    assert_eq!(send_rc, 0, "channel send failed: {}", take_error());

    let rc = unsafe { saikuro_channel_next_json(channel, &mut out_json, &mut out_done) };
    assert_eq!(rc, 0);
    assert_eq!(out_done, 0);
    assert_eq!(take_c_string(out_json), "\"pong\"");

    let close_rc = saikuro_channel_close(channel);
    assert_eq!(close_rc, 0, "channel close failed: {}", take_error());

    let call_fail = saikuro_client_call_json(handle, c("math.fail").as_ptr(), c("[]").as_ptr());
    assert!(call_fail.is_null(), "call should fail");
    let call_error = take_error();
    assert!(
        call_error.contains("ProviderError") || call_error.contains("boom"),
        "unexpected error mapping: {call_error}"
    );

    let timeout =
        saikuro_client_call_json_timeout(handle, c("slow.never").as_ptr(), c("[]").as_ptr(), 30);
    assert!(timeout.is_null(), "timeout call should fail");
    let timeout_error = take_error();
    assert!(
        timeout_error.contains("timed out") || timeout_error.contains("Timeout"),
        "unexpected timeout error: {timeout_error}"
    );

    let client_close_rc = saikuro_client_close(handle);
    assert_eq!(client_close_rc, 0, "client close failed: {}", take_error());
    saikuro_client_free(handle);

    let report = server.join().expect("server thread");
    assert!(report.saw_log, "server should receive log envelope");
    assert!(
        report.saw_channel_close,
        "server should receive channel close (cancellation) envelope"
    );
}

unsafe extern "C" fn add_cb(
    _user_data: *mut std::ffi::c_void,
    _args_json: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    let result = c("42");
    saikuro_string_dup(result.as_ptr())
}

fn spawn_scripted_server_for_provider() -> (String, thread::JoinHandle<ScriptReport>) {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create runtime");

        rt.block_on(async move {
            let socket = SocketAddr::from(([127, 0, 0, 1], 0));
            let mut listener = TcpTransportListener::bind(socket)
                .await
                .expect("bind listener");
            let _ = ready_tx.send(format!("tcp://{}", listener.local_addr()));
            let transport = listener
                .accept()
                .await
                .expect("accept result")
                .expect("accepted transport");
            let (mut tx, mut rx) = transport.split();

            let mut report = ScriptReport::default();

            let announce_frame = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("timed out waiting for announce frame")
                .expect("announce recv result")
                .expect("announce frame");
            let announce = Envelope::from_msgpack(&announce_frame).expect("decode announce");
            assert_eq!(announce.invocation_type, InvocationType::Announce);
            report.saw_announce = true;

            let ack = ResponseEnvelope::ok_empty(announce.id);
            tx.send(ack.to_msgpack().expect("encode ack").into())
                .await
                .expect("send announce ack");

            let call = Envelope::call("math.add", vec![Value::Int(20), Value::Int(22)]);
            tx.send(call.to_msgpack().expect("encode call").into())
                .await
                .expect("send call");

            let response_frame = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("timed out waiting for provider response frame")
                .expect("response recv result")
                .expect("response frame");
            let response =
                ResponseEnvelope::from_msgpack(&response_frame).expect("decode response");
            report.saw_provider_response = response.ok && response.result == Some(Value::Int(42));

            report
        })
    });

    let address = ready_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("provider scripted server did not become ready");

    (address, handle)
}

#[test]
fn c_provider_announce_and_runtime_dispatch_roundtrip() {
    let (address, server) = spawn_scripted_server_for_provider();

    let provider = saikuro_provider_new(c("math").as_ptr());
    assert!(
        !provider.is_null(),
        "provider create failed: {}",
        take_error()
    );

    let register_rc =
        saikuro_provider_register(provider, c("add").as_ptr(), Some(add_cb), ptr::null_mut());
    assert_eq!(register_rc, 0, "provider register failed: {}", take_error());

    let serve_rc = saikuro_provider_serve(provider, c(&address).as_ptr());
    assert_eq!(serve_rc, 0, "provider serve failed: {}", take_error());

    let report = server.join().expect("server thread");
    assert!(
        report.saw_announce,
        "provider should send announce handshake"
    );
    assert!(
        report.saw_provider_response,
        "provider should respond to runtime call with callback value"
    );

    saikuro_provider_free(provider);
}
