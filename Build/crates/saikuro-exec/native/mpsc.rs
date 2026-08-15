use crate::shared::mpsc::{SendError, TrySendError};
use crate::ChannelCapacity;

pub struct Sender<T> {
    inner: tokio::sync::mpsc::Sender<T>,
}

pub struct Receiver<T> {
    inner: tokio::sync::mpsc::Receiver<T>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Sender<T> {
    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.inner
            .send(value)
            .await
            .map_err(|e| SendError(e.into_inner()))
    }

    pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
        self.inner.try_send(value).map_err(|e| match e {
            tokio::sync::mpsc::TrySendError::Full(v) => TrySendError::Full(v),
            tokio::sync::mpsc::TrySendError::Closed(v) => TrySendError::Disconnected(v),
        })
    }

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

impl<T> Receiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        self.inner.recv().await
    }
}

pub fn channel<T>(capacity: ChannelCapacity) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::mpsc::channel(capacity.get());
    (Sender { inner: tx }, Receiver { inner: rx })
}
