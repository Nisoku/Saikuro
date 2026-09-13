#[cfg(any(feature = "no_std", feature = "embedded", feature = "wasm"))]
pub(crate) mod block_on;

pub mod heap_stats;
pub(crate) mod join;
pub(crate) mod queue;
pub(crate) mod runtime;
pub(crate) mod spawn;

#[cfg(target_has_atomic = "ptr")]
pub(crate) use crate::Arc;
use core::cell::RefCell;
use core::future::{poll_fn, Future};
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
#[cfg(any(feature = "no_std", feature = "embedded"))]
use core::time::Duration;
#[cfg(not(target_has_atomic = "ptr"))]
pub(crate) use portable_atomic_util::Arc;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
#[cfg(any(feature = "no_std", feature = "embedded"))]
use embassy_time::{Duration as EmbDuration, Timer};
use futures::future::{Fuse, FutureExt};

#[cfg(any(feature = "no_std", feature = "embedded"))]
use crate::shared::TimeoutError;

pub use embassy_futures::yield_now;

/// Heap-backed list of wakers with embassy `MultiWakerRegistration`
pub struct WakerList<const N: usize> {
    wakers: alloc::vec::Vec<Waker>,
}

impl<const N: usize> WakerList<N> {
    pub const fn new() -> Self {
        Self {
            wakers: alloc::vec::Vec::new(),
        }
    }

    /// Register a waker, deduplicating against an existing waker for the same
    /// task. When `N` distinct waiters are already registered, wake them all
    /// and reregister.
    pub fn register(&mut self, w: &Waker) {
        for existing in self.wakers.iter() {
            if w.will_wake(existing) {
                return;
            }
        }
        if self.wakers.len() >= N {
            self.wake();
        }
        self.wakers.push(w.clone());
    }

    /// Wake every registered waker and clear the list.
    pub fn wake(&mut self) {
        let wakers = core::mem::take(&mut self.wakers);
        for w in wakers {
            w.wake();
        }
    }
}

impl<const N: usize> Default for WakerList<N> {
    fn default() -> Self {
        Self::new()
    }
}

// The native and wasm engines override `sleep`/`timeout` with their own time
// drivers, so base's embassy-time versions exist only on embedded targets.
#[cfg(any(feature = "no_std", feature = "embedded"))]
fn emb_duration(dur: Duration) -> EmbDuration {
    EmbDuration::from_micros(dur.as_micros().min(u64::MAX as u128) as u64)
}

#[cfg(any(feature = "no_std", feature = "embedded"))]
pub async fn sleep(dur: Duration) {
    Timer::after(emb_duration(dur)).await;
}

#[cfg(any(feature = "no_std", feature = "embedded"))]
pub async fn timeout<F, T>(dur: Duration, fut: F) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    match embassy_futures::select::select(fut, Timer::after(emb_duration(dur))).await {
        embassy_futures::select::Either::First(res) => Ok(res),
        embassy_futures::select::Either::Second(_) => Err(TimeoutError),
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
