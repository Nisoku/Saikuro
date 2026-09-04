use alloc::boxed::Box;
use alloc::vec::Vec;
use core::future::Future;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll};

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

pub fn block_on<F: Future + 'static>(fut: F) -> F::Output {
    // A synchronous, returning `block_on` on browser-wasm is only possible for
    // futures that complete purely in-band, without yielding to the JS event loop.
    // Futures that need the event loop will never complete, so this function will spin forever.
    // The caller must ensure that the future is suitable for synchronous execution.
    static WAKER_VTABLE: core::task::RawWakerVTable = core::task::RawWakerVTable::new(
        |p| core::task::RawWaker::new(p, &WAKER_VTABLE),
        |p| unsafe { (*(p as *const AtomicBool)).store(true, Ordering::SeqCst) },
        |p| unsafe { (*(p as *const AtomicBool)).store(true, Ordering::SeqCst) },
        |_| {},
    );

    let woken = AtomicBool::new(true);
    let mut fut = Box::pin(fut);
    unsafe {
        // SAFETY: `woken` outlives this function; the waker only reads/writes
        // the boolean while we hold it on the stack.
        let waker = core::task::Waker::from_raw(core::task::RawWaker::new(
            &woken as *const AtomicBool as *const (),
            &WAKER_VTABLE,
        ));
        let mut cx = Context::from_waker(&waker);
        loop {
            woken.store(false, Ordering::SeqCst);
            if let Poll::Ready(val) = fut.as_mut().poll(&mut cx) {
                return val;
            }
            // Busy-wait until a produced/signalled wake sets the flag. Futures
            // that need the JS event loop will never set it and spin here.
            while !woken.load(Ordering::SeqCst) {}
        }
    }
}

fn static_executor() -> &'static mut ArchExecutor {
    static mut EXECUTOR: Option<ArchExecutor> = None;
    let ex = unsafe { (*core::ptr::addr_of_mut!(EXECUTOR)).get_or_insert_with(ArchExecutor::new) };
    // SAFETY: `EXECUTOR` is a `static mut` holding the sole executor instance; we
    // upgrade its borrow to `'static` for the duration of the program. It is never
    // moved or dropped, and `run`/`start`/`poll` are only called on this reference.
    unsafe { core::mem::transmute::<&mut ArchExecutor, &'static mut ArchExecutor>(ex) }
}

pub fn pump() {}
