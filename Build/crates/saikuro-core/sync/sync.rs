use core::fmt;
use core::ops::{Deref, DerefMut};

#[cfg(feature = "std")]
use std::sync as imp;

#[cfg(not(feature = "std"))]
use spin as imp;

/// A reader-writer lock. `read`/`write` return guards that deref to the
/// protected value.
pub struct RwLock<T: ?Sized> {
    inner: imp::RwLock<T>,
}

/// Guard acquired by [`RwLock::read`].
pub struct RwLockReadGuard<'a, T: ?Sized> {
    inner: imp::RwLockReadGuard<'a, T>,
}

/// Guard acquired by [`RwLock::write`].
pub struct RwLockWriteGuard<'a, T: ?Sized> {
    inner: imp::RwLockWriteGuard<'a, T>,
}

/// A mutual-exclusion lock. `lock` returns a guard that derefs to the
/// protected value.
pub struct Mutex<T: ?Sized> {
    inner: imp::Mutex<T>,
}

/// Guard acquired by [`Mutex::lock`].
pub struct MutexGuard<'a, T: ?Sized> {
    inner: imp::MutexGuard<'a, T>,
}

impl<T> RwLock<T> {
    /// Create a new lock guarding `value`.
    pub const fn new(value: T) -> Self {
        Self {
            inner: imp::RwLock::new(value),
        }
    }
}

impl<T: Default> Default for RwLock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Acquire the read guard.
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        #[cfg(feature = "std")]
        let inner = self
            .inner
            .read()
            .expect("RwLock poisoned by a panicking guard holder");
        #[cfg(not(feature = "std"))]
        let inner = self.inner.read();
        RwLockReadGuard { inner }
    }

    /// Acquire the write guard.
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        #[cfg(feature = "std")]
        let inner = self
            .inner
            .write()
            .expect("RwLock poisoned by a panicking guard holder");
        #[cfg(not(feature = "std"))]
        let inner = self.inner.write();
        RwLockWriteGuard { inner }
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Mutex<T> {
    /// Create a new mutex guarding `value`.
    pub const fn new(value: T) -> Self {
        Self {
            inner: imp::Mutex::new(value),
        }
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Acquire the guard.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        #[cfg(feature = "std")]
        let inner = self
            .inner
            .lock()
            .expect("Mutex poisoned by a panicking guard holder");
        #[cfg(not(feature = "std"))]
        let inner = self.inner.lock();
        MutexGuard { inner }
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

//  Debug

impl<T: fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLock")
            .field("inner", &&self.inner)
            .finish()
    }
}

impl<T: fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutex")
            .field("inner", &&self.inner)
            .finish()
    }
}
