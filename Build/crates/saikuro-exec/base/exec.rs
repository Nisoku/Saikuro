#![cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::future::Future;
#[cfg(any(feature = "wasm", feature = "no_std"))]
use core::mem::transmute;
use core::pin::Pin;
use core::task::{Context, Poll};

#[cfg(feature = "no_std")]
use core::ptr::null_mut;

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::signal::Signal;
use embassy_sync::waitqueue::MultiWakerRegistration;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::shared::JoinError;

#[cfg(feature = "no_std")]
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
    spawner.spawn(task_runner()).ok();
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
/// no_std (arch-spin busy-poll) so the result can be returned.
#[cfg(feature = "no_std")]
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let executor = static_executor();
    let spawner = executor.spawner();
    start_runner(spawner);

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

    loop {
        // SAFETY: `executor` is `&'static` (see `static_executor`) and `poll` is
        // only ever called from this single owner thread.
        unsafe { executor.poll() };
        if let Some(v) = slot.lock(|s| s.borrow_mut().value.take()) {
            return v;
        }
    }
}

#[cfg(feature = "wasm")]
pub fn run<F: Future + Send + 'static>(fut: F) {
    let executor = static_executor();
    executor.start(|spawner| {
        start_runner(spawner);
        let boxed: Pin<Box<dyn Future<Output = ()> + Send + 'static>> = Box::pin(async move {
            let _ = fut.await;
        });
        queue().lock(|q| q.borrow_mut().push(boxed));
        NOTIFY.signal(());
    });
}

#[cfg(feature = "wasm")]
pub fn block_on<F: Future + Send + 'static>(fut: F) -> F::Output {
    run(fut);
    // `run` returns to the JS event loop, which drives the executor; for a server
    // future this never completes. Unused on wasm (the entry uses `run`).
    loop {}
}

/// No-op on embassy engines: the executor is driven by its arch pender (JS
/// timer on wasm, the `#[embassy_executor::main]` loop on cortex-m). The wasm
/// host entry no longer needs to pump manually.
pub fn pump() {}

#[cfg(any(feature = "wasm", feature = "no_std"))]
fn static_executor() -> &'static mut ArchExecutor {
    static mut EXECUTOR: Option<ArchExecutor> = None;
    #[cfg(feature = "no_std")]
    let ex = unsafe {
        (*core::ptr::addr_of_mut!(EXECUTOR)).get_or_insert_with(|| ArchExecutor::new(null_mut()))
    };
    #[cfg(feature = "wasm")]
    let ex = unsafe { (*core::ptr::addr_of_mut!(EXECUTOR)).get_or_insert_with(ArchExecutor::new) };
    // SAFETY: `EXECUTOR` is a `static mut` holding the sole executor instance; we
    // upgrade its borrow to `'static` for the duration of the program. It is never
    // moved or dropped, and `run`/`start`/`poll` are only called on this reference.
    unsafe { transmute::<&mut ArchExecutor, &'static mut ArchExecutor>(ex) }
}

#[cfg(any(feature = "wasm", feature = "no_std"))]
pub fn new_runtime() -> Runtime {
    Runtime::new()
}

#[cfg(any(feature = "wasm", feature = "no_std"))]
pub struct Runtime;

#[cfg(any(feature = "wasm", feature = "no_std"))]
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

#[cfg(any(feature = "wasm", feature = "no_std"))]
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(feature = "wasm", feature = "no_std"))]
pub struct RuntimeBuilder {
    _private: (),
}

#[cfg(any(feature = "wasm", feature = "no_std"))]
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
