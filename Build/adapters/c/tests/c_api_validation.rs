use std::ffi::CString;
use std::ptr;

use saikuro_c::{
    saikuro_channel_abort, saikuro_channel_close, saikuro_channel_free, saikuro_channel_next_json,
    saikuro_channel_send_json, saikuro_client_batch_json, saikuro_client_call_json,
    saikuro_client_call_json_timeout, saikuro_client_cast_json, saikuro_client_channel_json,
    saikuro_client_connect, saikuro_client_log, saikuro_client_resource_json,
    saikuro_client_stream_json, saikuro_last_error_message, saikuro_provider_free,
    saikuro_provider_new, saikuro_provider_register, saikuro_stream_free, saikuro_stream_next_json,
    saikuro_string_dup, saikuro_string_free,
};

fn c(text: &str) -> CString {
    CString::new(text).expect("CString should be created")
}

fn take_error() -> String {
    let ptr = saikuro_last_error_message();
    if ptr.is_null() {
        return String::new();
    }

    let message = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .to_string();
    unsafe { saikuro_string_free(ptr) };
    message
}

#[test]
fn string_helpers_work_and_null_is_safe() {
    let duplicated = saikuro_string_dup(c("saikuro").as_ptr());
    assert!(!duplicated.is_null());

    let text = unsafe { std::ffi::CStr::from_ptr(duplicated) }
        .to_string_lossy()
        .to_string();
    assert_eq!(text, "saikuro");

    unsafe { saikuro_string_free(duplicated) };
    unsafe { saikuro_string_free(ptr::null_mut()) };
}

#[test]
fn client_connect_requires_non_null_address() {
    let handle = saikuro_client_connect(ptr::null());
    assert!(handle.is_null());

    let message = take_error();
    assert!(message.contains("address must not be null"));
}

#[test]
fn call_cast_batch_require_non_null_handle() {
    let target = c("math.add");
    let args = c("[1,2]");

    let call = saikuro_client_call_json(ptr::null_mut(), target.as_ptr(), args.as_ptr());
    assert!(call.is_null());
    assert!(take_error().contains("handle must not be null"));

    let cast = saikuro_client_cast_json(ptr::null_mut(), target.as_ptr(), args.as_ptr());
    assert_eq!(cast, 1);
    assert!(take_error().contains("handle must not be null"));

    let batch = saikuro_client_batch_json(ptr::null_mut(), c("[]").as_ptr());
    assert!(batch.is_null());
    assert!(take_error().contains("handle must not be null"));

    let timeout_call =
        saikuro_client_call_json_timeout(ptr::null_mut(), target.as_ptr(), args.as_ptr(), 100);
    assert!(timeout_call.is_null());
    assert!(take_error().contains("handle must not be null"));
}

#[test]
fn stream_and_channel_null_handle_paths_are_safe() {
    let stream = saikuro_client_stream_json(ptr::null_mut(), ptr::null(), ptr::null());
    assert!(stream.is_null());
    assert!(take_error().contains("handle must not be null"));

    let channel = saikuro_client_channel_json(ptr::null_mut(), ptr::null(), ptr::null());
    assert!(channel.is_null());
    assert!(take_error().contains("handle must not be null"));

    let mut out_json = ptr::null_mut();
    let mut out_done = 0;

    let stream_next =
        unsafe { saikuro_stream_next_json(ptr::null_mut(), &mut out_json, &mut out_done) };
    assert_eq!(stream_next, 1);
    assert!(take_error().contains("stream must not be null"));

    let channel_next =
        unsafe { saikuro_channel_next_json(ptr::null_mut(), &mut out_json, &mut out_done) };
    assert_eq!(channel_next, 1);
    assert!(take_error().contains("channel must not be null"));

    let send_rc = saikuro_channel_send_json(ptr::null_mut(), c("{}").as_ptr());
    assert_eq!(send_rc, 1);
    assert!(take_error().contains("channel must not be null"));

    let close_rc = saikuro_channel_close(ptr::null_mut());
    assert_eq!(close_rc, 1);
    assert!(take_error().contains("channel must not be null"));

    let abort_rc = saikuro_channel_abort(ptr::null_mut());
    assert_eq!(abort_rc, 1);
    assert!(take_error().contains("channel must not be null"));

    saikuro_stream_free(ptr::null_mut());
    saikuro_channel_free(ptr::null_mut());
}

#[test]
fn resource_and_log_require_non_null_handle() {
    let target = c("files.open");
    let args = c("[]");
    let resource = saikuro_client_resource_json(ptr::null_mut(), target.as_ptr(), args.as_ptr());
    assert!(resource.is_null());
    assert!(take_error().contains("handle must not be null"));

    let log_rc = saikuro_client_log(
        ptr::null_mut(),
        c("info").as_ptr(),
        c("tests").as_ptr(),
        c("hello").as_ptr(),
        c("{}").as_ptr(),
    );
    assert_eq!(log_rc, 1);
    assert!(take_error().contains("handle must not be null"));
}

unsafe extern "C" fn add_handler(
    _user_data: *mut std::ffi::c_void,
    args_json: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    if args_json.is_null() {
        return ptr::null_mut();
    }

    saikuro_string_dup(c("42").as_ptr())
}

#[test]
fn provider_registration_accepts_valid_callback() {
    let provider = saikuro_provider_new(c("math").as_ptr());
    assert!(!provider.is_null());

    let rc = saikuro_provider_register(
        provider,
        c("add").as_ptr(),
        Some(add_handler),
        ptr::null_mut(),
    );
    assert_eq!(rc, 0, "register should succeed: {}", take_error());

    saikuro_provider_free(provider);
}
