use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use embassy_sync::blocking_mutex::CriticalSectionMutex;

#[cfg(target_has_atomic = "ptr")]
use crate::Arc;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;

use crate::shared::JoinError;

pub(crate) struct JoinSlot<T> {
    pub(crate) value: Option<T>,
    pub(crate) closed: bool,
    pub(crate) wakers: super::WakerList<8>,
}

pub(crate) type JoinResultSlot<T> = CriticalSectionMutex<RefCell<JoinSlot<T>>>;

pub struct JoinHandle<T> {
    slot: Arc<JoinResultSlot<T>>,
}

impl<T> JoinHandle<T> {
    pub fn abort(&self) {
        self.slot.lock(|s| s.borrow_mut().closed = true);
    }

    pub fn is_finished(&self) -> bool {
        self.slot.lock(|s| s.borrow().value.is_some())
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut result = None;
        self.slot.lock(|s| {
            let mut state = s.borrow_mut();
            if let Some(v) = state.value.take() {
                result = Some(Ok(v));
            } else if state.closed {
                result = Some(Err(JoinError::cancelled()));
            } else {
                state.wakers.register(cx.waker());
            }
        });
        match result {
            Some(r) => Poll::Ready(r),
            None => Poll::Pending,
        }
    }
}

pub(crate) fn new_join_handle<T>() -> (Arc<JoinResultSlot<T>>, JoinHandle<T>) {
    let slot: Arc<JoinResultSlot<T>> =
        Arc::new(CriticalSectionMutex::new(RefCell::new(JoinSlot {
            value: None,
            closed: false,
            wakers: super::WakerList::new(),
        })));
    let handle = JoinHandle { slot: slot.clone() };
    (slot, handle)
}
