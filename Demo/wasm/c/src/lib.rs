use std::ffi::CString;
use wasm_bindgen::prelude::*;

// Ensure the saikuro-c rlib is linked so its C API symbols are available.
extern crate saikuro_c;

extern "C" {
    // linked from insight_c.c via build.rs
    #[link_name = "saikuro_c_start_provider"]
    fn c_start_provider(channel: *const std::ffi::c_char);
}

#[wasm_bindgen]
pub fn start_c_provider(channel: &str) {
    let c_channel = CString::new(channel).expect("channel contains null byte");
    unsafe { c_start_provider(c_channel.as_ptr()) }
}
