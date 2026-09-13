use std::net::SocketAddr;
use std::ptr;
use std::thread;
use std::time::Duration;

use saikuro_c::{
    saikuro_channel_close_async, saikuro_channel_next_json_async, saikuro_channel_send_json_async,
    saikuro_client_call_json_async, saikuro_client_call_json_timeout_async,
    saikuro_client_channel_json_async, saikuro_client_close_async, saikuro_client_connect_async,
    saikuro_client_free, saikuro_client_log_async, saikuro_client_resource_json_async,
    saikuro_client_stream_json_async, saikuro_provider_free, saikuro_provider_new,
    saikuro_provider_register, saikuro_provider_serve_async, saikuro_stream_next_json_async,
    saikuro_string_dup,
};
use saikuro_core::{
    envelope::{Envelope, InvocationType},
    ResponseEnvelope,
};
use saikuro_event::{ErrorCode, ErrorDetail, Value};
use saikuro_transport::tcp::TcpTransportListener;
use saikuro_transport::{Transport, TransportListener, TransportReceiver, TransportSender};

mod common;

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
        let rt = saikuro_exec::RuntimeBuilder::new_current_thread()
            .enable_all()
            .build();

        rt.block_on(async move {
            let socket = SocketAddr::from(([127, 0, 0, 1], 0));
            let mut listener =
                TcpTransportListener::bind(socket, std::sync::Arc::new(saikuro_event::NullSink))
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
                let frame = match saikuro_exec::timeout(Duration::from_secs(2), rx.recv()).await {
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
                        if matches!(env.stream_control, Some(saikuro_core::StreamControl::End)) {
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

    // Connect.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_void>();
    saikuro_client_connect_async(
        common::c(&address).as_ptr(),
        Some(common::connect_cb),
        user_data,
    );
    let handle = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(
        !handle.is_null(),
        "connect failed: {}",
        common::take_error()
    );

    // Resource.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_resource_json_async(
        handle,
        common::c("files.read").as_ptr(),
        common::c("[]").as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let resource = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(
        !resource.is_null(),
        "resource failed: {}",
        common::take_error()
    );
    assert_eq!(common::take_c_string(resource), "\"contents\"");

    // Log.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_client_log_async(
        handle,
        common::c("info").as_ptr(),
        common::c("tests").as_ptr(),
        common::c("hello").as_ptr(),
        common::c("{}").as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let log_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(log_rc, 0, "log failed: {}", common::take_error());

    // Stream open.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_void>();
    saikuro_client_stream_json_async(
        handle,
        common::c("events.watch").as_ptr(),
        common::c("[]").as_ptr(),
        Some(common::connect_cb),
        user_data,
    );
    let stream = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(
        !stream.is_null(),
        "stream open failed: {}",
        common::take_error()
    );

    // Stream next (item 1).
    let (rx, user_data) = common::channel_pair::<(*mut std::ffi::c_char, std::ffi::c_int)>();
    unsafe {
        saikuro_stream_next_json_async(stream, Some(common::item_cb), user_data);
    }
    let (out_json, out_done) = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(out_done, 0);
    assert_eq!(common::take_c_string(out_json), "1");

    // Stream next (item 2).
    let (rx, user_data) = common::channel_pair::<(*mut std::ffi::c_char, std::ffi::c_int)>();
    unsafe {
        saikuro_stream_next_json_async(stream, Some(common::item_cb), user_data);
    }
    let (out_json, out_done) = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(out_done, 0);
    assert_eq!(common::take_c_string(out_json), "2");

    // Stream next (done).
    let (rx, user_data) = common::channel_pair::<(*mut std::ffi::c_char, std::ffi::c_int)>();
    unsafe {
        saikuro_stream_next_json_async(stream, Some(common::item_cb), user_data);
    }
    let (_out_json, out_done) = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(out_done, 1);

    // Channel open.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_void>();
    saikuro_client_channel_json_async(
        handle,
        common::c("chat.open").as_ptr(),
        common::c("[]").as_ptr(),
        Some(common::connect_cb),
        user_data,
    );
    let channel = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(
        !channel.is_null(),
        "channel open failed: {}",
        common::take_error()
    );

    // Channel next (welcome).
    let (rx, user_data) = common::channel_pair::<(*mut std::ffi::c_char, std::ffi::c_int)>();
    unsafe {
        saikuro_channel_next_json_async(channel, Some(common::item_cb), user_data);
    }
    let (out_json, out_done) = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(out_done, 0);
    assert_eq!(common::take_c_string(out_json), "\"welcome\"");

    // Channel send.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_channel_send_json_async(
        channel,
        common::c("\"ping\"").as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let send_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(send_rc, 0, "channel send failed: {}", common::take_error());

    // Channel next (pong).
    let (rx, user_data) = common::channel_pair::<(*mut std::ffi::c_char, std::ffi::c_int)>();
    unsafe {
        saikuro_channel_next_json_async(channel, Some(common::item_cb), user_data);
    }
    let (out_json, out_done) = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(out_done, 0);
    assert_eq!(common::take_c_string(out_json), "\"pong\"");

    // Channel close.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_channel_close_async(channel, Some(common::status_cb), user_data);
    let close_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(
        close_rc,
        0,
        "channel close failed: {}",
        common::take_error()
    );

    // Call (error path).
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_call_json_async(
        handle,
        common::c("math.fail").as_ptr(),
        common::c("[]").as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let call_fail = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(call_fail.is_null(), "call should fail");
    let call_error = common::take_error();
    assert!(
        call_error.contains("ProviderError") || call_error.contains("boom"),
        "unexpected error mapping: {call_error}"
    );

    // Call (timeout path).
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_call_json_timeout_async(
        handle,
        common::c("slow.never").as_ptr(),
        common::c("[]").as_ptr(),
        30,
        Some(common::result_cb),
        user_data,
    );
    let timeout = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(timeout.is_null(), "timeout call should fail");
    let timeout_error = common::take_error();
    assert!(
        timeout_error.contains("timed out") || timeout_error.contains("Timeout"),
        "unexpected timeout error: {timeout_error}"
    );

    // Close client.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_client_close_async(handle, Some(common::status_cb), user_data);
    let client_close_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(
        client_close_rc,
        0,
        "client close failed: {}",
        common::take_error()
    );
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
    let result = common::c("42");
    saikuro_string_dup(result.as_ptr())
}

fn spawn_scripted_server_for_provider() -> (String, thread::JoinHandle<ScriptReport>) {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        let rt = saikuro_exec::RuntimeBuilder::new_current_thread()
            .enable_all()
            .build();

        rt.block_on(async move {
            let socket = SocketAddr::from(([127, 0, 0, 1], 0));
            let mut listener =
                TcpTransportListener::bind(socket, std::sync::Arc::new(saikuro_event::NullSink))
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

            let announce_frame = saikuro_exec::timeout(Duration::from_secs(5), rx.recv())
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

            let call = Envelope::call("math.add", vec![Value::Int(20), Value::Int(22)])
                .expect("entropy available");
            tx.send(call.to_msgpack().expect("encode call").into())
                .await
                .expect("send call");

            let response_frame = saikuro_exec::timeout(Duration::from_secs(5), rx.recv())
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

    let provider = saikuro_provider_new(common::c("math").as_ptr());
    assert!(
        !provider.is_null(),
        "provider create failed: {}",
        common::take_error()
    );

    let register_rc = saikuro_provider_register(
        provider,
        common::c("add").as_ptr(),
        Some(add_cb),
        ptr::null_mut(),
    );
    assert_eq!(
        register_rc,
        0,
        "provider register failed: {}",
        common::take_error()
    );

    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_provider_serve_async(
        provider,
        common::c(&address).as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let serve_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(
        serve_rc,
        0,
        "provider serve failed: {}",
        common::take_error()
    );

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
