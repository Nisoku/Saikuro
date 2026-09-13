use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::shared::oneshot::RecvError;

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    (Sender { inner: tx }, Receiver { inner: rx })
}

pub struct Sender<T> {
    inner: tokio::sync::oneshot::Sender<T>,
}

impl<T> Sender<T> {
    pub fn send(self, value: T) -> Result<(), T> {
        self.inner.send(value)
    }
}

pub struct Receiver<T> {
    inner: tokio::sync::oneshot::Receiver<T>,
}

impl<T> Future for Receiver<T> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll(cx) {
            Poll::Ready(Ok(v)) => Poll::Ready(Ok(v)),
            Poll::Ready(Err(_)) => Poll::Ready(Err(RecvError)),
            Poll::Pending => Poll::Pending,
        }
    }
}
