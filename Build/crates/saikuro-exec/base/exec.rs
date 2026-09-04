#![cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]

#[cfg(target_has_atomic = "ptr")]
use crate::Arc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::future::Future;
#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
use core::mem::transmute;
use core::pin::Pin;
use core::task::{Context, Poll};
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;

#[cfg(any(feature = "no_std", feature = "embedded"))]
use core::ptr::null_mut;

#[cfg(not(target_has_atomic = "ptr"))]
use self::no_atomic_futures::FuturesUnordered;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::signal::Signal;
use embassy_sync::waitqueue::MultiWakerRegistration;
#[cfg(target_has_atomic = "ptr")]
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;

use crate::shared::JoinError;

#[cfg(any(feature = "no_std", feature = "embedded"))]
use embassy_executor::raw::Executor as ArchExecutor;
#[cfg(feature = "wasm")]
use embassy_executor::Executor as ArchExecutor;

/// Shared result slot between a spawned task and its [`JoinHandle`].
struct JoinSlot<T> {
    value: Option<T>,
    closed: bool,
    wakers: MultiWakerRegistration<8>,
}

type JoinResultSlot<T> = CriticalSectionMutex<RefCell<JoinSlot<T>>>;

#[cfg(feature = "native")]
type BoxedFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
#[cfg(not(feature = "native"))]
type BoxedFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

/// Queue of dynamically spawned futures waiting to be picked up by `task_runner`.
#[cfg(feature = "native")]
static QUEUE: CriticalSectionMutex<RefCell<Vec<BoxedFuture>>> =
    CriticalSectionMutex::new(RefCell::new(Vec::new()));

#[cfg(not(feature = "native"))]
struct QueueWrapper(CriticalSectionMutex<RefCell<Vec<BoxedFuture>>>);
#[cfg(not(feature = "native"))]
unsafe impl Sync for QueueWrapper {}
#[cfg(not(feature = "native"))]
static QUEUE: QueueWrapper = QueueWrapper(CriticalSectionMutex::new(RefCell::new(Vec::new())));

#[cfg(feature = "native")]
fn queue() -> &'static CriticalSectionMutex<RefCell<Vec<BoxedFuture>>> {
    &QUEUE
}
#[cfg(not(feature = "native"))]
fn queue() -> &'static CriticalSectionMutex<RefCell<Vec<BoxedFuture>>> {
    &QUEUE.0
}

/// Wakes `task_runner` when a new future is queued (or when the runner should
/// re-check the queue after draining).
static NOTIFY: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Launch the single multiplexing task onto the supplied `Spawner`.
pub fn start_runner(spawner: Spawner) {
    spawner.spawn(task_runner().expect("task_runner"));
}

/// The one embassy task. It multiplexes every dynamically spawned future through
/// a `FuturesUnordered`, so the embassy task set stays statically sized (just
/// this task) while concurrency is unbounded and heap-backed.
#[embassy_executor::task]
async fn task_runner() {
    let mut set: FuturesUnordered<BoxedFuture> = FuturesUnordered::new();
    loop {
        let batch: Vec<BoxedFuture> = queue().lock(|q| q.borrow_mut().drain(..).collect());
        for fut in batch {
            set.push(fut);
        }

        // With an empty set, `FuturesUnordered::next()` yields `None` (Ready)
        // immediately, so it must not be selected against `NOTIFY`: that would
        // hot-loop forever. Block on `NOTIFY` directly instead.
        if set.is_empty() {
            NOTIFY.wait().await;
            continue;
        }

        // Wait until either a multiplexed future completes or a new one is queued.
        embassy_futures::select::select(set.next(), NOTIFY.wait()).await;
    }
}

/// Spawn a runtime-dynamic future. The future is boxed and handed to
/// `task_runner`; its output is delivered through the returned [`JoinHandle`].
#[cfg(feature = "native")]
pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let slot: Arc<JoinResultSlot<F::Output>> =
        Arc::new(CriticalSectionMutex::new(RefCell::new(JoinSlot {
            value: None,
            closed: false,
            wakers: MultiWakerRegistration::new(),
        })));
    let task_slot = slot.clone();
    let boxed: Pin<Box<dyn Future<Output = ()> + Send + 'static>> = Box::pin(async move {
        let result = fut.await;
        task_slot.lock(|s| {
            s.borrow_mut().value = Some(result);
            s.borrow_mut().wakers.wake();
        });
    });
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());
    JoinHandle { slot }
}

