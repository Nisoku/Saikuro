use std::ffi::{CStr, CString};

use saikuro_c::{saikuro_last_error_message, saikuro_string_free};

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
