use alloc::boxed::Box;
use core::any::Any;
use core::future::Future;
use core::time::Duration;

pub use crate::base::join::JoinHandle;
use crate::base::queue::{queue, BoxedFuture, NOTIFY};
pub use crate::base::runtime::{new_runtime, Runtime, RuntimeBuilder};
pub use crate::base::spawn::spawn;
use wasm_bindgen::prelude::*;

use super::jspi::{ensure_executor, static_executor, take_output, OutputCapture, OUTPUT_SLOT};

/// Upper bound on `block_on` spin time before declaring the executor wedged.
const BLOCK_ON_DEADLOCK: Duration = Duration::from_secs(30);

/// Ambient JSPI-context probe: rewritten in-wasm by the wasm-bindgen CLI to
/// read `__jspi_stack_base` (non-zero only while a `#[wasm_bindgen(jspi)]`
/// frame is on the stack) and a constant `0` in modules without JSPI.
#[wasm_bindgen(raw_module = "__wbindgen_placeholder__")]
extern "C" {
    fn __wbindgen_jspi_in_context() -> u32;
}

/// Run `fut` on the shared executor, driven by the JS event loop.
///
/// Spawns `fut` and returns immediately; the exported `pump()` entry point
/// must be called from the browser event loop to drive task progress.
pub fn run<F: Future + 'static>(fut: F) {
    ensure_executor();
    let boxed: BoxedFuture = Box::pin(async move {
        let _ = fut.await;
    });
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());
}

/// Block on `fut` using JSPI with a spin-loop fallback.
///
/// **JSPI path** (real async I/O): if the caller is inside a
/// `#[wasm_bindgen(jspi)]` export, `jspi_block_on_promise` suspends the
/// wasm stack until the future completes. JS pumps the executor while we
/// are suspended.
///
/// **Spin fallback** (degraded): if JSPI is unavailable (not in a JSPI
/// context), falls back to a busy-poll loop that drives the executor
/// directly. Futures that need the JS event loop will never complete in
/// this path.
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: Any,
{
    ensure_executor();

    let (done_tx, done_rx) = futures::channel::oneshot::channel::<()>();

    let wrapped = async move {
        let _ = OutputCapture { inner: fut }.await;
        let _ = done_tx.send(());
    };

    let boxed: BoxedFuture = Box::pin(wrapped);
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());

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
            return unsafe { take_output::<F::Output>() };
        }
    }

    // JSPI unavailable (or not in a JSPI context): spin-poll the raw executor
    // that ensure_executor() set up. The task_runner (running on this
    // executor) will drive the spawned future to completion, and the clock
    // registry keeps `sleep`/`timeout` advancing even though JS never runs.
    let executor = static_executor();
    let start = super::time::now();
    loop {
        if unsafe { OUTPUT_SLOT.get() }.is_some() {
            break;
        }
        super::time::advance_clock();
        unsafe { executor.poll() };
        core::hint::spin_loop();
        // Deadlock guard: a wedged run queue must surface as a diagnosable
        // panic instead of an unattributable test hang.
        if super::time::now().saturating_duration_since(start) > BLOCK_ON_DEADLOCK {
            panic!(
                "block_on: no progress after {}s, OUTPUT_SLOT empty",
                BLOCK_ON_DEADLOCK.as_secs()
            );
        }
    }

    unsafe { take_output::<F::Output>() }
}

/// Pump the shared executor once: advance pending sleeps, then run the
/// executor. Call from the browser event loop.
pub fn pump() {
    super::time::advance_clock();
    unsafe { static_executor().poll() };
}
