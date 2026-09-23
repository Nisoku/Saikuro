use alloc::boxed::Box;
use core::any::Any;
use core::future::Future;
use core::time::Duration;

pub use crate::base::join::JoinHandle;
use crate::base::queue::{queue, BoxedFuture, NOTIFY};
pub use crate::base::runtime::{new_runtime, Runtime, RuntimeBuilder};
pub use crate::base::spawn::{active_tasks, spawn};
use wasm_bindgen::prelude::*;

use super::jspi::{claim, ensure_executor, static_executor, OutputCapture};
use super::pump::{entering_poll, is_driving};

/// Upper bound on the `block_on` spin fallback before declaring the executor
/// wedged.
const BLOCK_ON_SPIN_TIMEOUT: Duration = Duration::from_secs(30);

/// JSPI-context probe: rewritten in-wasm by the wasm-bindgen CLI to
/// read `__jspi_stack_base` (non-zero only while a `#[wasm_bindgen(jspi)]`
/// frame is on the stack) and a constant `0` in modules without JSPI.
#[wasm_bindgen(raw_module = "__wbindgen_placeholder__")]
extern "C" {
    fn __wbindgen_jspi_in_context() -> u32;
}

/// Run `fut` on the shared executor, driven by the JS event loop.
pub fn run<F: Future + 'static>(fut: F) {
    ensure_executor();
    let boxed: BoxedFuture = Box::pin(async move {
        let _ = fut.await;
    });
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());
    super::pump::schedule();
}

/// Block on `fut` until it returns its output, using JSPI with a spin-loop
/// fallback.
///
/// **JSPI path** (real async I/O): if the caller is inside a
/// `#[wasm_bindgen(jspi)]` export, `jspi_block_on_promise` suspends the
/// wasm stack until the future completes. While suspended, the executor is
/// driven automatically by a pender-scheduled microtask pump..
///
/// **Spin fallback** (degraded, last resort): if JSPI is unavailable (not in
/// a JSPI context, or the suspension failed), falls back to a busy-poll loop
/// that drives the executor directly. Because wasm holds the CPU while
/// spinning, futures that wait on [`crate::time::sleep`]/[`crate::time::timeout`], 
/// or any JS event-loop event, cannot complete here, only internal work can.
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: Any,
{
    if is_driving() {
        panic!(
            "block_on: called from inside the executor (a task on this runtime is already \
             driving it); this would deadlock - restructure the task to `await` instead"
        );
    }
    ensure_executor();

    let (done_tx, done_rx) = futures::channel::oneshot::channel::<()>();

    // Per-call output cell, boxed so it lives on the heap.
    let mut cell: Box<Option<Box<dyn Any>>> = Box::new(None);
    let cell_ptr: *mut Option<Box<dyn Any>> = core::ptr::addr_of_mut!(*cell);

    let wrapped = async move {
        let _ = OutputCapture::new(fut, cell_ptr).await;
        let _ = done_tx.send(());
    };

    let boxed: BoxedFuture = Box::pin(wrapped);
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());
    super::pump::schedule();

    // Try JSPI first, but only from inside a promising-wrapped frame.
    let promise = wasm_bindgen_futures::future_to_promise(async {
        done_rx
            .await
            .map_err(|_| wasm_bindgen::JsValue::undefined())
            .map(|()| wasm_bindgen::JsValue::undefined())
    });

    let jspi_context = __wbindgen_jspi_in_context() != 0;
    if jspi_context {
        #[allow(deprecated)]
        if js_sys::futures::jspi_block_on_promise(&promise).is_ok() {
            return claim::<F::Output>(cell_ptr);
        }
    }

    // JSPI unavailable (or not in a JSPI context): spin-poll the raw executor
    // that ensure_executor() set up.
    let executor = static_executor();
    let start = super::time::now();
    loop {
        if unsafe { &mut *cell_ptr }.is_some() {
            break;
        }
        {
            let _guard = entering_poll();
            super::time::advance_clock();
            unsafe { executor.poll() };
        }
        core::hint::spin_loop();
        // Deadlock guard
        if super::time::now().saturating_duration_since(start) > BLOCK_ON_SPIN_TIMEOUT {
            panic!(
                "block_on: spin fallback made no progress after {}s \
                 ({} sleep{}) - futures waiting on JS timers/events cannot complete while \
                 wasm spins; run this call inside a JSPI export (COOP/COEP) instead",
                BLOCK_ON_SPIN_TIMEOUT.as_secs(),
                super::time::pending_sleeps(),
                if super::time::pending_sleeps() == 1 { "" } else { "s" }
            );
        }
    }

    claim::<F::Output>(cell_ptr)
}

/// Pump the shared executor once: advance pending sleeps, then run the
/// executor. Normally unnecessary (the `__pender` microtask pump does this
/// automatically); a root driver is only useful for custom run loops.
pub fn pump() {
    let _guard = entering_poll();
    super::time::advance_clock();
    unsafe { static_executor().poll() };
}
