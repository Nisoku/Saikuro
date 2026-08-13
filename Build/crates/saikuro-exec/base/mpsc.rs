// mpsc

use super::*;
use crate::ChannelCapacity;
pub use crate::shared::mpsc::{SendError, TrySendError};

    pub const CHANNEL_CAPACITY: usize = 256;
    const MAX_WAITING_SENDERS: usize = 16;

    struct ChannelState {
        capacity: usize,
        senders: usize,
        receivers: usize,
        senders_waiting: MultiWakerRegistration<MAX_WAITING_SENDERS>,
        receivers_waiting: MultiWakerRegistration<1>,
    }

    impl ChannelState {
        const fn new(capacity: usize) -> Self {
            ChannelState {
                capacity,
                senders: 0,
                receivers: 0,
                senders_waiting: MultiWakerRegistration::new(),
                receivers_waiting: MultiWakerRegistration::new(),
            }
        }
    }

    struct ChannelInner<T> {
        state: CriticalSectionMutex<RefCell<ChannelState>>,
        channel: EmbChannel<CriticalSectionRawMutex, T, CHANNEL_CAPACITY>,
    }

    pub struct Sender<T> {
        inner: Arc<ChannelInner<T>>,
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            self.inner.state.lock(|s| s.borrow_mut().senders += 1);
            Sender {
                inner: self.inner.clone(),
            }
        }
    }

    impl<T> Drop for Sender<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.senders -= 1;
                if state.senders == 0 {
                    state.receivers_waiting.wake();
                }
            });
        }
    }

    enum EnqueueOutcome<T> {
        Sent,
        Full(T),
        Disconnected(T),
    }

    impl<T> Sender<T> {
        pub fn is_closed(&self) -> bool {
            self.inner.state.lock(|s| s.borrow().receivers == 0)
        }

        fn enqueue(&self, value: T) -> EnqueueOutcome<T> {
            self.inner.state.lock(|s| {
                let state = s.borrow_mut();
                if state.receivers == 0 {
                    return EnqueueOutcome::Disconnected(value);
                }
                if self.inner.channel.len() >= state.capacity {
                    return EnqueueOutcome::Full(value);
                }
                match self.inner.channel.try_send(value) {
                    Ok(()) => EnqueueOutcome::Sent,
                    Err(EmbTrySendError::Full(value)) => EnqueueOutcome::Full(value),
                }
            })
        }

        fn has_capacity(&self) -> bool {
            self.inner.state.lock(|s| {
                let state = s.borrow();
                self.inner.channel.len() < state.capacity
            })
        }

        pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
            match self.enqueue(value) {
                EnqueueOutcome::Sent => Ok(()),
                EnqueueOutcome::Full(value) => Err(TrySendError::Full(value)),
                EnqueueOutcome::Disconnected(value) => Err(TrySendError::Disconnected(value)),
            }
        }

        pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
            let mut pending = Some(value);
            poll_fn(move |cx| {
                loop {
                    if self.is_closed() {
                        let message = pending.take().expect("mpsc send message restored on Full path");
                        return Poll::Ready(Err(SendError(message)));
                    }
                    let message = pending.take().expect("mpsc send message restored on Full path");
                    match self.enqueue(message) {
                        EnqueueOutcome::Sent => return Poll::Ready(Ok(())),
                        EnqueueOutcome::Disconnected(message) => {
                            return Poll::Ready(Err(SendError(message)))
                        }
                        EnqueueOutcome::Full(message) => {
                            pending = Some(message);
                            self.inner
                                .state
                                .lock(|s| s.borrow_mut().senders_waiting.register(cx.waker()));
                            if self.is_closed() {
                                let message = pending.take().expect("mpsc send message restored on Full path");
                                return Poll::Ready(Err(SendError(message)));
                            }
                            if self.has_capacity() {
                                continue;
                            }
                            return Poll::Pending;
                        }
                    }
                }
            })
            .await
        }
    }

    pub struct Receiver<T> {
        inner: Arc<ChannelInner<T>>,
    }

    impl<T> Drop for Receiver<T> {
        fn drop(&mut self) {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.receivers -= 1;
                if state.receivers == 0 {
                    state.senders_waiting.wake();
                }
            });
        }
    }

    impl<T> Receiver<T> {
        pub async fn recv(&mut self) -> Option<T> {
            poll_fn(|cx| self.poll_recv(cx)).await
        }

        fn poll_recv(&self, cx: &mut Context<'_>) -> Poll<Option<T>> {
            let all_senders_gone = self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                state.receivers_waiting.register(cx.waker());
                state.senders == 0
            });

            if let Ok(value) = self.inner.channel.try_receive() {
                self.inner.state.lock(|s| s.borrow_mut().senders_waiting.wake());
                return Poll::Ready(Some(value));
            }

            if all_senders_gone {
                return Poll::Ready(None);
            }

            match self.inner.channel.poll_receive(cx) {
                Poll::Ready(value) => {
                    self.inner.state.lock(|s| s.borrow_mut().senders_waiting.wake());
                    Poll::Ready(Some(value))
                }
                Poll::Pending => {
                    if self.inner.state.lock(|s| s.borrow().senders) == 0 {
                        match self.inner.channel.try_receive() {
                            Ok(value) => {
                                self.inner.state.lock(|s| s.borrow_mut().senders_waiting.wake());
                                Poll::Ready(Some(value))
                            }
                            Err(_) => Poll::Ready(None),
                        }
                    } else {
                        Poll::Pending
                    }
                }
            }
        }
    }

    pub fn channel<T>(capacity: ChannelCapacity) -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(ChannelInner {
            state: CriticalSectionMutex::new(RefCell::new(ChannelState::new(capacity.get()))),
            channel: EmbChannel::new(),
        });
        inner.state.lock(|s| {
            let mut state = s.borrow_mut();
            state.senders = 1;
            state.receivers = 1;
        });
        (
            Sender { inner: inner.clone() },
            Receiver { inner },
        )
    }

