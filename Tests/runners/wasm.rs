//! Wasm runner (`wasm32-unknown-unknown`).

#[expect(unused)]
#[path = "../tests/wasm/mod.rs"]
pub mod wasm;

use wasm_bindgen_test::wasm_bindgen_test_configure;

wasm_bindgen_test_configure!(run_in_browser);

mod wasm_critical_section {
    struct NoopCriticalSection;

    critical_section::set_impl!(NoopCriticalSection);

    // SAFETY: single-threaded wasm, no interrupts or preemption points.
    unsafe impl critical_section::Impl for NoopCriticalSection {
        unsafe fn acquire() {}

        unsafe fn release(_token: ()) {}
    }
}
