// watch

use super::*;
pub use crate::shared::watch::{RecvError, SendError};

    const MAX_WAITING_RECEIVERS: usize = 16;

    struct WatchState<T> {
        value: T,
        version: u64,
        senders: usize,
        receivers: usize,
        waiting: MultiWakerRegistration<MAX_WAITING_RECEIVERS>,
    }

    struct WatchInner<T> {
        state: CriticalSectionMutex<RefCell<WatchState<T>>>,
    }

    pub struct Sender<T> {
        inner: Arc<WatchInner<T>>,
    }

    impl<T: Clone> Sender<T> {
        pub fn send(&self, value: T) -> Result<(), SendError<T>> {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                if state.receivers == 0 {
                    return Err(SendError(value));
                }
                state.value = value;
                state.version += 1;
                state.waiting.wake();
                Ok(())
            })
        }
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            self.inner.state.lock(|s| s.borrow_mut().senders += 1);
            Sender { inner: self.inner.clone() }
        }
    }

    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.senders -= 1;
                if state.senders == 0 {
                    state.waiting.wake();
                }
            });
        }
    }

    pub struct Receiver<T> {
        inner: Arc<WatchInner<T>>,
        version: u64,
    }

    impl<T: Clone> Receiver<T> {
        pub fn borrow(&self) -> T {
            self.inner.state.lock(|s| s.borrow().value.clone())
        }

        pub fn changed(&mut self) -> ChangedFuture<'_, T> {
            ChangedFuture { receiver: self }
        }
    }

    impl<T> Clone for Receiver<T> {
        fn clone(&self) -> Self {
            self.inner.state.lock(|s| s.borrow_mut().receivers += 1);
            Receiver {
                inner: self.inner.clone(),
                version: self.version,
            }
        }
    }

    impl<T> Drop for Receiver<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| s.borrow_mut().receivers -= 1);
        }
    }

    pub struct ChangedFuture<'a, T> {
        receiver: &'a mut Receiver<T>,
    }

    impl<T> Future for ChangedFuture<'_, T> {
        type Output = Result<(), RecvError>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            this.receiver.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.waiting.register(cx.waker());
                let version = state.version;
                if this.receiver.version != version {
                    this.receiver.version = version;
                    return Poll::Ready(Ok(()));
                }
                if state.senders == 0 {
                    return Poll::Ready(Err(RecvError));
                }
                Poll::Pending
            })
        }
    }

    pub fn channel<T: Clone>(initial: T) -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(WatchInner {
            state: CriticalSectionMutex::new(RefCell::new(WatchState {
                value: initial,
                version: 0,
                senders: 1,
                receivers: 1,
                waiting: MultiWakerRegistration::new(),
            })),
        });
        let receiver = Receiver {
            inner: inner.clone(),
            version: 0,
        };
        (Sender { inner }, receiver)
    }

