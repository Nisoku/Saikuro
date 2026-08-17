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
use futures::future::{Fuse, FutureExt};

pub use embassy_futures::yield_now;

fn emb_duration(dur: Duration) -> EmbDuration {
    EmbDuration::from_micros(dur.as_micros().min(u64::MAX as u128) as u64)
}

pub async fn sleep(dur: Duration) {
    Timer::after(emb_duration(dur)).await;
}

pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    match embassy_futures::select::select(fut, Timer::after(emb_duration(dur))).await {
        embassy_futures::select::Either::First(res) => Ok(res),
        embassy_futures::select::Either::Second(_) => Err(()),
    }
}

#[doc(hidden)]
pub fn fuse_select<F: Future>(fut: F) -> Fuse<F> {
    FutureExt::fuse(fut)
}

pub mod mpsc;
pub mod oneshot;
pub mod sync;
pub mod watch;

// Non-native engines share a `pending()`-based signal stub: there are no OS
// signals on wasm/embedded/no_std, so a shutdown signal simply never fires.
#[cfg(any(feature = "wasm", feature = "embedded", feature = "no_std"))]
pub mod signal {
    use core::convert::Infallible;

    pub async fn ctrl_c() -> Result<(), Infallible> {
        core::future::pending().await
    }
}

// Heap executor harness
#[cfg(any(feature = "wasm", feature = "no_std", feature = "embedded"))]
pub(crate) mod exec;
