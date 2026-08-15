use std::future::Future;
use std::time::Duration;

use tokio::runtime::{Builder, Runtime as TokioRuntime};
use tokio::task::{JoinError as TokioJoinError, JoinHandle as TokioJoinHandle};

use crate::shared::JoinError;

pub use tokio::signal;

pub fn new_runtime() -> Runtime {
    Runtime::new()
}

pub struct Runtime {
    inner: TokioRuntime,
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            inner: Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("saikuro-exec: failed to build tokio runtime"),
        }
    }

    pub fn new_multi_thread() -> Self {
        Self::new()
    }

    pub fn new_current_thread() -> Self {
        Runtime {
            inner: Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("saikuro-exec: failed to build tokio runtime"),
        }
    }

    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        self.inner.block_on(fut)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RuntimeBuilder {
    inner: Builder,
}

impl RuntimeBuilder {
    pub fn new_multi_thread() -> Self {
        RuntimeBuilder {
            inner: Builder::new_multi_thread(),
        }
    }

    pub fn new_current_thread() -> Self {
        RuntimeBuilder {
            inner: Builder::new_current_thread(),
        }
    }

    pub fn worker_threads(mut self, n: usize) -> Self {
        self.inner.worker_threads(n);
        self
    }

    pub fn enable_all(mut self) -> Self {
        self.inner.enable_all();
        self
    }

    pub fn build(self) -> Runtime {
        Runtime {
            inner: self
                .inner
                .build()
                .expect("saikuro-exec: failed to build tokio runtime"),
        }
    }
}

pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    JoinHandle {
        inner: tokio::spawn(fut),
    }
}

impl<T> JoinHandle<T> {
    pub async fn abort(&self) {
        self.inner.abort();
    }

    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll(cx) {
            core::task::Poll::Ready(Ok(v)) => core::task::Poll::Ready(Ok(v)),
            core::task::Poll::Ready(Err(e)) => {
                core::task::Poll::Ready(Err(JoinError::from_tokio(e)))
            }
            core::task::Poll::Pending => core::task::Poll::Pending,
        }
    }
}

impl JoinError {
    fn from_tokio(e: TokioJoinError) -> Self {
        if e.is_cancelled() {
            JoinError::cancelled()
        } else {
            JoinError::panic()
        }
    }
}

pub async fn sleep(dur: Duration) {
    tokio::time::sleep(dur).await;
}

pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    match tokio::time::timeout(dur, fut).await {
        Ok(v) => Ok(v),
        Err(_) => Err(()),
    }
}

pub async fn yield_now() {
    tokio::task::yield_now().await;
}
