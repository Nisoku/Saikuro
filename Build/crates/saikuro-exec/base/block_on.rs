//! Executor entry point shared by the no_std and embedded engines.

#[cfg(any(feature = "no_std", feature = "embedded"))]
use core::future::Future;
#[cfg(any(feature = "no_std", feature = "embedded"))]
use core::pin::Pin;
#[cfg(any(feature = "no_std", feature = "embedded"))]
use core::ptr::null;
use core::ptr::null_mut;
#[cfg(any(feature = "no_std", feature = "embedded"))]
use core::task::{Context, Poll};

use embassy_executor::raw::Executor as ArchExecutor;
use futures::stream::StreamExt;

#[cfg(not(target_has_atomic = "ptr"))]
use crate::base::queue::no_atomic_futures::FuturesUnordered;
#[cfg(target_has_atomic = "ptr")]
use futures::stream::FuturesUnordered;

use crate::base::queue::{queue, BoxedFuture, NOTIFY};

pub fn start_runner(spawner: embassy_executor::Spawner) {
    spawner.spawn(task_runner().expect("task_runner"));
}

/// Ensure the task-runner singleton has been spawned. Called lazily.
pub(crate) fn ensure_runner_started() {
    use core::sync::atomic::Ordering;
    static RUNNER_STARTED: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);
    if !RUNNER_STARTED.swap(true, Ordering::SeqCst) {
        let exec_shared = static_executor();
        start_runner(exec_shared.spawner());
    }
}

/// No-op: the executor is driven by its arch pender (the
/// `#[embassy_executor::main]` loop on cortex-m). The wasm host entry no
/// longer needs to pump manually.
#[cfg(any(feature = "no_std", feature = "embedded"))]
pub fn pump() {}

#[embassy_executor::task]
async fn task_runner() {
    let mut set: FuturesUnordered<BoxedFuture> = FuturesUnordered::new();
    loop {
        let batch: alloc::vec::Vec<BoxedFuture> =
            queue().lock(|q| q.borrow_mut().drain(..).collect());
        // Reclaim the QUEUE Vec's retained capacity after draining.
        queue().lock(|q| q.borrow_mut().shrink_to_fit());
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

/// Shared failure for a `block_on` invoked from inside a future this runtime is
/// already driving.
#[cold]
#[track_caller]
pub(crate) fn nested_block_on_panic() -> ! {
    panic!(
        "block_on: called from inside the executor (a task on this runtime is already driving \
         it); this would deadlock so restructure the task to `await` instead"
    );
}

/// Nesting depth of the embedded `block_on` poll loop.
#[cfg(any(feature = "no_std", feature = "embedded"))]
static BLOCK_ON_DEPTH: portable_atomic::AtomicUsize = portable_atomic::AtomicUsize::new(0);

/// Releases a [`BLOCK_ON_DEPTH`] reservation on drop, including on unwind.
#[cfg(any(feature = "no_std", feature = "embedded"))]
struct BlockOnDepthGuard;

#[cfg(any(feature = "no_std", feature = "embedded"))]
impl Drop for BlockOnDepthGuard {
    fn drop(&mut self) {
        BLOCK_ON_DEPTH.fetch_sub(1, core::sync::atomic::Ordering::SeqCst);
    }
}

/// Run `fut` to completion on the spin executor. Loops until `fut` resolves so
/// the result can be returned. Used by the no_std and embedded engines; the
/// wasm engine drives work through `wasm::pump` instead.
///
/// Panics if called from inside a future already being driven.
#[cfg(any(feature = "no_std", feature = "embedded"))]
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: 'static,
{
    use core::sync::atomic::Ordering;
    if BLOCK_ON_DEPTH.load(Ordering::SeqCst) != 0 {
        nested_block_on_panic();
    }
    BLOCK_ON_DEPTH.fetch_add(1, Ordering::SeqCst);
    let _depth = BlockOnDepthGuard;
    block_on_inner(fut)
}

#[cfg(any(feature = "no_std", feature = "embedded"))]
fn block_on_inner<F>(mut fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: 'static,
{
    let executor = static_executor();

    fn noop_waker_noop(_: *const ()) {}
    static NOOP_WAKER_VTABLE: core::task::RawWakerVTable = core::task::RawWakerVTable::new(
        |p| core::task::RawWaker::new(p, &NOOP_WAKER_VTABLE),
        noop_waker_noop,
        noop_waker_noop,
        noop_waker_noop,
    );

    let waker = unsafe {
        core::task::Waker::from_raw(core::task::RawWaker::new(null(), &NOOP_WAKER_VTABLE))
    };
    let mut cx = Context::from_waker(&waker);
    let mut fut = unsafe { Pin::new_unchecked(&mut fut) };

    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => {}
        }
        unsafe { executor.poll() };
    }
}

pub(crate) fn static_executor() -> &'static ArchExecutor {
    use core::sync::atomic::Ordering;
    static mut EXECUTOR: Option<ArchExecutor> = None;
    static EXECUTOR_INIT: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);

    // SAFETY: `EXECUTOR` is initialized (through the unique borrow `addr_of_mut!`
    // hands out) at most once before any reference to it exists, and the engine
    // is single-threaded, so the check-and-write cannot observe a partially
    // written executor.
    if !EXECUTOR_INIT.load(Ordering::Acquire) {
        let _ = unsafe {
            (*core::ptr::addr_of_mut!(EXECUTOR))
                .get_or_insert_with(|| ArchExecutor::new(null_mut()))
        };
        EXECUTOR_INIT.store(true, Ordering::Release);
    }

    // SAFETY: after `EXECUTOR_INIT` is set the executor is `Some`, never moved,
    // dropped, or written again, so the shared reference derived from the static
    // stays valid for the rest of the program. Every call reads through the raw
    // place, so no stale unique tag is reused and polling/spawning only ever see
    // shared aliases (`poll` and `spawner` both take `&self`).
    unsafe {
        (*core::ptr::addr_of_mut!(EXECUTOR))
            .as_ref()
            .unwrap_unchecked()
    }
}
