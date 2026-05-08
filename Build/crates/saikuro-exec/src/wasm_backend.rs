use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::{mpsc as futures_mpsc, oneshot as futures_oneshot};

/// Spawn a future on the JS/WASM executor (via `spawn_local`). Returns a
/// `JoinHandle` that implements `Future<Output = T>` so code can `.await` it.
pub fn spawn<F, T>(fut: F) -> JoinHandle<T>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let (tx, rx) = futures_oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let res = fut.await;
        let _ = tx.send(res);
    });
    JoinHandle { rx }
}

pub mod mpsc {
    pub use futures_mpsc::{channel, Receiver, Sender};
}

pub mod oneshot {
    pub use futures_oneshot::{channel, Receiver, Sender};
}

pub mod sync {
    pub use futures::lock::Mutex;
    pub use futures::lock::RwLock;
}

pub mod signal {
    pub async fn ctrl_c() -> Result<(), ()> {
        Err(())
    }
}

pub mod watch {
    pub use futures::channel::watch::{channel, Receiver, Sender};
}

pub struct JoinHandle<T> {
    rx: futures_oneshot::Receiver<T>,
}

impl<T> std::future::Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Delegate to the inner receiver (which returns Result<T, Canceled>)
        let this = self.get_mut();
        match Pin::new(&mut this.rx).poll(cx) {
            Poll::Ready(Ok(v)) => Poll::Ready(v),
            Poll::Ready(Err(_)) => panic!("JoinHandle canceled"),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub async fn sleep(dur: Duration) {
    // Use JS timers via wasm-bindgen-futures + setTimeout is more involved;
    // for now implement via a future that resolves after a small delay using
    // `gloo_timers` if available, but keep this lightweight placeholder.
    let _ = wasm_timer::Delay::new(dur).await;
}

pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    // Implement timeout using `wasm_timer` crate's Delay combined with futures::select
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
    futures::future::yield_now().await
}

pub fn block_on<F>(_future: F) -> F::Output
where
    F: Future + 'static,
{
    panic!("block_on is not supported on wasm-runtime yet")
}