/// Embedded variant: embassy-net runs single-threaded, so spawned tasks are
/// `!Send`.
#[cfg(not(feature = "native"))]
pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + 'static,
    F::Output: 'static,
{
    let slot: Arc<JoinResultSlot<F::Output>> =
        Arc::new(CriticalSectionMutex::new(RefCell::new(JoinSlot {
            value: None,
            closed: false,
            wakers: MultiWakerRegistration::new(),
        })));
    let task_slot = slot.clone();
    let boxed: Pin<Box<dyn Future<Output = ()> + 'static>> = Box::pin(async move {
        let result = fut.await;
        task_slot.lock(|s| {
            s.borrow_mut().value = Some(result);
            s.borrow_mut().wakers.wake();
        });
    });
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());
    JoinHandle { slot }
}

/// Run `fut` to completion on the embassy executor. Never returns on wasm
/// (the JS event loop drives the executor); loops until `fut` resolves on
/// no_std/embedded (arch-spin busy-poll) so the result can be returned.
#[cfg(feature = "no_std")]
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: 'static,
{
    block_on_inner(fut)
}

/// Embedded variant: single-threaded, futures need not be `Send`.
#[cfg(feature = "embedded")]
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: 'static,
{
    block_on_inner(fut)
}

#[cfg(any(feature = "no_std", feature = "embedded"))]
fn block_on_inner<F>(mut fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: 'static,
{
    let executor = static_executor();

    use core::task::{RawWaker, RawWakerVTable, Waker};
    fn noop_waker_noop(_: *const ()) {}
    static NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |p| RawWaker::new(p, &NOOP_WAKER_VTABLE),
        noop_waker_noop,
        noop_waker_noop,
        noop_waker_noop,
    );

    let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE)) };
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

#[cfg(feature = "wasm")]
pub fn run<F: Future + 'static>(fut: F) {
    let executor = static_executor();
    executor.start(|spawner| {
        start_runner(spawner);
        let boxed: Pin<Box<dyn Future<Output = ()> + 'static>> = Box::pin(async move {
            let _ = fut.await;
        });
        queue().lock(|q| q.borrow_mut().push(boxed));
        NOTIFY.signal(());
    });
}

