use alloc::boxed::Box;
use core::future::Future;

use crate::base::join::{new_join_handle, JoinHandle};
use crate::base::queue::{queue, BoxedFuture, NOTIFY};

pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + 'static,
    F::Output: 'static,
{
    let (slot, handle) = new_join_handle::<F::Output>();
    let task_slot = slot;
    let boxed: BoxedFuture = Box::pin(async move {
        let result = fut.await;
        task_slot.lock(|s| {
            s.borrow_mut().value = Some(result);
            s.borrow_mut().wakers.wake();
        });
    });
    queue().lock(|q| q.borrow_mut().push(boxed));
    NOTIFY.signal(());
    handle
}
