//! Embassy backend for `saikuro-exec` (`no_std`).
//!
//! Embassy-backed implementations of the saikuro-exec API surface. The actual
//! executor comes from the application via `embassy-executor`; all this crate
//! provides is the concurrency facade.
//!
//! # Channels
//!
//! `mpsc`, `oneshot`, and `watch` are real, owned wrappers over embassy-sync
//! primitives. The channel state is shared between sender and receiver through
//! `alloc::sync::Arc`, so the handles are `'static` (same as the tokio facade)
//! and the backing storage is freed once the last handle is dropped. In
//! practice the router creates its facade channels once and keeps them around
//! for the whole life of the process.
//!
//! Channel state is guarded by
//! `embassy_sync::blocking_mutex::CriticalSectionRawMutex`. On single-core MCUs
//! the `critical-section` backend comes from the HAL
//! (`critical-section-single-core`, `cortex-m`, etc.); multicore targets have
//! to supply a critical-section impl that covers the whole core.
//!
//! # Task lifecycle
//!
//! There's no `spawn` or `block_on` here. The embassy executor owns task
//! scheduling: the application stands up a static `embassy_executor::Executor`
//! and hands out `Spawner`s. A facade can't conjure its own global executor
//! without clashing with the application's. The stubs are only here so that
//! host-only crates selecting `tokio-runtime` still resolve, call one and it
//! panics, pointing you at the embassy equivalent. `net`, `signal`, and
//! `runtime` are missing from the embassy model for the same reason.

use alloc::sync::Arc;
use core::cell::RefCell;
use core::future::{poll_fn, Future};
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use core::time::Duration;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::channel::Channel as EmbChannel;
use embassy_sync::channel::TrySendError as EmbTrySendError;
use embassy_sync::waitqueue::MultiWakerRegistration;
use embassy_time::{Duration as EmbDuration, Timer};

// Sleep / Timeout / Yield

pub async fn sleep(dur: Duration) {
    Timer::after(EmbDuration::from_millis(dur.as_millis() as u64)).await;
}

pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    match embassy_futures::select::select(
        fut,
        Timer::after(EmbDuration::from_millis(dur.as_millis() as u64)),
    )
    .await
    {
        embassy_futures::select::Either::First(res) => Ok(res),
        embassy_futures::select::Either::Second(_) => Err(()),
    }
}

pub async fn yield_now() {
    embassy_futures::yield_now().await;
}

// Spawn / Block-on
// See the module documentation: the application owns the executor and its
// Spawner, so the facade cannot provide a global spawn or block_on.
pub fn spawn<F, T>(_fut: F) -> JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    panic!(
        "saikuro-exec: embassy spawn requires a Spawner; \
         use embassy_executor::Spawner::spawn() directly"
    )
}

pub fn block_on<F>(_future: F) -> F::Output
where
    F: Future,
{
    panic!(
        "saikuro-exec: block_on is not available on embassy-runtime; \
         use embassy_executor::Executor instead"
    )
}

// JoinHandle / Runtime stubs
pub struct JoinHandle<T> {
    _marker: core::marker::PhantomData<T>,
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        unreachable!("saikuro-exec: JoinHandle::poll on embassy-runtime (spawn is not provided)")
    }
}

#[derive(Debug)]
pub struct JoinError;

impl core::fmt::Display for JoinError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("task was cancelled")
    }
}

pub struct Runtime {
    _private: (),
}

pub struct RuntimeBuilder {
    _private: (),
}

impl RuntimeBuilder {
    pub fn enable_all(self) -> Self {
        self
    }

    pub fn build(self) -> Result<Runtime, RuntimeBuildError> {
        Ok(Runtime { _private: () })
    }
}

#[derive(Debug)]
pub struct RuntimeBuildError;

impl core::fmt::Display for RuntimeBuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("embassy runtime does not support tokio-style Builder")
    }
}

pub fn new_runtime() -> RuntimeBuilder {
    RuntimeBuilder { _private: () }
}

impl Runtime {
    pub fn block_on<F: Future>(&self, _future: F) -> F::Output {
        panic!("saikuro-exec: Runtime::block_on is not available on embassy-runtime")
    }
}

