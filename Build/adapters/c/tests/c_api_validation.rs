use std::ptr;

use saikuro_c::{
    saikuro_channel_abort_async, saikuro_channel_close_async, saikuro_channel_next_json_async,
    saikuro_channel_send_json_async, saikuro_client_batch_json_async,
    saikuro_client_call_json_async, saikuro_client_call_json_timeout_async,
    saikuro_client_cast_json_async, saikuro_client_channel_json_async,
    saikuro_client_connect_async, saikuro_client_log_async, saikuro_client_resource_json_async,
    saikuro_client_stream_json_async, saikuro_provider_free, saikuro_provider_new,
    saikuro_provider_register, saikuro_stream_free, saikuro_stream_next_json_async,
    saikuro_string_dup, saikuro_string_free,
};

mod common;

#[test]
fn string_helpers_work_and_null_is_safe() {
    let duplicated = saikuro_string_dup(common::c("saikuro").as_ptr());
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
    saikuro_client_connect_async(ptr::null(), Some(common::noop_connect_cb), ptr::null_mut());
    assert!(common::take_error().contains("address must not be null"));
}

#[test]
fn call_cast_batch_require_non_null_handle() {
    let target = common::c("math.add");
    let args = common::c("[1,2]");

    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_call_json_async(
        ptr::null_mut(),
        target.as_ptr(),
        args.as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let call = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(call.is_null());
    assert!(common::take_error().contains("handle must not be null"));

    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_client_cast_json_async(
        ptr::null_mut(),
        target.as_ptr(),
        args.as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let cast = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(cast, 1);
    assert!(common::take_error().contains("handle must not be null"));

    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_batch_json_async(
        ptr::null_mut(),
        common::c("[]").as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let batch = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(batch.is_null());
    assert!(common::take_error().contains("handle must not be null"));

    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_call_json_timeout_async(
        ptr::null_mut(),
        target.as_ptr(),
        args.as_ptr(),
        100,
        Some(common::result_cb),
        user_data,
    );
    let timeout_call = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(timeout_call.is_null());
    assert!(common::take_error().contains("handle must not be null"));
}

#[test]
fn stream_and_channel_null_handle_paths_are_safe() {
    // Null client handle on stream open.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_void>();
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

    // Null client handle on channel open.
    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_void>();
    saikuro_client_channel_json_async(
        ptr::null_mut(),
        ptr::null(),
        ptr::null(),
        Some(common::connect_cb),
        user_data,
    );
    let channel = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(channel.is_null());
    assert!(common::take_error().contains("handle must not be null"));

    // Null stream on next.
    unsafe {
        saikuro_stream_next_json_async(ptr::null_mut(), Some(common::noop_item_cb), ptr::null_mut())
    };
    assert!(common::take_error().contains("stream must not be null"));

    // Null channel on next.
    unsafe {
        saikuro_channel_next_json_async(
            ptr::null_mut(),
            Some(common::noop_item_cb),
            ptr::null_mut(),
        )
    };
    assert!(common::take_error().contains("channel must not be null"));

    // Null channel on send.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_channel_send_json_async(
        ptr::null_mut(),
        common::c("{}").as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let send_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(send_rc, 1);
    assert!(common::take_error().contains("channel must not be null"));

    // Null channel on close.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_channel_close_async(ptr::null_mut(), Some(common::status_cb), user_data);
    let close_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(close_rc, 1);
    assert!(common::take_error().contains("channel must not be null"));

    // Null channel on abort.
    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_channel_abort_async(ptr::null_mut(), Some(common::status_cb), user_data);
    let abort_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(abort_rc, 1);
    assert!(common::take_error().contains("channel must not be null"));

    saikuro_stream_free(ptr::null_mut());
}

#[test]
fn resource_and_log_require_non_null_handle() {
    let target = common::c("files.open");
    let args = common::c("[]");

    let (rx, user_data) = common::channel_pair::<*mut std::ffi::c_char>();
    saikuro_client_resource_json_async(
        ptr::null_mut(),
        target.as_ptr(),
        args.as_ptr(),
        Some(common::result_cb),
        user_data,
    );
    let resource = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert!(resource.is_null());
    assert!(common::take_error().contains("handle must not be null"));

    let (rx, user_data) = common::channel_pair::<std::ffi::c_int>();
    saikuro_client_log_async(
        ptr::null_mut(),
        common::c("info").as_ptr(),
        common::c("tests").as_ptr(),
        common::c("hello").as_ptr(),
        common::c("{}").as_ptr(),
        Some(common::status_cb),
        user_data,
    );
    let log_rc = rx.recv_timeout(common::CALLBACK_TIMEOUT).unwrap();
    assert_eq!(log_rc, 1);
    assert!(common::take_error().contains("handle must not be null"));
}

unsafe extern "C" fn add_handler(
    _user_data: *mut std::ffi::c_void,
    args_json: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    if args_json.is_null() {
        return ptr::null_mut();
    }

    saikuro_string_dup(common::c("42").as_ptr())
}

#[test]
fn provider_registration_accepts_valid_callback() {
    let provider = saikuro_provider_new(common::c("math").as_ptr());
    assert!(!provider.is_null());

    let rc = saikuro_provider_register(
        provider,
        common::c("add").as_ptr(),
        Some(add_handler),
        ptr::null_mut(),
    );
    assert_eq!(rc, 0, "register should succeed: {}", common::take_error());

    saikuro_provider_free(provider);
}
