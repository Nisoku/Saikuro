//! Wasm runner (`wasm32-unknown-unknown`).

#[path = "../tests/wasm/mod.rs"]
pub mod wasm;

// No `wasm_bindgen_test_configure!` call: Node.js is the default execution
// target for wasm-bindgen-test. The same binary also runs headlessly in a
// browser when driven through `wasm-pack test` with a browser flag.

mod wasm_critical_section {
    struct NoopCriticalSection;

    critical_section::set_impl!(NoopCriticalSection);

    // SAFETY: single-threaded wasm, no interrupts or preemption points.
    unsafe impl critical_section::Impl for NoopCriticalSection {
        unsafe fn acquire() -> critical_section::RawRestoreState {
            Default::default()
        }

        unsafe fn release(_restore_state: critical_section::RawRestoreState) {}
    }
}