// mpsc
/// Bounded multi-producer, single-consumer channel.
pub mod mpsc {
    use super::*;

    /// Fixed backing capacity of an embassy mpsc channel.
    ///
    /// `saikuro-exec::mpsc::channel` takes a runtime capacity to match tokio's
    /// API, but embassy-sync's `Channel` needs the capacity as a const generic.
    /// The facade allocates a queue of this size and asserts that the requested
    /// capacity fits within it.  The router's default channel capacity is 128;
    /// 256 leaves headroom for runtime configuration.
    pub const CHANNEL_CAPACITY: usize = 256;

    /// Waker slots for senders blocked on a full channel.
    ///
    /// `MultiWakerRegistration` falls back to waking every registered waker
    /// when this fills up, so this is a performance knob, not a hard limit.
    const MAX_WAITING_SENDERS: usize = 16;

    /// Error returned by [`Sender::send`] when the receiver has been dropped.
    #[derive(Debug)]
    pub struct SendError<T>(pub T);

    impl<T> SendError<T> {
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> core::fmt::Display for SendError<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("send failed: channel is disconnected")
        }
    }

    /// Error returned by [`Sender::try_send`].
    #[derive(Debug)]
    pub enum TrySendError<T> {
        Full(T),
        Disconnected(T),
    }

    impl<T> TrySendError<T> {
        pub fn into_inner(self) -> T {
            match self {
                TrySendError::Full(v) => v,
                TrySendError::Disconnected(v) => v,
            }
        }

        pub fn is_full(&self) -> bool {
            matches!(self, TrySendError::Full(_))
        }

        pub fn is_disconnected(&self) -> bool {
            matches!(self, TrySendError::Disconnected(_))
        }
    }

    impl<T> core::fmt::Display for TrySendError<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                TrySendError::Full(_) => f.write_str("send failed: channel is full"),
                TrySendError::Disconnected(_) => {
                    f.write_str("send failed: channel is disconnected")
                }
            }
        }
    }

    struct ChannelState {
        senders: usize,
        receivers: usize,
        senders_waiting: MultiWakerRegistration<MAX_WAITING_SENDERS>,
        receivers_waiting: MultiWakerRegistration<1>,
    }

    impl ChannelState {
        const fn new() -> Self {
            ChannelState {
                senders: 0,
                receivers: 0,
                senders_waiting: MultiWakerRegistration::new(),
                receivers_waiting: MultiWakerRegistration::new(),
            }
        }
    }

    struct ChannelInner<T> {
        state: CriticalSectionMutex<RefCell<ChannelState>>,
        channel: EmbChannel<CriticalSectionRawMutex, T, CHANNEL_CAPACITY>,
    }

    /// Sending half of a bounded mpsc channel.  Cloneable; each clone can send
    /// independently, and the channel closes for receivers once every sender is
    /// dropped.
    pub struct Sender<T> {
        inner: Arc<ChannelInner<T>>,
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            self.inner.state.lock(|s| s.borrow_mut().senders += 1);
            Sender {
                inner: self.inner.clone(),
            }
        }
    }

    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.senders -= 1;
                if state.senders == 0 {
                    // A receiver parked in recv() must observe the closure.
                    state.receivers_waiting.wake();
                }
            });
        }
    }

    impl<T> Sender<T> {
        /// Returns true once the receiver has been dropped.
        pub fn is_closed(&self) -> bool {
            self.inner.state.lock(|s| s.borrow().receivers == 0)
        }

        /// Attempt to enqueue `value` without waiting.
        pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
            if self.is_closed() {
                return Err(TrySendError::Disconnected(value));
            }
            match self.inner.channel.try_send(value) {
                Ok(()) => Ok(()),
                Err(EmbTrySendError::Full(value)) => Err(TrySendError::Full(value)),
            }
        }

        /// Send `value`, waiting for capacity when the channel is full.
        ///
        /// Returns `Err(SendError(value))` once the receiver has been dropped.
        pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
            // The message lives in an Option so the FnMut poll closure can take
            // and restore it without moving out of the captured binding.
            let mut pending = Some(value);
            poll_fn(move |cx| {
                loop {
                    if self.is_closed() {
                        // The Full arm always restores the message before the
                        // loop continues, so `pending` is Some here.
                        let message = pending
                            .take()
                            .expect("mpsc send message is restored on the Full path");
                        return Poll::Ready(Err(SendError(message)));
                    }
                    let message = pending
                        .take()
                        .expect("mpsc send message is restored on the Full path");
                    match self.inner.channel.try_send(message) {
                        Ok(()) => return Poll::Ready(Ok(())),
                        Err(EmbTrySendError::Full(message)) => {
                            pending = Some(message);
                            self.inner
                                .state
                                .lock(|s| s.borrow_mut().senders_waiting.register(cx.waker()));
                            // Re-check after registering so a wake that fired
                            // between try_send and register is not missed.
                            if self.is_closed() {
                                let message = pending
                                    .take()
                                    .expect("mpsc send message is restored on the Full path");
                                return Poll::Ready(Err(SendError(message)));
                            }
                            if !self.inner.channel.is_full() {
                                continue;
                            }
                            return Poll::Pending;
                        }
                    }
                }
            })
            .await
        }
    }

    /// Receiving half of a bounded mpsc channel.  Not cloneable.
    pub struct Receiver<T> {
        inner: Arc<ChannelInner<T>>,
    }

    impl<T> Drop for Receiver<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.receivers -= 1;
                if state.receivers == 0 {
                    // Senders blocked on a full channel must observe the
                    // receiver disappearing, otherwise they wait forever.
                    state.senders_waiting.wake();
                }
            });
        }
    }

    impl<T> Receiver<T> {
        /// Receive the next value, or `None` once every sender has been dropped
        /// and the buffered values have been drained.
        pub async fn recv(&mut self) -> Option<T> {
            poll_fn(|cx| self.poll_recv(cx)).await
        }

        fn poll_recv(&self, cx: &mut Context<'_>) -> Poll<Option<T>> {
            // Register the closure waker under the same lock as the closure
            // check so a sender drop racing with registration is observed.
            let all_senders_gone = self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.receivers_waiting.register(cx.waker());
                state.senders == 0
            });

            if let Ok(value) = self.inner.channel.try_receive() {
                self.inner
                    .state
                    .lock(|s| s.borrow_mut().senders_waiting.wake());
                return Poll::Ready(Some(value));
            }

            if all_senders_gone {
                return Poll::Ready(None);
            }

            match self.inner.channel.poll_receive(cx) {
                Poll::Ready(value) => {
                    self.inner
                        .state
                        .lock(|s| s.borrow_mut().senders_waiting.wake());
                    Poll::Ready(Some(value))
                }
                Poll::Pending => {
                    // A sender may have enqueued and dropped between the checks
                    // above; drain instead of parking on an empty closed queue.
                    if self.inner.state.lock(|s| s.borrow().senders) == 0 {
                        match self.inner.channel.try_receive() {
                            Ok(value) => {
                                self.inner
                                    .state
                                    .lock(|s| s.borrow_mut().senders_waiting.wake());
                                Poll::Ready(Some(value))
                            }
                            Err(_) => Poll::Ready(None),
                        }
                    } else {
                        Poll::Pending
                    }
                }
            }
        }
    }

    /// Create a bounded channel with the given capacity.
    ///
    /// The embassy backend stores the queue in a fixed `CHANNEL_CAPACITY`
    /// buffer, so `capacity` must not exceed it.  The channel state is
    /// reference-counted and freed once all handles are dropped.
    pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
        assert!(
            capacity <= CHANNEL_CAPACITY,
            "saikuro-exec: mpsc capacity {capacity} exceeds the fixed \
             embassy capacity {CHANNEL_CAPACITY}"
        );
        let inner = Arc::new(ChannelInner {
            state: CriticalSectionMutex::new(RefCell::new(ChannelState::new())),
            channel: EmbChannel::new(),
        });
        inner.state.lock(|s| {
            let mut state = s.borrow_mut();
            state.senders = 1;
            state.receivers = 1;
        });
        (
            Sender {
                inner: inner.clone(),
            },
            Receiver { inner },
        )
    }
}