#[cfg(feature = "wasm")]
pub fn block_on<F: Future + 'static>(fut: F) -> F::Output {
    // A synchronous, returning `block_on` on browser-wasm is only possible for
    // futures that complete purely in-band, without yielding to the JS event loop.
    // Futures that need the event loop will never complete, so this function will spin forever.
    // The caller must ensure that the future is suitable for synchronous execution.
    use core::sync::atomic::{AtomicBool, Ordering};
    use core::task::{RawWaker, RawWakerVTable, Waker};

    static WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |p| RawWaker::new(p, &WAKER_VTABLE),
        |p| unsafe { (*(p as *const AtomicBool)).store(true, Ordering::SeqCst) },
        |p| unsafe { (*(p as *const AtomicBool)).store(true, Ordering::SeqCst) },
        |_| {},
    );

    let woken = AtomicBool::new(true);
    let mut fut = Box::pin(fut);
    unsafe {
        // SAFETY: `woken` outlives this function; the waker only reads/writes
        // the boolean while we hold it on the stack.
        let waker = Waker::from_raw(RawWaker::new(
            &woken as *const AtomicBool as *const (),
            &WAKER_VTABLE,
        ));
        let mut cx = core::task::Context::from_waker(&waker);
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

/// No-op on embassy engines: the executor is driven by its arch pender (JS
/// timer on wasm, the `#[embassy_executor::main]` loop on cortex-m). The wasm
/// host entry no longer needs to pump manually.
pub fn pump() {}

#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
fn static_executor() -> &'static mut ArchExecutor {
    static mut EXECUTOR: Option<ArchExecutor> = None;
    #[cfg(any(feature = "no_std", feature = "embedded"))]
    let ex = unsafe {
        (*core::ptr::addr_of_mut!(EXECUTOR)).get_or_insert_with(|| ArchExecutor::new(null_mut()))
    };
    #[cfg(feature = "wasm")]
    let ex = unsafe { (*core::ptr::addr_of_mut!(EXECUTOR)).get_or_insert_with(ArchExecutor::new) };
    // SAFETY: `EXECUTOR` is a `static mut` holding the sole executor instance; we
    // upgrade its borrow to `'static` for the duration of the program. It is never
    // moved or dropped, and `run`/`start`/`poll` are only called on this reference.
    let ex: &'static mut ArchExecutor =
        unsafe { transmute::<&mut ArchExecutor, &'static mut ArchExecutor>(ex) };
    // The multiplexing task is a singleton: spawn it exactly once, at executor
    // creation, so repeated `block_on` calls reuse the runner instead of
    // re-arming the (permanently-resident) task pool slot.
    #[cfg(any(feature = "no_std", feature = "embedded"))]
    {
        use core::sync::atomic::Ordering;
        // `portable_atomic` provides `swap` on targets without native atomic
        // support (e.g. thumbv6m/riscv32imc) via the critical-section fallback.
        static RUNNER_STARTED: portable_atomic::AtomicBool =
            portable_atomic::AtomicBool::new(false);
        if !RUNNER_STARTED.swap(true, Ordering::SeqCst) {
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
    ex
}

#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
pub fn new_runtime() -> Runtime {
    Runtime::new()
}

#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
pub struct Runtime;

#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
impl Runtime {
    pub fn new() -> Self {
        Runtime
    }

    pub fn new_multi_thread() -> Self {
        Runtime
    }

    pub fn new_current_thread() -> Self {
        Runtime
    }

    pub fn block_on<F: Future + Send + 'static>(&self, fut: F) -> F::Output
    where
        F::Output: Send + 'static,
    {
        block_on(fut)
    }
}

#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
pub struct RuntimeBuilder {
    _private: (),
}

#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
impl RuntimeBuilder {
    pub fn new_multi_thread() -> Self {
        RuntimeBuilder { _private: () }
    }

    pub fn new_current_thread() -> Self {
        RuntimeBuilder { _private: () }
    }

    pub fn enable_all(self) -> Self {
        self
    }

    pub fn worker_threads(self, _n: usize) -> Self {
        self
    }

    pub fn build(self) -> Runtime {
        Runtime::new()
    }
}

pub struct JoinHandle<T> {
    slot: Arc<JoinResultSlot<T>>,
}

impl<T> JoinHandle<T> {
    pub fn abort(&self) {
        self.slot.lock(|s| s.borrow_mut().closed = true);
    }

    pub fn is_finished(&self) -> bool {
        self.slot.lock(|s| s.borrow().value.is_some())
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut result = None;
        self.slot.lock(|s| {
            let mut state = s.borrow_mut();
            if let Some(v) = state.value.take() {
                result = Some(Ok(v));
            } else if state.closed {
                result = Some(Err(JoinError::cancelled()));
            } else {
                state.wakers.register(cx.waker());
            }
        });
        match result {
            Some(r) => Poll::Ready(r),
            None => Poll::Pending,
        }
    }
}

/// Minimal `FuturesUnordered` for targets without `target_has_atomic = "ptr"`.
///
/// Polls all contained futures on every waker notification.
#[cfg(not(target_has_atomic = "ptr"))]
mod no_atomic_futures {
    use alloc::vec::Vec;
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};
    use futures::stream::Stream;

    pub(super) struct FuturesUnordered<F> {
        futures: Vec<F>,
    }

    impl<F> FuturesUnordered<F> {
        pub fn new() -> Self {
            Self {
                futures: Vec::new(),
            }
        }

        pub fn push(&mut self, f: F) {
            self.futures.push(f);
        }

        pub fn is_empty(&self) -> bool {
            self.futures.is_empty()
        }
    }

    impl<F: Future> Stream for FuturesUnordered<F> {
        type Item = F::Output;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let this = unsafe { self.get_unchecked_mut() };
            let mut i = this.futures.len();
            while i > 0 {
                i -= 1;
                if let Poll::Ready(output) =
                    unsafe { Pin::new_unchecked(&mut this.futures[i]) }.poll(cx)
                {
                    this.futures.swap_remove(i);
                    return Poll::Ready(Some(output));
                }
            }
            Poll::Pending
        }
    }
}
