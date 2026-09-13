use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::shared::watch::{RecvError, SendError};

pub fn channel<T: Clone>(initial: T) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::watch::channel(initial);
    (Sender { inner: tx }, Receiver { inner: rx })
}

pub struct Sender<T> {
    inner: tokio::sync::watch::Sender<T>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Clone> Sender<T> {
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.inner.send(value).map_err(|e| SendError(e.0))
    }
}

pub struct Receiver<T> {
    inner: tokio::sync::watch::Receiver<T>,
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Receiver {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Clone> Receiver<T> {
    pub fn borrow(&self) -> T {
        (*self.inner.borrow()).clone()
    }

    pub fn changed(&mut self) -> ChangedFuture<'_, T>
    where
        T: Send + Sync,
    {
        ChangedFuture {
            inner: Box::pin(self.inner.changed()),
            _marker: PhantomData,
        }
    }
}

pub struct ChangedFuture<'a, T> {
    inner:
        Pin<Box<dyn Future<Output = Result<(), tokio::sync::watch::error::RecvError>> + Send + 'a>>,
    _marker: PhantomData<fn(T)>,
}

impl<T> Unpin for ChangedFuture<'_, T> {}

impl<T> Future for ChangedFuture<'_, T> {
    type Output = Result<(), RecvError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this.inner.as_mut().poll(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(_)) => Poll::Ready(Err(RecvError)),
            Poll::Pending => Poll::Pending,
        }
    }
}