// oneshot

/// Single-value channel used to return one response to one caller.
pub mod oneshot {
    use super::*;

    enum State<T> {
        Empty,
        Waiting(Waker),
        Ready(T),
        Closed,
    }

    struct InnerData<T> {
        channel: State<T>,
        receiver_alive: bool,
    }

    struct Inner<T> {
        state: CriticalSectionMutex<RefCell<InnerData<T>>>,
    }

    /// Sending half of a one-shot channel.  Not cloneable; `send` consumes it.
    pub struct Sender<T> {
        inner: Arc<Inner<T>>,
    }

    impl<T> Sender<T> {
        /// Deliver `value`, returning it if the receiver was already dropped.
        pub fn send(self, value: T) -> Result<(), T> {
            self.inner.state.lock(|s| {
                let mut data = s.borrow_mut();
                if !data.receiver_alive {
                    return Err(value);
                }
                match core::mem::replace(&mut data.channel, State::Empty) {
                    State::Empty => data.channel = State::Ready(value),
                    State::Waiting(waker) => {
                        data.channel = State::Ready(value);
                        waker.wake();
                    }
                    State::Ready(v) => {
                        data.channel = State::Ready(v);
                        core::unreachable!("oneshot sender cannot send twice");
                    }
                    State::Closed => {
                        data.channel = State::Closed;
                        core::unreachable!("oneshot sender cannot send on a closed channel");
                    }
                }
                Ok(())
            })
        }
    }

    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| {
                let mut data = s.borrow_mut();
                if matches!(data.channel, State::Ready(_)) {
                    // The value was delivered; keep it available to the receiver.
                    return;
                }
                let old = core::mem::replace(&mut data.channel, State::Closed);
                if let State::Waiting(waker) = old {
                    waker.wake();
                }
            });
        }
    }

    /// Error returned by the receiver when the sender is dropped without
    /// sending a value.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RecvError;

    impl core::fmt::Display for RecvError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("oneshot receiver closed")
        }
    }

    /// Receiving half of a one-shot channel.  Awaits the single value.
    pub struct Receiver<T> {
        inner: Arc<Inner<T>>,
    }

    impl<T> Drop for Receiver<T> {
        fn drop(&mut self) {
            self.inner
                .state
                .lock(|s| s.borrow_mut().receiver_alive = false);
        }
    }

    impl<T> Future for Receiver<T> {
        type Output = Result<T, RecvError>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.get_mut().inner.state.lock(|s| {
                let mut data = s.borrow_mut();
                match core::mem::replace(&mut data.channel, State::Empty) {
                    State::Ready(value) => Poll::Ready(Ok(value)),
                    State::Closed => Poll::Ready(Err(RecvError)),
                    State::Empty => {
                        data.channel = State::Waiting(cx.waker().clone());
                        Poll::Pending
                    }
                    State::Waiting(w) => {
                        if w.will_wake(cx.waker()) {
                            data.channel = State::Waiting(w);
                        } else {
                            data.channel = State::Waiting(cx.waker().clone());
                            w.wake();
                        }
                        Poll::Pending
                    }
                }
            })
        }
    }

    /// Create a one-shot channel.  The channel state is reference-counted and
    /// freed once both handles are dropped.
    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(Inner {
            state: CriticalSectionMutex::new(RefCell::new(InnerData {
                channel: State::Empty,
                receiver_alive: true,
            })),
        });
        (
            Sender {
                inner: inner.clone(),
            },
            Receiver { inner },
        )
    }
}

