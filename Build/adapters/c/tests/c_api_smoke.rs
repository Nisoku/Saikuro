use std::ffi::{c_void, CStr, CString};
use std::ptr;

use saikuro_c::{
    saikuro_channel_next_json_async, saikuro_channel_send_json_async,
    saikuro_client_batch_json_async, saikuro_client_channel_json_async,
    saikuro_client_connect_async, saikuro_client_log_async, saikuro_client_resource_json_async,
    saikuro_client_stream_json_async, saikuro_provider_free, saikuro_provider_new,
    saikuro_provider_register, saikuro_stream_free, saikuro_stream_next_json_async,
    saikuro_string_dup, saikuro_string_free,
};

mod common;

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
    let _lock = common::LAST_ERROR_LOCK.lock().expect("lock poisoned");
    saikuro_client_connect_async(ptr::null(), Some(common::noop_connect_cb), ptr::null_mut());
    assert!(common::take_error().contains("address must not be null"));
}

#[test]
fn provider_register_rejects_null_callback() {
    let _lock = common::LAST_ERROR_LOCK.lock().expect("lock poisoned");
    let ns = CString::new("math").expect("CString should be created");
    let provider = saikuro_provider_new(ns.as_ptr());
    assert!(!provider.is_null());

    let fn_name = CString::new("add").expect("CString should be created");
    let result = saikuro_provider_register(provider, fn_name.as_ptr(), None, ptr::null_mut());
    assert_eq!(result, 1);

    let message = common::take_error();
    assert!(message.contains("callback must not be null"));

    saikuro_provider_free(provider);
}

#[test]
fn batch_rejects_null_handle() {
    let _lock = common::LAST_ERROR_LOCK.lock().expect("lock poisoned");
    let calls = CString::new("{}").expect("CString should be created");
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_batch_json_async(
        ptr::null_mut(),
        calls.as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let result = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(result.is_null());
    assert!(common::take_error().contains("handle must not be null"));
}

#[test]
fn stream_rejects_null_stream_handle() {
    let _lock = common::LAST_ERROR_LOCK.lock().expect("lock poisoned");
    // Null client handle on open.
    let (rx, user_data) = common::channel_pair::<*mut c_void>();
    saikuro_client_stream_json_async(
        ptr::null_mut(),
        ptr::null(),
        ptr::null(),
        Some(common::connect_cb),
        user_data,
    );
    let stream = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(stream.is_null());
    assert!(common::take_error().contains("handle must not be null"));

    // Null stream handle on next — validated synchronously, callback not called.
    unsafe {
        saikuro_stream_next_json_async(ptr::null_mut(), Some(common::noop_item_cb), ptr::null_mut())
    };
    assert!(common::take_error().contains("stream must not be null"));

    // stream_free is null-safe.
    saikuro_stream_free(ptr::null_mut());
}

#[test]
fn channel_calls_reject_null_handles() {
    let _lock = common::LAST_ERROR_LOCK.lock().expect("lock poisoned");
    // Null client handle on channel open.
    let (rx, user_data) = common::channel_pair::<*mut c_void>();
    saikuro_client_channel_json_async(
        ptr::null_mut(),
        ptr::null(),
        ptr::null(),
        Some(common::connect_cb),
        user_data,
    );
    let ch = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(ch.is_null());
    assert!(common::take_error().contains("handle must not be null"));

    // Null channel on send.
    let payload = CString::new("{}").expect("CString should be created");
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_channel_send_json_async(
        ptr::null_mut(),
        payload.as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(rc, 1);
    assert!(common::take_error().contains("channel must not be null"));

    // Null channel on next — validated synchronously.
    unsafe {
        saikuro_channel_next_json_async(
            ptr::null_mut(),
            Some(common::noop_item_cb),
            ptr::null_mut(),
        )
    };
    assert!(common::take_error().contains("channel must not be null"));
}

#[test]
fn resource_and_log_reject_null_handles() {
    let _lock = common::LAST_ERROR_LOCK.lock().expect("lock poisoned");
    let target = CString::new("files.open").expect("CString should be created");
    let args = CString::new("[]").expect("CString should be created");

    // Null client handle on resource.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_resource_json_async(
        ptr::null_mut(),
        target.as_ptr(),
        args.as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let res = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(res.is_null());
    assert!(common::take_error().contains("handle must not be null"));

    // Null client handle on log.
    let level = CString::new("info").expect("CString should be created");
    let name = CString::new("tests").expect("CString should be created");
    let msg = CString::new("hello").expect("CString should be created");
    let fields = CString::new("{}").expect("CString should be created");
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_client_log_async(
        ptr::null_mut(),
        level.as_ptr(),
        name.as_ptr(),
        msg.as_ptr(),
        fields.as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(rc, 1);
    assert!(common::take_error().contains("handle must not be null"));
}
