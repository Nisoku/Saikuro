use alloc::boxed::Box;
use core::any::Any;
use core::future::Future;
#[cfg(not(feature = "asyncify"))]
use core::time::Duration;

pub use crate::base::join::JoinHandle;
use crate::base::queue::{queue, BoxedFuture, NOTIFY};
pub use crate::base::runtime::{new_runtime, Runtime, RuntimeBuilder};
pub use crate::base::spawn::{active_tasks, spawn};
#[cfg(not(feature = "asyncify"))]
use wasm_bindgen::prelude::*;

use super::cell::{self, OutputCapture};
use super::pump::{entering_poll, is_driving};
use crate::base::block_on::{ensure_runner_started, nested_block_on_panic, static_executor};

/// Upper bound on the `block_on` spin fallback before declaring the executor
/// wedged.
#[cfg(not(feature = "asyncify"))]
const BLOCK_ON_SPIN_TIMEOUT: Duration = Duration::from_secs(30);

/// JSPI-context probe: rewritten in-wasm by the wasm-bindgen CLI to
/// read `__wbindgen_stack_base` (non-zero only while a `#[wasm_bindgen(jspi)]`
/// frame is on the stack) and a constant `0` in modules without JSPI.
#[cfg(not(feature = "asyncify"))]
#[wasm_bindgen(raw_module = "__wbindgen_placeholder__")]
extern "C" {
    fn __wbindgen_jspi_in_context() -> u32;
}

/// Run `fut` on the shared executor, driven by the JS event loop.
pub fn run<F: Future + 'static>(fut: F) {
    ensure_runner_started();
    let boxed: BoxedFuture = Box::pin(async move {
        let _ = fut.await;
    });
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());
    super::pump::schedule();
}

/// Run `fut` to completion on the shared executor, suspending the wasm stack
/// until the future stores its output in the shared cell.
///
/// **Asyncify** (`asyncify` feature): unwinds to the host and lets the JS event
/// loop run, so JS timers, `fetch` and DOM events all progress. This is the
/// fallback for hosts without JSPI.
///
/// **JSPI** (default): if the caller is inside a `#[wasm_bindgen(jspi)]` export,
/// `jspi_block_on_promise` suspends the wasm stack natively while the executor
/// is driven by the pender-scheduled microtask pump.
///
/// **Spin** (last resort, no `asyncify` and no JSPI context): busy-polls the
/// executor. `advance_clock` reads the real clock, so [`crate::time::sleep`] and
/// [`crate::time::timeout`] do complete, by burning CPU for their duration. What
/// cannot complete is anything needing the JS event loop itself, such as
/// `wasm_bindgen_futures` promises or DOM events: wasm holds the CPU, so those
/// callbacks never run.
///
/// Panics if called from inside a future already being driven.
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: Any,
{
    debug_assert!(
        !is_driving(),
        "block_on: reentered from inside the executor; the result cell is shared"
    );
    if is_driving() {
        nested_block_on_panic();
    }
    ensure_runner_started();

    cell::clear();
    let cell_ptr = cell::cell_ptr();

    // JSPI needs a promise to suspend on, which a oneshot provides.
    let (done_tx, done_rx) = futures::channel::oneshot::channel::<()>();
    let wrapped = async move {
        let _ = OutputCapture::new(fut, cell_ptr).await;
        let _ = done_tx.send(());
    };

    let boxed: BoxedFuture = Box::pin(wrapped);
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());
    super::pump::schedule();

    #[cfg(feature = "asyncify")]
    {
        // Asyncify polls the cell directly, so no promise is built and the
        // receiving half is dropped.
        drop(done_rx);
        super::asyncify::wait_until(cell::is_ready);
    }

    #[cfg(not(feature = "asyncify"))]
    {
        if __wbindgen_jspi_in_context() != 0 {
            let promise = wasm_bindgen_futures::future_to_promise(async {
                done_rx
                    .await
                    .map_err(|_| wasm_bindgen::JsValue::undefined())
                    .map(|()| wasm_bindgen::JsValue::undefined())
            });
            #[allow(deprecated)]
            if js_sys::futures::jspi_block_on_promise(&promise).is_ok() {
                return cell::claim::<F::Output>();
            }
        }

        let executor = static_executor();
        // Measure a stall, not total runtime. A pending sleep completes as the
        // clock advances, so the spin fallback is making progress even when the
        // sleep outlives the timeout
        let mut stalled_since = super::time::now();
        loop {
            if cell::is_ready() {
                break;
            }
            {
                let _guard = entering_poll();
                super::time::advance_clock();
                unsafe { executor.poll() };
            }
            core::hint::spin_loop();
            if super::time::pending_sleeps() == 0 {
                if super::time::now().saturating_duration_since(stalled_since)
                    > BLOCK_ON_SPIN_TIMEOUT
                {
                    panic!(
                        "block_on: spin fallback made no progress after {}s \
                         ({} sleep{}); futures waiting on JS timers/events cannot complete \
                         while wasm spins; run this call inside a JSPI export (COOP/COEP) \
                         instead",
                        BLOCK_ON_SPIN_TIMEOUT.as_secs(),
                        super::time::pending_sleeps(),
                        if super::time::pending_sleeps() == 1 {
                            ""
                        } else {
                            "s"
                        }
                    );
                }
            } else {
                stalled_since = super::time::now();
            }
        }
    }

    cell::claim::<F::Output>()
}

/// Pump the shared executor once: advance pending sleeps, then run the
/// executor. Normally unnecessary (the `__pender` microtask pump does this
/// automatically); a root driver is only useful for custom run loops.
pub fn pump() {
    let _guard = entering_poll();
    super::time::advance_clock();
    unsafe { static_executor().poll() };
}
