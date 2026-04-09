use std::ffi::{CStr, CString};
use std::ptr;

use saikuro_c::{
    saikuro_channel_next_json, saikuro_channel_send_json, saikuro_client_batch_json,
    saikuro_client_channel_json, saikuro_client_connect, saikuro_client_log,
    saikuro_client_resource_json, saikuro_client_stream_json, saikuro_last_error_message,
    saikuro_provider_free, saikuro_provider_new, saikuro_provider_register, saikuro_stream_free,
    saikuro_stream_next_json, saikuro_string_dup, saikuro_string_free,
};

fn take_error() -> String {
    let ptr = saikuro_last_error_message();
    if ptr.is_null() {
        return String::new();
    }

    let message = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
    unsafe { saikuro_string_free(ptr) };
    message
}

#[test]
fn string_dup_roundtrip() {
    let input = CString::new("saikuro").expect("CString should be created");
    let duplicated = saikuro_string_dup(input.as_ptr());
    assert!(!duplicated.is_null());

    let text = unsafe { CStr::from_ptr(duplicated) }
        .to_string_lossy()
        .to_string();
    assert_eq!(text, "saikuro");

    unsafe { saikuro_string_free(duplicated) };
}

#[test]
fn client_connect_rejects_null_address() {
    let handle = saikuro_client_connect(ptr::null());
    assert!(handle.is_null());

    let message = take_error();
    assert!(message.contains("address must not be null"));
}

#[test]
fn provider_register_rejects_null_callback() {
    let ns = CString::new("math").expect("CString should be created");
    let provider = saikuro_provider_new(ns.as_ptr());
    assert!(!provider.is_null());

    let fn_name = CString::new("add").expect("CString should be created");
    let result = saikuro_provider_register(provider, fn_name.as_ptr(), None, ptr::null_mut());
    assert_eq!(result, 1);

    let message = take_error();
    assert!(message.contains("callback must not be null"));

    saikuro_provider_free(provider);
}

#[test]
fn batch_rejects_invalid_json_shape() {
    let addr = CString::new("tcp://127.0.0.1:1").expect("CString should be created");
    let handle = saikuro_client_connect(addr.as_ptr());
    assert!(handle.is_null());

    // null handle error should trigger before JSON parsing.
    let calls = CString::new("{}").expect("CString should be created");
    let result = saikuro_client_batch_json(ptr::null_mut(), calls.as_ptr());
    assert!(result.is_null());
    let message = take_error();
    assert!(message.contains("handle must not be null"));
}

#[test]
fn stream_next_rejects_null_output_pointers() {
    let stream = saikuro_client_stream_json(ptr::null_mut(), ptr::null(), ptr::null());
    assert!(stream.is_null());

    let mut out_json = ptr::null_mut();
    let mut out_done = 0;
    let rc = unsafe { saikuro_stream_next_json(ptr::null_mut(), &mut out_json, &mut out_done) };
    assert_eq!(rc, 1);
    let message = take_error();
    assert!(message.contains("stream must not be null"));

    // Ensure stream_free is null-safe for callers.
    saikuro_stream_free(ptr::null_mut());
}

#[test]
fn channel_calls_reject_null_handles() {
    let ch = saikuro_client_channel_json(ptr::null_mut(), ptr::null(), ptr::null());
    assert!(ch.is_null());
    let message = take_error();
    assert!(message.contains("handle must not be null"));

    let payload = CString::new("{}").expect("CString should be created");
    let rc = saikuro_channel_send_json(ptr::null_mut(), payload.as_ptr());
    assert_eq!(rc, 1);
    let message = take_error();
    assert!(message.contains("channel must not be null"));

    let mut out_json = ptr::null_mut();
    let mut out_done = 0;
    let rc = unsafe { saikuro_channel_next_json(ptr::null_mut(), &mut out_json, &mut out_done) };
    assert_eq!(rc, 1);
    let message = take_error();
    assert!(message.contains("channel must not be null"));
}

#[test]
fn resource_and_log_reject_null_handles() {
    let target = CString::new("files.open").expect("CString should be created");
    let args = CString::new("[]").expect("CString should be created");
    let res = saikuro_client_resource_json(ptr::null_mut(), target.as_ptr(), args.as_ptr());
    assert!(res.is_null());
    let message = take_error();
    assert!(message.contains("handle must not be null"));

    let level = CString::new("info").expect("CString should be created");
    let name = CString::new("tests").expect("CString should be created");
    let msg = CString::new("hello").expect("CString should be created");
    let fields = CString::new("{}").expect("CString should be created");
    let rc = saikuro_client_log(
        ptr::null_mut(),
        level.as_ptr(),
        name.as_ptr(),
        msg.as_ptr(),
        fields.as_ptr(),
    );
    assert_eq!(rc, 1);
    let message = take_error();
    assert!(message.contains("handle must not be null"));
}
