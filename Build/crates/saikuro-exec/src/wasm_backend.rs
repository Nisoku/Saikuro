//! WASM backend for saikuro-exec.
//!
//! Backed by `futures` channels and `wasm_bindgen_futures` for single-threaded
//! WASM execution. This is the default backend when compiling to WASM.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::oneshot as inner_oneshot;

/// Spawn a future on the JS/WASM executor.
pub fn spawn<F, T>(fut: F) -> JoinHandle<T>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let (tx, rx) = inner_oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = tx.send(fut.await);
    });
    JoinHandle { rx }
}

pub mod mpsc {
    use futures::channel::mpsc as inner;
    use futures::lock::Mutex;
    use futures::stream::StreamExt;
    use std::sync::Arc;

    #[derive(Debug)]
    pub struct SendError<T>(pub T);

    impl<T> SendError<T> {
        pub fn into_inner(self) -> T {
            self.0
        }
        pub fn is_disconnected(&self) -> bool {
            true
        }
    }

    impl<T> std::fmt::Display for SendError<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "send failed: channel is disconnected")
        }
    }

    impl<T: std::fmt::Debug> std::error::Error for SendError<T> {}

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

    impl<T> std::fmt::Debug for TrySendError<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TrySendError::Full(_) => write!(f, "TrySendError::Full(..)"),
                TrySendError::Disconnected(_) => write!(f, "TrySendError::Disconnected(..)"),
            }
        }
    }

    impl<T> std::fmt::Display for TrySendError<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TrySendError::Full(_) => write!(f, "send failed: channel is full"),
                TrySendError::Disconnected(_) => write!(f, "send failed: channel is disconnected"),
            }
        }
    }

    impl<T: std::fmt::Debug> std::error::Error for TrySendError<T> {}

    pub struct Sender<T> {
        inner: Arc<Mutex<inner::Sender<T>>>,
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            Sender {
                inner: self.inner.clone(),
            }
        }
    }

    pub struct Receiver<T> {
        inner: inner::Receiver<T>,
    }

    pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = inner::channel(buffer);
        (
            Sender {
                inner: Arc::new(Mutex::new(tx)),
            },
            Receiver { inner: rx },
        )
    }

    impl<T> Sender<T> {
        pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
            let mut value = value;
            loop {
                let mut guard = self.inner.lock().await;
                match guard.try_send(value) {
                    Ok(()) => return Ok(()),
                    Err(e) if e.is_full() => {
                        value = e.into_inner();
                        drop(guard);
                        super::yield_now().await;
                    }
                    Err(e) => {
                        return Err(SendError(e.into_inner()));
                    }
                }
            }
        }

        pub fn is_closed(&self) -> bool {
            if let Some(guard) = self.inner.try_lock() {
                guard.is_closed()
            } else {
                false
            }
        }

        pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
            if let Some(mut guard) = self.inner.try_lock() {
                guard.try_send(value).map_err(|e| {
                    if e.is_full() {
                        TrySendError::Full(e.into_inner())
                    } else {
                        TrySendError::Disconnected(e.into_inner())
                    }
                })
            } else {
                Err(TrySendError::Full(value))
            }
        }
    }

    impl<T> Receiver<T> {
        pub fn recv(&mut self) -> impl futures::future::FusedFuture<Output = Option<T>> + '_ {
            self.inner.next()
        }
    }
}

pub mod oneshot {
    pub use futures::channel::oneshot::{channel, Receiver, Sender};
}

pub mod sync {
    pub use futures::lock::Mutex;
}

/// Signal handling for WASM.
///
/// WASM has no OS signals, so `ctrl_c` never completes: it awaits
/// `std::future::pending()` so callers awaiting a shutdown signal simply
/// stay parked instead of triggering an immediate (spurious) shutdown.
pub mod signal {
    pub async fn ctrl_c() -> Result<(), ()> {
        std::future::pending().await
    }
}

