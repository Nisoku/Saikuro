use core::any::Any;
use core::cell::UnsafeCell;
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

// Output slot

pub(crate) struct SyncUnsafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncUnsafeCell<T> {}
impl<T> SyncUnsafeCell<T> {
    pub(crate) const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }
    // Unsafe `&mut` through `&self` is the point of this cell: it is a
    // `Sync`-declared owner of a JSPI output slot whose aliasing safety is
    // upheld entirely by the caller (single-threaded wasm, readonly after
    // capture).
    #[allow(clippy::mut_from_ref)]
    pub(crate) unsafe fn get(&self) -> &mut T {
        &mut *self.0.get()
    }
}

pub(crate) static OUTPUT_SLOT: SyncUnsafeCell<Option<*mut dyn Any>> = SyncUnsafeCell::new(None);

pub(crate) struct OutputCapture<T> {
    pub(crate) inner: T,
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
                let boxed: Box<dyn Any> = Box::new(val);
                let ptr: *mut dyn Any = Box::into_raw(boxed);
                unsafe {
                    *OUTPUT_SLOT.get() = Some(ptr);
                }
                core::task::Poll::Ready(())
            }
            core::task::Poll::Pending => core::task::Poll::Pending,
        }
    }
}

pub(crate) unsafe fn take_output<F: 'static>() -> F {
    let ptr = unsafe { OUTPUT_SLOT.get() }
        .take()
        .expect("block_on: output slot was not set");

    let boxed: Box<dyn Any> = unsafe { Box::from_raw(ptr) };

    match boxed.downcast::<F>() {
        Ok(val) => *val,
        Err(_) => unreachable!("block_on: type mismatch in output slot"),
    }
}
