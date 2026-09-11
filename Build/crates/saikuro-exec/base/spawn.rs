use alloc::boxed::Box;
use core::future::Future;
#[cfg(target_has_atomic = "ptr")]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic::{AtomicUsize, Ordering};

use crate::base::join::{new_join_handle, JoinHandle};
use crate::base::queue::{queue, BoxedFuture, NOTIFY};

/// Number of `spawn`ed tasks that have been created but not yet dropped.
static ACTIVE_TASKS: AtomicUsize = AtomicUsize::new(0);

/// Return the number of currently outstanding `spawn`ed tasks.
pub fn active_tasks() -> usize {
    ACTIVE_TASKS.load(Ordering::Relaxed)
}

struct TaskGuard;

impl Drop for TaskGuard {
    fn drop(&mut self) {
        ACTIVE_TASKS.fetch_sub(1, Ordering::Relaxed);
    }
}

pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + 'static,
    F::Output: 'static,
{
    super::block_on::ensure_runner_started();
    ACTIVE_TASKS.fetch_add(1, Ordering::Relaxed);
    let (slot, handle) = new_join_handle::<F::Output>();
    let task_slot = slot;
    let boxed: BoxedFuture = Box::pin(async move {
        let _guard = TaskGuard;
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