// sync
pub mod sync {
    use super::*;

    /// Async mutual-exclusion lock.
    pub use embassy_sync::mutex::Mutex;

    /// Read/write lock.
    ///
    /// Backed by a single async `embassy_sync::mutex::Mutex`.  Readers are
    /// serialized with writers rather than running concurrently; this is a safe
    /// subset of the tokio semantics.  Guards deref to the guarded value.
    pub struct RwLock<T> {
        inner: embassy_sync::mutex::Mutex<CriticalSectionRawMutex, T>,
    }

    impl<T> RwLock<T> {
        pub const fn new(value: T) -> Self {
            RwLock {
                inner: embassy_sync::mutex::Mutex::new(value),
            }
        }

        /// Acquire a shared read guard.
        pub async fn read(&self) -> RwLockReadGuard<'_, T> {
            RwLockReadGuard {
                guard: self.inner.lock().await,
            }
        }

        /// Acquire an exclusive write guard.
        pub async fn write(&self) -> RwLockWriteGuard<'_, T> {
            RwLockWriteGuard {
                guard: self.inner.lock().await,
            }
        }
    }

    pub struct RwLockReadGuard<'a, T> {
        guard: embassy_sync::mutex::MutexGuard<'a, CriticalSectionRawMutex, T>,
    }

    impl<T> core::ops::Deref for RwLockReadGuard<'_, T> {
        type Target = T;
        fn deref(&self) -> &T {
            &self.guard
        }
    }

    pub struct RwLockWriteGuard<'a, T> {
        guard: embassy_sync::mutex::MutexGuard<'a, CriticalSectionRawMutex, T>,
    }

    impl<T> core::ops::Deref for RwLockWriteGuard<'_, T> {
        type Target = T;
        fn deref(&self) -> &T {
            &self.guard
        }
    }

    impl<T> core::ops::DerefMut for RwLockWriteGuard<'_, T> {
        fn deref_mut(&mut self) -> &mut T {
            &mut self.guard
        }
    }

    /// Waker slots for tasks parked at a barrier.
    const MAX_BARRIER_WAITERS: usize = 16;

    /// Synchronization barrier that releases `n` tasks together.
    pub struct Barrier {
        inner: Arc<BarrierInner>,
    }

    struct BarrierInner {
        state: CriticalSectionMutex<RefCell<BarrierState>>,
    }

    struct BarrierState {
        count: usize,
        arrived: usize,
        generation: u64,
        waiting: MultiWakerRegistration<MAX_BARRIER_WAITERS>,
    }

    impl Barrier {
        pub fn new(n: usize) -> Self {
            assert!(n > 0, "saikuro-exec: Barrier::new requires n > 0");
            let inner = Arc::new(BarrierInner {
                state: CriticalSectionMutex::new(RefCell::new(BarrierState {
                    count: n,
                    arrived: 0,
                    generation: 0,
                    waiting: MultiWakerRegistration::new(),
                })),
            });
            Barrier { inner }
        }

        /// Wait until all `n` tasks have called `wait`.  Returns immediately for
        /// the task that releases the barrier.
        pub async fn wait(&self) {
            let released = self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.arrived += 1;
                if state.arrived == state.count {
                    state.arrived = 0;
                    state.generation += 1;
                    state.waiting.wake();
                    true
                } else {
                    false
                }
            });
            if released {
                return;
            }
            let mut gen = self.inner.state.lock(|s| s.borrow().generation);
            poll_fn(move |cx| {
                self.inner.state.lock(|s| {
                    let mut state = s.borrow_mut();
                    if state.generation != gen {
                        gen = state.generation;
                        Poll::Ready(())
                    } else {
                        state.waiting.register(cx.waker());
                        if state.generation != gen {
                            gen = state.generation;
                            Poll::Ready(())
                        } else {
                            Poll::Pending
                        }
                    }
                })
            })
            .await
        }
    }
}

