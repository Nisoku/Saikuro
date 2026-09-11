// mpsc

use alloc::collections::VecDeque;
use core::cell::RefCell;
use core::task::{Context, Poll};

use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::waitqueue::WakerRegistration;

use super::*;
pub use crate::shared::mpsc::{SendError, TrySendError};
use crate::ChannelCapacity;

const MAX_WAITING_SENDERS: usize = 16;

struct ChannelData<T> {
    queue: VecDeque<T>,
    capacity: usize,
    senders: usize,
    receivers: usize,
    senders_waiting: super::WakerList<MAX_WAITING_SENDERS>,
    receivers_waiting: WakerRegistration,
}

struct ChannelInner<T> {
    data: CriticalSectionMutex<RefCell<ChannelData<T>>>,
}

pub struct Sender<T> {
    inner: Arc<ChannelInner<T>>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.inner.data.lock(|s| s.borrow_mut().senders += 1);
        Sender {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.inner.data.lock(|s| {
            let mut d = s.borrow_mut();
            d.senders -= 1;
            if d.senders == 0 {
                d.receivers_waiting.wake();
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
        self.inner.data.lock(|s| s.borrow().receivers == 0)
    }

    fn enqueue(&self, value: T) -> EnqueueOutcome<T> {
        self.inner.data.lock(|s| {
            let mut d = s.borrow_mut();
            if d.receivers == 0 {
                return EnqueueOutcome::Disconnected(value);
            }
            if d.queue.len() >= d.capacity {
                return EnqueueOutcome::Full(value);
            }
            d.queue.push_back(value);
            d.receivers_waiting.wake();
            EnqueueOutcome::Sent
        })
    }

    fn has_capacity(&self) -> bool {
        self.inner.data.lock(|s| {
            let d = s.borrow();
            d.queue.len() < d.capacity
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
        poll_fn(move |cx| loop {
            if self.is_closed() {
                let message = pending
                    .take()
                    .expect("mpsc send message restored on Full path");
                return Poll::Ready(Err(SendError(message)));
            }
            let message = pending
                .take()
                .expect("mpsc send message restored on Full path");
            match self.enqueue(message) {
                EnqueueOutcome::Sent => return Poll::Ready(Ok(())),
                EnqueueOutcome::Disconnected(message) => {
                    return Poll::Ready(Err(SendError(message)))
                }
                EnqueueOutcome::Full(message) => {
                    pending = Some(message);
                    self.inner
                        .data
                        .lock(|s| s.borrow_mut().senders_waiting.register(cx.waker()));
                    if self.is_closed() {
                        let message = pending
                            .take()
                            .expect("mpsc send message restored on Full path");
                        return Poll::Ready(Err(SendError(message)));
                    }
                    if self.has_capacity() {
                        continue;
                    }
                    return Poll::Pending;
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
        self.inner.data.lock(|s| {
            let mut d = s.borrow_mut();
            d.receivers -= 1;
            if d.receivers == 0 {
                d.senders_waiting.wake();
            }
        });
    }
}

impl<T> Receiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        poll_fn(|cx| self.poll_recv(cx)).await
    }

    fn poll_recv(&self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let all_senders_gone = self.inner.data.lock(|s| {
            let mut d = s.borrow_mut();
            d.receivers_waiting.register(cx.waker());
            d.senders == 0
        });

        let result = self.inner.data.lock(|s| {
            let mut d = s.borrow_mut();
            if let Some(value) = d.queue.pop_front() {
                d.senders_waiting.wake();
                return Poll::Ready(Some(value));
            }
            Poll::Pending
        });

        if result.is_ready() {
            return result;
        }

        if all_senders_gone {
            return self.inner.data.lock(|s| {
                let mut d = s.borrow_mut();
                match d.queue.pop_front() {
                    Some(value) => {
                        d.senders_waiting.wake();
                        Poll::Ready(Some(value))
                    }
                    None => Poll::Ready(None),
                }
            });
        }

        Poll::Pending
    }
}

pub fn channel<T>(capacity: ChannelCapacity) -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(ChannelInner {
        data: CriticalSectionMutex::new(RefCell::new(ChannelData {
            queue: VecDeque::new(),
            capacity: capacity.get(),
            senders: 1,
            receivers: 1,
            senders_waiting: super::WakerList::new(),
            receivers_waiting: WakerRegistration::new(),
        })),
    });
    (
        Sender {
            inner: inner.clone(),
        },
        Receiver { inner },
    )
}
