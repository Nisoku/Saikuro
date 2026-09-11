// sync

use super::*;

pub struct Mutex<T> {
    inner: embassy_sync::mutex::Mutex<CriticalSectionRawMutex, T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Mutex {
            inner: embassy_sync::mutex::Mutex::new(value),
        }
    }

    pub async fn lock(&self) -> MutexGuard<'_, T> {
        MutexGuard {
            inner: self.inner.lock().await,
        }
    }
}

pub struct MutexGuard<'a, T> {
    inner: embassy_sync::mutex::MutexGuard<'a, CriticalSectionRawMutex, T>,
}

impl<T> core::ops::Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> core::ops::DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

pub struct RwLock<T> {
    inner: embassy_sync::mutex::Mutex<CriticalSectionRawMutex, T>,
}

impl<T> RwLock<T> {
    pub const fn new(value: T) -> Self {
        RwLock {
            inner: embassy_sync::mutex::Mutex::new(value),
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, T> {
        RwLockReadGuard {
            guard: self.inner.lock().await,
        }
    }

    pub async fn write(&self) -> RwLockWriteGuard<'_, T> {
        RwLockWriteGuard {
            guard: self.inner.lock().await,
        }
    }
}

pub struct RwLockReadGuard<'a, T> {
    guard: embassy_sync::mutex::MutexGuard<'a, CriticalSectionRawMutex, T>,
}

impl<T> core::ops::Deref for RwLockReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

pub struct RwLockWriteGuard<'a, T> {
    guard: embassy_sync::mutex::MutexGuard<'a, CriticalSectionRawMutex, T>,
}

impl<T> core::ops::Deref for RwLockWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T> core::ops::DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

const MAX_BARRIER_WAITERS: usize = 16;

pub struct Barrier {
    inner: Arc<BarrierInner>,
}

struct BarrierInner {
    state: CriticalSectionMutex<RefCell<BarrierState>>,
}

struct BarrierState {
    count: usize,
    arrived: usize,
    generation: u64,
    waiting: super::WakerList<MAX_BARRIER_WAITERS>,
}

impl Barrier {
    pub fn new(n: usize) -> Self {
        assert!(n > 0, "saikuro-exec: Barrier::new requires n > 0");
        let inner = Arc::new(BarrierInner {
            state: CriticalSectionMutex::new(RefCell::new(BarrierState {
                count: n,
                arrived: 0,
                generation: 0,
                waiting: super::WakerList::new(),
            })),
        });
        Barrier { inner }
    }

    pub async fn wait(&self) {
        let pre_release_generation = self.inner.state.lock(|s| {
            let mut state = s.borrow_mut();
            state.arrived += 1;
            if state.arrived == state.count {
                state.arrived = 0;
                state.generation += 1;
                state.waiting.wake();
                None
            } else {
                Some(state.generation)
            }
        });
        let Some(mut gen) = pre_release_generation else {
            return;
        };
        poll_fn(move |cx| {
            self.inner.state.lock(|s| {
                let mut state = s.borrow_mut();
                if state.generation != gen {
                    gen = state.generation;
                    Poll::Ready(())
                } else {
                    state.waiting.register(cx.waker());
                    if state.generation != gen {
                        gen = state.generation;
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                }
            })
        })
        .await
    }
}