// signal / watch / net / runtime
pub mod signal {
    pub async fn ctrl_c() -> Result<(), core::convert::Infallible> {
        core::future::pending().await
    }
}

/// Watch channel: a shared value with change notification.
pub mod watch {
    use super::*;

    /// Waker slots for receivers blocked in [`Receiver::changed`].
    ///
    /// `MultiWakerRegistration` falls back to waking every registered waker
    /// when this fills up, so this is a performance knob, not a hard limit.
    const MAX_WAITING_RECEIVERS: usize = 16;

    /// Error returned by [`Sender::send`] when there are no receivers left.
    #[derive(Debug)]
    pub struct SendError<T>(pub T);

    impl<T> SendError<T> {
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> core::fmt::Display for SendError<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("watch channel has no receivers")
        }
    }

    /// Error returned by `changed` once all senders have been dropped.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RecvError;

    impl core::fmt::Display for RecvError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("watch channel closed")
        }
    }

    struct WatchState<T> {
        value: T,
        version: u64,
        senders: usize,
        receivers: usize,
        waiting: MultiWakerRegistration<MAX_WAITING_RECEIVERS>,
    }

    struct WatchInner<T> {
        state: CriticalSectionMutex<RefCell<WatchState<T>>>,
    }

    /// Sending half of a watch channel.  Cloneable; the channel closes for
    /// receivers when the last sender is dropped.
    pub struct Sender<T> {
        inner: Arc<WatchInner<T>>,
    }

    impl<T: Clone> Sender<T> {
        /// Publish `value`, returning it if there are no receivers left.
        pub fn send(&self, value: T) -> Result<(), SendError<T>> {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                if state.receivers == 0 {
                    return Err(SendError(value));
                }
                state.value = value;
                state.version += 1;
                state.waiting.wake();
                Ok(())
            })
        }
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            self.inner.state.lock(|s| s.borrow_mut().senders += 1);
            Sender {
                inner: self.inner.clone(),
            }
        }
    }

    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.senders -= 1;
                if state.senders == 0 {
                    // Receivers parked in changed() observe the closure.
                    state.waiting.wake();
                }
            });
        }
    }

    /// Receiving half of a watch channel.  Cloneable; each clone tracks its own
    /// observed version.
    pub struct Receiver<T> {
        inner: Arc<WatchInner<T>>,
        version: u64,
    }

    impl<T: Clone> Receiver<T> {
        /// Snapshot of the latest value.
        ///
        /// The value is cloned rather than borrowed, matching the wasm backend;
        /// this avoids holding a critical section across the returned borrow.
        pub fn borrow(&self) -> T {
            self.inner.state.lock(|s| s.borrow().value.clone())
        }

        /// Future that completes when a new value is sent, or with `Err` once
        /// all senders are dropped.
        pub fn changed(&mut self) -> ChangedFuture<'_, T> {
            ChangedFuture { receiver: self }
        }
    }

    impl<T> Clone for Receiver<T> {
        fn clone(&self) -> Self {
            self.inner.state.lock(|s| s.borrow_mut().receivers += 1);
            Receiver {
                inner: self.inner.clone(),
                version: self.version,
            }
        }
    }

    impl<T> Drop for Receiver<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| s.borrow_mut().receivers -= 1);
        }
    }

    /// Future returned by [`Receiver::changed`].
    pub struct ChangedFuture<'a, T> {
        receiver: &'a mut Receiver<T>,
    }

    impl<T> Future for ChangedFuture<'_, T> {
        type Output = Result<(), RecvError>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            this.receiver.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                // Register before checking so a send that races with the
                // registration is not missed.
                state.waiting.register(cx.waker());
                if state.senders == 0 {
                    return Poll::Ready(Err(RecvError));
                }
                let version = state.version;
                if this.receiver.version != version {
                    this.receiver.version = version;
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        }
    }

    /// Create a watch channel seeded with `initial`.  The channel state is
    /// reference-counted and freed once all handles are dropped.
    pub fn channel<T: Clone>(initial: T) -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(WatchInner {
            state: CriticalSectionMutex::new(RefCell::new(WatchState {
                value: initial,
                version: 0,
                senders: 1,
                receivers: 1,
                waiting: MultiWakerRegistration::new(),
            })),
        });
        let receiver = Receiver {
            inner: inner.clone(),
            version: 0,
        };
        (Sender { inner }, receiver)
    }
}

pub mod net {
    // Empty. networking on embedded uses embassy-net, not tokio::net.
}

pub mod runtime {
    pub struct Builder {
        _private: (),
    }

    pub struct Runtime {
        _private: (),
    }

    impl Builder {
        pub fn new_current_thread() -> Self {
            Builder { _private: () }
        }

        pub fn enable_all(self) -> Self {
            self
        }

        pub fn build(self) -> Result<Runtime, super::RuntimeBuildError> {
            Ok(Runtime { _private: () })
        }
    }

    impl Runtime {
        pub fn block_on<F: core::future::Future>(&self, _future: F) -> F::Output {
            panic!("saikuro-exec: runtime::Runtime::block_on is not available on embassy-runtime")
        }
    }
}
