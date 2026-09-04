#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

// Exactly one engine must be selected; `native` requires `std`; the `no_std`
// engine must not enable `std`.
#[cfg(all(feature = "native", feature = "no_std"))]
compile_error!("engine conflict: native and no_std are mutually exclusive");
#[cfg(all(feature = "native", feature = "wasm"))]
compile_error!("engine conflict: native and wasm are mutually exclusive");
#[cfg(all(feature = "native", feature = "embedded"))]
compile_error!("engine conflict: native and embedded are mutually exclusive");
#[cfg(all(feature = "no_std", feature = "wasm"))]
compile_error!("engine conflict: no_std and wasm are mutually exclusive");
#[cfg(all(feature = "no_std", feature = "embedded"))]
compile_error!("engine conflict: no_std and embedded are mutually exclusive");
#[cfg(all(feature = "wasm", feature = "embedded"))]
compile_error!("engine conflict: wasm and embedded are mutually exclusive");
#[cfg(not(any(
    feature = "native",
    feature = "no_std",
    feature = "wasm",
    feature = "embedded"
)))]
compile_error!("exactly one engine must be selected: native | no_std | wasm | embedded");
#[cfg(all(feature = "native", not(feature = "std")))]
compile_error!("native engine requires the std toolchain");

mod shared;
pub use shared::*;

// Wasm32-unknown-unknown cannot emit real atomic instructions (no `+atomics`
// target feature), so `portable-atomic` routes through `critical-section`.
#[cfg(target_arch = "wasm32")]
mod wasm_critical_section {
    struct NoopCriticalSection;

    critical_section::set_impl!(NoopCriticalSection);

    // SAFETY: wasm32 here has no shared memory, no threads, and no interrupt
    // sources, so acquire/release pairs cannot be interleaved.
    unsafe impl critical_section::Impl for NoopCriticalSection {
        unsafe fn acquire() -> critical_section::RawRestoreState {
            Default::default()
        }

        unsafe fn release(_restore_state: critical_section::RawRestoreState) {}
    }
}

#[cfg(feature = "embedded")]
pub mod embedded;
#[cfg(feature = "native")]
pub mod native;
#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "embedded")]
pub use embedded::*;
#[cfg(feature = "native")]
pub use native::*;
#[cfg(feature = "wasm")]
pub use wasm::*;

// The no_std (wasm / wasi) engine has no allocator from the toolchain, so the
// runtime must provide one.
#[cfg(all(
    not(feature = "std"),
    not(feature = "embedded"),
    target_family = "wasm"
))]
#[global_allocator]
static HEAP: talc::TalckWasm = unsafe { talc::TalckWasm::new_global() };

/// Prepare the no_std heap.
#[cfg(all(
    not(feature = "std"),
    not(feature = "embedded"),
    target_family = "wasm"
))]
pub fn init_heap() {}

#[cfg(all(
    feature = "default-panic-handler",
    not(feature = "std"),
    any(all(target_os = "wasi", feature = "wasi-preview1"), target_os = "none")
))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: terminating the wasm instance is the only valid action on
        // panic for an embedded-wasm target. `core::arch::wasm32::unreachable`
        // is an `unsafe fn` under bare wasm32 but a safe intrinsic under wasi,
        // so the inner `unsafe` block is only conditionally required.
        #[allow(unused_unsafe)]
        unsafe {
            core::arch::wasm32::unreachable();
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        loop {}
    }
}
