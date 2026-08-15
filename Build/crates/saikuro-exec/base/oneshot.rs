// oneshot

use super::*;
pub use crate::shared::oneshot::RecvError;

enum State<T> {
    Empty,
    Waiting(Waker),
    Ready(T),
    Closed,
}

struct InnerData<T> {
    channel: State<T>,
    receiver_alive: bool,
}

struct Inner<T> {
    state: CriticalSectionMutex<RefCell<InnerData<T>>>,
}

pub struct Sender<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Sender<T> {
    pub fn send(self, value: T) -> Result<(), T> {
        self.inner.state.lock(|s| {
            let mut data = s.borrow_mut();
            if !data.receiver_alive {
                return Err(value);
            }
            match core::mem::replace(&mut data.channel, State::Empty) {
                State::Empty => data.channel = State::Ready(value),
                State::Waiting(waker) => {
                    data.channel = State::Ready(value);
                    waker.wake();
                }
                State::Ready(v) => {
                    data.channel = State::Ready(v);
                    core::unreachable!("oneshot sender cannot send twice");
                }
                State::Closed => {
                    data.channel = State::Closed;
                    core::unreachable!("oneshot sender cannot send on a closed channel");
                }
            }
            Ok(())
        })
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.inner.state.lock(|s| {
            let mut data = s.borrow_mut();
            if matches!(data.channel, State::Ready(_)) {
                return;
            }
            let old = core::mem::replace(&mut data.channel, State::Closed);
            if let State::Waiting(waker) = old {
                waker.wake();
            }
        });
    }
}

pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.inner
            .state
            .lock(|s| s.borrow_mut().receiver_alive = false);
    }
}

impl<T> Future for Receiver<T> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().inner.state.lock(|s| {
            let mut data = s.borrow_mut();
            match core::mem::replace(&mut data.channel, State::Empty) {
                State::Ready(value) => {
                    data.channel = State::Closed;
                    Poll::Ready(Ok(value))
                }
                State::Closed => Poll::Ready(Err(RecvError)),
                State::Empty => {
                    data.channel = State::Waiting(cx.waker().clone());
                    Poll::Pending
                }
                State::Waiting(w) => {
                    if w.will_wake(cx.waker()) {
                        data.channel = State::Waiting(w);
                    } else {
                        data.channel = State::Waiting(cx.waker().clone());
                        w.wake();
                    }
                    Poll::Pending
                }
            }
        })
    }
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        state: CriticalSectionMutex::new(RefCell::new(InnerData {
            channel: State::Empty,
            receiver_alive: true,
        })),
    });
    (
        Sender {
            inner: inner.clone(),
        },
        Receiver { inner },
    )
}
