use core::ops::{Deref, DerefMut};

pub struct Mutex<T> {
    inner: tokio::sync::Mutex<T>,
}

impl<T> Mutex<T> {
    pub fn new(value: T) -> Self {
        Mutex {
            inner: tokio::sync::Mutex::new(value),
        }
    }

    pub async fn lock(&self) -> MutexGuard<'_, T> {
        MutexGuard {
            inner: self.inner.lock().await,
        }
    }
}

pub struct MutexGuard<'a, T> {
    inner: tokio::sync::MutexGuard<'a, T>,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

pub struct RwLock<T> {
    inner: tokio::sync::RwLock<T>,
}

impl<T> RwLock<T> {
    pub fn new(value: T) -> Self {
        RwLock {
            inner: tokio::sync::RwLock::new(value),
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, T> {
        RwLockReadGuard {
            guard: self.inner.read().await,
        }
    }

    pub async fn write(&self) -> RwLockWriteGuard<'_, T> {
        RwLockWriteGuard {
            guard: self.inner.write().await,
        }
    }
}

pub struct RwLockReadGuard<'a, T> {
    guard: tokio::sync::RwLockReadGuard<'a, T>,
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

pub struct RwLockWriteGuard<'a, T> {
    guard: tokio::sync::RwLockWriteGuard<'a, T>,
}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

pub struct Barrier {
    inner: tokio::sync::Barrier,
}

impl Barrier {
    pub fn new(n: usize) -> Self {
        Barrier {
            inner: tokio::sync::Barrier::new(n),
        }
    }

    pub async fn wait(&self) {
        self.inner.wait().await;
    }
}
