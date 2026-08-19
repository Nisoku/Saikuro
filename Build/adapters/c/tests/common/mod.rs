use std::ffi::{c_int, c_void, CStr, CString};
use std::sync::mpsc;
use std::time::Duration;

use saikuro_c::{saikuro_last_error_message, saikuro_string_free};

pub const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5);

pub fn c(text: &str) -> CString {
    CString::new(text).expect("CString should be created")
}

pub fn take_c_string(ptr: *mut std::ffi::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
    unsafe { saikuro_string_free(ptr) };
    text
}

pub fn take_error() -> String {
    take_c_string(saikuro_last_error_message())
}

pub fn channel_pair<T: 'static>() -> (mpsc::Receiver<T>, *mut c_void) {
    let (tx, rx) = mpsc::channel();
    let user_data = Box::into_raw(Box::new(tx)) as *mut c_void;
    (rx, user_data)
}

/// `SaikuroConnectCb = extern "C" fn(*mut c_void, *mut c_void)`
pub extern "C" fn connect_cb(handle: *mut c_void, user_data: *mut c_void) {
    let tx = unsafe { Box::from_raw(user_data as *mut mpsc::Sender<*mut c_void>) };
    tx.send(handle).ok();
}

/// `SaikuroResultCb = extern "C" fn(*mut c_char, *mut c_void)`
pub extern "C" fn result_cb(result: *mut std::ffi::c_char, user_data: *mut c_void) {
    let tx = unsafe { Box::from_raw(user_data as *mut mpsc::Sender<*mut std::ffi::c_char>) };
    tx.send(result).ok();
}

/// `SaikuroStatusCb = extern "C" fn(c_int, *mut c_void)`
pub extern "C" fn status_cb(status: c_int, user_data: *mut c_void) {
    let tx = unsafe { Box::from_raw(user_data as *mut mpsc::Sender<c_int>) };
    tx.send(status).ok();
}

/// `SaikuroItemCb = extern "C" fn(*mut c_char, c_int, *mut c_void)`
pub extern "C" fn item_cb(item: *mut std::ffi::c_char, done: c_int, user_data: *mut c_void) {
    let tx =
        unsafe { Box::from_raw(user_data as *mut mpsc::Sender<(*mut std::ffi::c_char, c_int)>) };
    tx.send((item, done)).ok();
}

/// Dummy no-op callbacks for tests that expect the callback to NOT fire
/// (e.g. synchronous validation errors that return before spawning).
pub extern "C" fn noop_connect_cb(_h: *mut c_void, _ud: *mut c_void) {}
pub extern "C" fn noop_result_cb(_r: *mut std::ffi::c_char, _ud: *mut c_void) {}
pub extern "C" fn noop_status_cb(_s: c_int, _ud: *mut c_void) {}
pub extern "C" fn noop_item_cb(_item: *mut std::ffi::c_char, _done: c_int, _ud: *mut c_void) {}
