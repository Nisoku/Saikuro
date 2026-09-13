use alloc::boxed::Box;
use alloc::vec::Vec;
use core::future::Future;
use core::pin::Pin;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

pub(crate) type BoxedFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

struct QueueWrapper(
    embassy_sync::blocking_mutex::CriticalSectionMutex<core::cell::RefCell<Vec<BoxedFuture>>>,
);

// SAFETY: access is serialized through CriticalSectionRawMutex.
unsafe impl Sync for QueueWrapper {}

static QUEUE: QueueWrapper = QueueWrapper(embassy_sync::blocking_mutex::CriticalSectionMutex::new(
    core::cell::RefCell::new(Vec::new()),
));

pub(crate) static NOTIFY: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub(crate) fn queue() -> &'static embassy_sync::blocking_mutex::CriticalSectionMutex<
    core::cell::RefCell<Vec<BoxedFuture>>,
> {
    &QUEUE.0
}

/// Minimal `FuturesUnordered` for targets without `target_has_atomic = "ptr"`.
///
/// Polls all contained futures on every waker notification.
#[cfg(not(target_has_atomic = "ptr"))]
pub(crate) mod no_atomic_futures {
    use alloc::vec::Vec;
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    use futures::stream::Stream;

    pub struct FuturesUnordered<F> {
        futures: Vec<F>,
    }

    impl<F> FuturesUnordered<F> {
        pub fn new() -> Self {
            Self {
                futures: Vec::new(),
            }
        }

        pub fn push(&mut self, f: F) {
            self.futures.push(f);
        }

        pub fn is_empty(&self) -> bool {
            self.futures.is_empty()
        }
    }

    impl<F: Future> Stream for FuturesUnordered<F> {
        type Item = F::Output;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let this = unsafe { self.get_unchecked_mut() };
            let mut i = this.futures.len();
            while i > 0 {
                i -= 1;
                if let Poll::Ready(output) =
                    unsafe { Pin::new_unchecked(&mut this.futures[i]) }.poll(cx)
                {
                    this.futures.swap_remove(i);
                    return Poll::Ready(Some(output));
                }
            }
            Poll::Pending
        }
    }
}
