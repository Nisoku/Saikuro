use core::future::Future;
use core::pin::Pin;
use core::ptr::{null, null_mut};
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
    static RUNNER_STARTED: portable_atomic::AtomicBool =
        portable_atomic::AtomicBool::new(false);
    if !RUNNER_STARTED.swap(true, Ordering::SeqCst) {
        let ex = static_executor();
        // SAFETY: `ex` is the sole static executor alive for the program's
        // duration; the shared reborrow is only live until `start_runner`
        // returns, so it never overlaps with the mutable borrow aliasing
        // the same single instance.
        let exec_shared: &'static ArchExecutor = unsafe {
            core::mem::transmute::<&mut ArchExecutor, &'static ArchExecutor>(&mut *ex)
        };
        start_runner(exec_shared.spawner());
    }
}

/// No-op: the executor is driven by its arch pender (the
/// `#[embassy_executor::main]` loop on cortex-m). The wasm host entry no
/// longer needs to pump manually.
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

/// Run `fut` to completion on the spin executor. Loops until `fut` resolves so
/// the result can be returned. Used by the no_std and embedded engines.
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: 'static,
{
    block_on_inner(fut)
}

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

fn static_executor() -> &'static mut ArchExecutor {
    static mut EXECUTOR: Option<ArchExecutor> = None;
    let ex = unsafe {
        (*core::ptr::addr_of_mut!(EXECUTOR)).get_or_insert_with(|| ArchExecutor::new(null_mut()))
    };
    // SAFETY: `EXECUTOR` is a `static mut` holding the sole executor instance; we
    // upgrade its borrow to `'static` for the duration of the program. It is never
    // moved or dropped, and `run`/`start`/`poll` are only called on this reference.
    let ex: &'static mut ArchExecutor =
        unsafe { core::mem::transmute::<&mut ArchExecutor, &'static mut ArchExecutor>(ex) };
    ex
}
