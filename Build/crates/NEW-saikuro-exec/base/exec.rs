#![cfg(any(feature = "wasm", feature = "no_std"))]

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::cell::OnceCell;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use embassy_executor::{Executor, Spawner};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::waitqueue::MultiWakerRegistration;

use crate::shared::JoinError;

pub use crate::base::{fuse_select, sleep, timeout, yield_now};
pub use crate::base::{mpsc, oneshot, sync, watch};

/// Shared result slot between a spawned task and its [`JoinHandle`].
struct JoinSlot<T> {
    value: Option<T>,
    closed: bool,
    wakers: MultiWakerRegistration<8>,
}

type JoinResultSlot<T> = CriticalSectionMutex<RefCell<JoinSlot<T>>>;

static EXECUTOR: OnceCell<Executor> = OnceCell::new();
static SPAWNER: OnceCell<Spawner> = OnceCell::new();

fn global_executor() -> &'static Executor {
    EXECUTOR.get_or_init(Executor::new)
}

fn global_spawner() -> &'static Spawner {
    SPAWNER.get_or_init(|| global_executor().spawner())
}

/// Safe wrapper around embassy-executor's `unsafe fn poll()`. The host or
/// `main` calls this in a loop.
pub fn pump() {
    let executor = global_executor();
    // SAFETY: `executor` is `&'static` and initialized exactly once via
    // `get_or_init`. `poll` is never called reentrantly on this executor,
    // and the embassy pender (arch-spin) never calls `poll` directly.
    unsafe { executor.poll() };
}

pub fn new_runtime() -> Runtime {
    Runtime::new()
}

pub struct Runtime;

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

    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        block_on(fut)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RuntimeBuilder {
    _private: (),
}

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

pub fn block_on<F: Future + 'static>(fut: F) -> F::Output {
    let slot: Arc<JoinResultSlot<Option<F::Output>>> = Arc::new(CriticalSectionMutex::new(
        RefCell::new(JoinSlot {
            value: None,
            closed: false,
            wakers: MultiWakerRegistration::new(),
        }),
    ));
    let task_slot = slot.clone();
    let token = global_executor().spawn(async move {
        let result = fut.await;
        task_slot.lock(|s| {
            s.borrow_mut().value = Some(result);
            s.borrow().wakers.wake();
        });
    });
    global_spawner().spawn(token).ok();
    loop {
        pump();
        if let Some(v) = slot.lock(|s| s.borrow_mut().value.take()) {
            return v;
        }
    }
}

pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + 'static,
    F::Output: 'static,
{
    let slot: Arc<JoinResultSlot<Option<F::Output>>> = Arc::new(CriticalSectionMutex::new(
        RefCell::new(JoinSlot {
            value: None,
            closed: false,
            wakers: MultiWakerRegistration::new(),
        }),
    ));
    let task_slot = slot.clone();
    let token = global_executor().spawn(async move {
        let result = fut.await;
        task_slot.lock(|s| {
            s.borrow_mut().value = Some(result);
            s.borrow().wakers.wake();
        });
    });
    global_spawner().spawn(token).ok();
    JoinHandle { slot }
}

pub struct JoinHandle<T> {
    slot: Arc<JoinResultSlot<Option<T>>>,
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
