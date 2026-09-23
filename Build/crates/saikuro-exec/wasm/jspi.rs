use core::any::Any;
use core::future::Future;

use alloc::boxed::Box;
use alloc::vec::Vec;
use futures::stream::StreamExt;

#[cfg(not(target_has_atomic = "ptr"))]
use crate::base::queue::no_atomic_futures::FuturesUnordered;
#[cfg(target_has_atomic = "ptr")]
use futures::stream::FuturesUnordered;

use crate::base::queue::{queue, BoxedFuture, NOTIFY};

pub(crate) fn start_runner(spawner: embassy_executor::Spawner) {
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

pub(crate) fn ensure_executor() {
    static STARTED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, core::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let executor = static_executor();
    let shared: &'static embassy_executor::raw::Executor =
        unsafe { core::mem::transmute::<&mut _, &'static _>(executor) };
    start_runner(shared.spawner());
}

pub(crate) fn static_executor() -> &'static mut embassy_executor::raw::Executor {
    static mut EXECUTOR: Option<embassy_executor::raw::Executor> = None;
    let ex = unsafe {
        (*core::ptr::addr_of_mut!(EXECUTOR))
            .get_or_insert_with(|| embassy_executor::raw::Executor::new(core::ptr::null_mut()))
    };
    // SAFETY: sole static instance, never moved or dropped.
    unsafe { core::mem::transmute::<&mut _, &'static mut _>(ex) }
}

// Output cell

type OutputCell = Option<Box<dyn Any>>;

/// Wrapper future that boxes the inner future's output into a per-call cell.
pub(crate) struct OutputCapture<T> {
    pub(crate) inner: T,
    out: *mut OutputCell,
}

impl<T> OutputCapture<T> {
    pub(crate) fn new(inner: T, out: *mut OutputCell) -> Self {
        Self { inner, out }
    }
}

impl<T> Future for OutputCapture<T>
where
    T: Future,
    T::Output: Any,
{
    type Output = ();

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<()> {
        match unsafe { self.as_mut().map_unchecked_mut(|s| &mut s.inner) }.poll(cx) {
            core::task::Poll::Ready(val) => {
                // SAFETY: `self.out` points into the owning `block_on` frame,
                // which stays live until the done-channel signalled from here
                // has been received, a strictly later event. wasm is
                // single-threaded, so no other writer touches the cell.
                unsafe {
                    (*self.out) = Some(Box::new(val));
                }
                core::task::Poll::Ready(())
            }
            core::task::Poll::Pending => core::task::Poll::Pending,
        }
    }
}

/// Collect the inner output from a `block_on` cell and downcast it.
pub(crate) fn claim<F: 'static>(out: *mut OutputCell) -> F {
    let boxed = unsafe { &mut *out }
        .take()
        .expect("block_on: done-channel resolved without output; executor lost the future");
    match boxed.downcast::<F>() {
        Ok(val) => *val,
        Err(other) => panic!(
            "block_on: output type mismatch in result cell (expected {} [{:?}], stored [{:?}])",
            core::any::type_name::<F>(),
            core::any::TypeId::of::<F>(),
            other.type_id(),
        ),
    }
}
