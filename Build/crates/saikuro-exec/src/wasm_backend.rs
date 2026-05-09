//! WASM backend for saikuro-exec.
//!
//! Provides the same public API as the tokio backend, but backed by
//! `futures` channels and `wasm_bindgen_futures` for single-threaded WASM
//! execution. The mpsc/types are wrapped so that callers see a tokio-
//! compatible API (e.g. `recv()` returns `Option<T>`, `send()` is inherent).

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::{mpsc as inner_mpsc, oneshot as inner_oneshot};

/// Spawn a future on the JS/WASM executor
pub fn spawn<F, T>(fut: F) -> JoinHandle<T>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let (tx, rx) = inner_oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let res = fut.await;
        let _ = tx.send(res);
    });
    JoinHandle { rx }
}

// Wrapped mpsc
pub mod mpsc {
    use std::error::Error;
    use std::fmt;
    use std::sync::{Arc, Mutex};

    use futures::stream::StreamExt;

    use super::inner_mpsc;

    pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = inner_mpsc::channel(buffer);
        (
            Sender {
                inner: Arc::new(Mutex::new(tx)),
            },
            Receiver { inner: rx },
        )
    }

    pub struct Sender<T> {
        inner: Arc<Mutex<inner_mpsc::Sender<T>>>,
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            Sender {
                inner: self.inner.clone(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SendError;

    impl fmt::Display for SendError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "channel closed")
        }
    }

    impl Error for SendError {}

    impl<T: 'static> Sender<T> {
        pub async fn send(&self, value: T) -> Result<(), SendError> {
            let mut to_send = Some(value);
            loop {
                let (ok, retry_value) = {
                    let mut sender = self.inner.lock().unwrap();
                    match sender.try_send(to_send.take().unwrap()) {
                        Ok(()) => (true, None),
                        Err(e) => {
                            if sender.is_closed() {
                                return Err(SendError);
                            }
                            (false, Some(e.into_inner()))
                        }
                    }
                };
                if ok {
                    return Ok(());
                }
                to_send = retry_value;
                super::yield_now().await;
            }
        }

        pub fn is_closed(&self) -> bool {
            self.inner.lock().unwrap().is_closed()
        }
    }

    pub struct Receiver<T> {
        inner: inner_mpsc::Receiver<T>,
    }

    impl<T> Receiver<T> {
        pub async fn recv(&mut self) -> Option<T> {
            self.inner.next().await
        }
    }
}

// Wrapped oneshot
pub mod oneshot {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use super::inner_oneshot;

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = inner_oneshot::channel();
        (Sender { inner: tx }, Receiver { inner: rx })
    }

    pub struct Sender<T> {
        inner: inner_oneshot::Sender<T>,
    }

    impl<T> Sender<T> {
        pub fn send(self, value: T) -> Result<(), T> {
            self.inner.send(value)
        }
    }

    pub struct Receiver<T> {
        inner: inner_oneshot::Receiver<T>,
    }

    impl<T> Future for Receiver<T> {
        type Output = Result<T, ()>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            Pin::new(&mut self.inner).poll(cx).map_err(|_| ())
        }
    }
}

// Wrapped sync
pub mod sync {
    pub use futures::lock::Mutex;
}

// Wrapped signal
/// TODO: Implement signal handling for WASM
pub mod signal {
    pub async fn ctrl_c() -> Result<(), ()> {
        Err(())
    }
}

// Wrapped watch (waker-based)
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
        wakers: Vec<Waker>,
    }

    pub struct Sender<T> {
        inner: Arc<Mutex<Inner<T>>>,
    }

    impl<T: Clone> Sender<T> {
        pub fn send(&self, val: T) {
            let mut inner = self.inner.lock().unwrap();
            inner.value = val;
            inner.changed = true;
            for w in inner.wakers.drain(..) {
                w.wake();
            }
        }
    }

    pub struct Receiver<T> {
        inner: Arc<Mutex<Inner<T>>>,
    }

    impl<T: Clone> Receiver<T> {
        pub fn borrow(&self) -> T {
            self.inner.lock().unwrap().value.clone()
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
            let mut inner = self.receiver.inner.lock().unwrap();
            if inner.changed {
                inner.changed = false;
                return Poll::Ready(Ok(()));
            }
            inner.wakers.push(cx.waker().clone());
            if inner.changed {
                inner.wakers.pop();
                inner.changed = false;
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
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
pub struct JoinHandle<T> {
    rx: inner_oneshot::Receiver<T>,
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match Pin::new(&mut this.rx).poll(cx) {
            Poll::Ready(Ok(v)) => Poll::Ready(v),
            Poll::Ready(Err(_)) => panic!("JoinHandle dropped the sender without sending a value"),
            Poll::Pending => Poll::Pending,
        }
    }
}

// Time utilities
pub async fn sleep(dur: Duration) {
    let _ = wasm_timer::Delay::new(dur).await;
}

pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    use futures::future::{Either, FutureExt};
    let delay = wasm_timer::Delay::new(dur).fuse();
    futures::pin_mut!(delay);
    futures::pin_mut!(fut);
    match futures::future::select(fut, delay).await {
        Either::Left((res, _)) => Ok(res),
        Either::Right((_, _)) => Err(()),
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

pub fn block_on<F>(_future: F) -> F::Output
where
    F: Future + 'static,
{
    // TODO: implement a single-threaded block_on
    panic!("block_on is not supported on wasm-runtime yet")
}
