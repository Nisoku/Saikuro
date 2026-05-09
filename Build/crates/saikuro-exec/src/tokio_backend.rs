use std::future::Future;
use std::time::Duration;

pub type JoinHandle<T> = tokio::task::JoinHandle<T>;
pub type Runtime = tokio::runtime::Runtime;
pub type RuntimeBuilder = tokio::runtime::Builder;

pub fn new_runtime() -> RuntimeBuilder {
    tokio::runtime::Builder::new_multi_thread()
}

pub fn spawn<F, T>(fut: F) -> JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    tokio::spawn(fut)
}

pub mod mpsc {
    pub use tokio::sync::mpsc::{channel, Receiver, Sender};
}

pub mod oneshot {
    pub use tokio::sync::oneshot::{channel, Receiver, Sender};
}

pub mod sync {
    pub use tokio::sync::{Barrier, Mutex, RwLock};
}

pub mod net {
    pub use tokio::net::*;
}

pub mod signal {
    pub use tokio::signal::*;
}

pub mod watch {
    pub use tokio::sync::watch::{channel, Receiver, Sender};
}

pub mod runtime {
    pub use tokio::runtime::{Builder, Runtime};
}

pub fn block_on<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
        .block_on(future)
}

pub async fn sleep(dur: Duration) {
    tokio::time::sleep(dur).await
}

pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, tokio::time::error::Elapsed>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(dur, fut).await
}

pub async fn yield_now() {
    tokio::task::yield_now().await
}

pub use tokio_util;

// Re-export select macro via a thin wrapper macro in crate root if needed.
