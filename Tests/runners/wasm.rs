//! Wasm runner (`wasm32-unknown-unknown`).
//!
//! This module provides a test runner for the wasm32-unknown-unknown target.
//! Node.js is the default execution target for wasm-bindgen-test; the same
//! binary also runs headlessly in a browser via `wasm-bindgen-test` with the
//! browser flag.

#[path = "../tests/wasm/mod.rs"]
pub mod wasm;

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