// Watch channel
pub mod watch {
    use std::fmt;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    pub fn channel<T: Clone>(initial: T) -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(Mutex::new(Inner {
            value: initial,
            changed: false,
            closed: false,
            senders: 1,
            wakers: Vec::new(),
        }));
        (
            Sender {
                inner: inner.clone(),
            },
            Receiver { inner },
        )
    }

    struct Inner<T> {
        value: T,
        changed: bool,
        closed: bool,
        senders: usize,
        wakers: Vec<Waker>,
    }

    pub struct Sender<T> {
        inner: Arc<Mutex<Inner<T>>>,
    }

    impl<T: Clone> Sender<T> {
        pub fn send(&self, val: T) {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.value = val;
            inner.changed = true;
            for w in inner.wakers.drain(..) {
                w.wake();
            }
        }
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.senders += 1;
            drop(guard);
            Sender {
                inner: self.inner.clone(),
            }
        }
    }

    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.senders -= 1;
            if guard.senders == 0 {
                guard.closed = true;
                for w in guard.wakers.drain(..) {
                    w.wake();
                }
            }
        }
    }

    pub struct Receiver<T> {
        inner: Arc<Mutex<Inner<T>>>,
    }

    impl<T: Clone> Receiver<T> {
        pub fn borrow(&self) -> T {
            self.inner
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .value
                .clone()
        }

        pub fn changed(&mut self) -> ChangedFuture<'_, T> {
            ChangedFuture { receiver: self }
        }
    }

    pub struct ChangedFuture<'a, T> {
        receiver: &'a Receiver<T>,
    }

    impl<T: Clone> Future for ChangedFuture<'_, T> {
        type Output = Result<(), ()>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut inner = self
                .receiver
                .inner
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if inner.changed {
                inner.changed = false;
                Poll::Ready(Ok(()))
            } else if inner.closed {
                Poll::Ready(Err(()))
            } else {
                let waker = cx.waker();
                if !inner.wakers.iter().any(|w| w.will_wake(waker)) {
                    inner.wakers.push(waker.clone());
                }
                if inner.closed {
                    Poll::Ready(Err(()))
                } else if inner.changed {
                    inner.changed = false;
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            }
        }
    }

    impl<T> fmt::Debug for ChangedFuture<'_, T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("ChangedFuture").finish_non_exhaustive()
        }
    }
}

// JoinHandle

/// Error returned when a spawned task is cancelled (sender dropped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinError;

impl std::fmt::Display for JoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task was cancelled")
    }
}

impl std::error::Error for JoinError {}

pub struct JoinHandle<T> {
    rx: inner_oneshot::Receiver<T>,
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.get_mut().rx).poll(cx) {
            Poll::Ready(Ok(v)) => Poll::Ready(Ok(v)),
            Poll::Ready(Err(_)) => Poll::Ready(Err(JoinError)),
            Poll::Pending => Poll::Pending,
        }
    }
}

// Time utilities
pub async fn sleep(dur: Duration) {
    let _ = fluvio_wasm_timer::Delay::new(dur).await;
}

pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    use futures::future::{Either, FutureExt};
    futures::pin_mut!(fut);
    let delay = fluvio_wasm_timer::Delay::new(dur).fuse();
    futures::pin_mut!(delay);
    match futures::future::select(fut, delay).await {
        Either::Left((res, _)) => Ok(res),
        Either::Right(_) => Err(()),
    }
}

pub async fn yield_now() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if !yielded {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await
}

/// Run a future to completion on the current WASM thread.
///
/// This uses `futures::executor::block_on` which polls the future in a loop.
///
/// ## Important
///
/// On single-threaded WASM targets (no atomics) this **cannot** yield to the
/// JavaScript event loop, so futures that depend on JS I/O (timers,
/// `fetch`, `BroadcastChannel`, …) will never complete.  For those futures
/// use `spawn_local` with an async entrypoint instead.
///
/// On WASM targets with atomics enabled (`RUSTFLAGS="--cfg
/// target_feature=atomics"`), `block_on` uses `Atomics.wait()` for proper
/// blocking, which allows the JS event loop to make progress.
pub fn block_on<F>(future: F) -> F::Output
where
    F: Future + 'static,
{
    futures::executor::block_on(future)
}

/// Stub Runtime for wasm that mirrors tokio's runtime API so
/// `saikuro-c` (and other consumers) can use the same code
/// path on wasm32-unknown-unknown.
pub struct Runtime {
    _private: (),
}

/// Stub builder that always produces a Runtime successfully.
pub struct RuntimeBuilder {
    _private: (),
}

impl RuntimeBuilder {
    /// No-op: everything is already enabled on wasm.
    pub fn enable_all(self) -> Self {
        self
    }

    /// Always returns Ok(Runtime).
    pub fn build(self) -> Result<Runtime, Box<dyn std::error::Error>> {
        Ok(Runtime { _private: () })
    }
}

/// Create a new runtime builder for the wasm backend.
pub fn new_runtime() -> RuntimeBuilder {
    RuntimeBuilder { _private: () }
}

impl Runtime {
    /// Block on a future using the single-threaded wasm executor.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        futures::executor::block_on(future)
    }
}
