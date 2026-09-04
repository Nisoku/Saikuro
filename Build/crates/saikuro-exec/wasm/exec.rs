use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use core::future::Future;

use embassy_executor::Executor as ArchExecutor;
use futures::stream::StreamExt;

#[cfg(not(target_has_atomic = "ptr"))]
use crate::base::queue::no_atomic_futures::FuturesUnordered;
#[cfg(target_has_atomic = "ptr")]
use futures::stream::FuturesUnordered;

pub use crate::base::join::JoinHandle;
use crate::base::queue::{queue, BoxedFuture, NOTIFY};
pub use crate::base::runtime::{new_runtime, Runtime, RuntimeBuilder};
pub use crate::base::spawn::spawn;

use super::jspi::{ensure_executor, take_output, OutputCapture, OUTPUT_SLOT};

pub fn start_runner(spawner: embassy_executor::Spawner) {
    spawner.spawn(task_runner().expect("task_runner"));
}

#[embassy_executor::task]
async fn task_runner() {
    let mut set: FuturesUnordered<BoxedFuture> = FuturesUnordered::new();
    loop {
        let batch: Vec<BoxedFuture> = queue().lock(|q| q.borrow_mut().drain(..).collect());
        for fut in batch {
            set.push(fut);
        }

        if set.is_empty() {
            NOTIFY.wait().await;
            continue;
        }

        embassy_futures::select::select(set.next(), NOTIFY.wait()).await;
    }
}

/// Run `fut` to completion on the embassy executor, driving it from the JS
/// event loop. Never returns.
pub fn run<F: Future + 'static>(fut: F) {
    let executor = static_executor();
    executor.start(|spawner| {
        start_runner(spawner);
        let boxed: BoxedFuture = Box::pin(async move {
            let _ = fut.await;
        });
        queue().lock(|q| q.borrow_mut().push(boxed));
        NOTIFY.signal(());
    });
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

    // Try JSPI first: suspend the wasm stack until the promise resolves.
    let promise = wasm_bindgen_futures::future_to_promise(async {
        done_rx
            .await
            .map_err(|_| wasm_bindgen::JsValue::undefined())
            .map(|()| wasm_bindgen::JsValue::undefined())
    });

    #[allow(deprecated)]
    if js_sys::futures::jspi_block_on_promise(&promise).is_ok() {
        return unsafe { take_output::<F::Output>() };
    }

    // JSPI unavailable: spin-poll the raw executor that ensure_executor()
    // set up. The task_runner (running on this executor) will drive the
    // spawned future to completion.
    let executor = super::jspi::static_executor();
    loop {
        if unsafe { OUTPUT_SLOT.get() }.is_some() {
            break;
        }
        unsafe { executor.poll() };
        core::hint::spin_loop();
    }

    unsafe { take_output::<F::Output>() }
}

fn static_executor() -> &'static mut ArchExecutor {
    static mut EXECUTOR: Option<ArchExecutor> = None;
    let ex = unsafe { (*core::ptr::addr_of_mut!(EXECUTOR)).get_or_insert_with(ArchExecutor::new) };
    unsafe { core::mem::transmute::<&mut ArchExecutor, &'static mut ArchExecutor>(ex) }
}

pub fn pump() {}
