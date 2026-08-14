use core::fmt;
use core::ops::{Deref, DerefMut};

#[cfg(feature = "std")]
use std::sync as imp;

#[cfg(not(feature = "std"))]
use spin as imp;

/// A reader-writer lock.  `read`/`write` return guards that deref to the
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

/// A mutual-exclusion lock.  `lock` returns a guard that derefs to the
/// protected value.
pub struct Mutex<T: ?Sized> {
    inner: imp::Mutex<T>,
}

/// Guard acquired by [`Mutex::lock`].
pub struct MutexGuard<'a, T: ?Sized> {
    inner: imp::MutexGuard<'a, T>,
}

// The std and spin backends disagree on whether lock acquisition can fail
// (std returns `LockResult`, spin returns a guard directly).  These traits
// normalize the two behind a single guard-returning API.

trait RwLockAccess<T: ?Sized> {
    fn read_guard(&self) -> imp::RwLockReadGuard<'_, T>;
    fn write_guard(&self) -> imp::RwLockWriteGuard<'_, T>;
}

trait MutexAccess<T: ?Sized> {
    fn lock_guard(&self) -> imp::MutexGuard<'_, T>;
}

#[cfg(feature = "std")]
impl<T: ?Sized> RwLockAccess<T> for imp::RwLock<T> {
    fn read_guard(&self) -> imp::RwLockReadGuard<'_, T> {
        self.read()
            .expect("RwLock poisoned by a panicking guard holder")
    }

    fn write_guard(&self) -> imp::RwLockWriteGuard<'_, T> {
        self.write()
            .expect("RwLock poisoned by a panicking guard holder")
    }
}

#[cfg(feature = "std")]
impl<T: ?Sized> MutexAccess<T> for imp::Mutex<T> {
    fn lock_guard(&self) -> imp::MutexGuard<'_, T> {
        self.lock()
            .expect("Mutex poisoned by a panicking guard holder")
    }
}

#[cfg(not(feature = "std"))]
impl<T: ?Sized> RwLockAccess<T> for imp::RwLock<T> {
    fn read_guard(&self) -> imp::RwLockReadGuard<'_, T> {
        self.read()
    }

    fn write_guard(&self) -> imp::RwLockWriteGuard<'_, T> {
        self.write()
    }
}

#[cfg(not(feature = "std"))]
impl<T: ?Sized> MutexAccess<T> for imp::Mutex<T> {
    fn lock_guard(&self) -> imp::MutexGuard<'_, T> {
        self.lock()
    }
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
        RwLockReadGuard {
            inner: self.inner.read_guard(),
        }
    }

    /// Acquire the write guard.
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        RwLockWriteGuard {
            inner: self.inner.write_guard(),
        }
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
        MutexGuard {
            inner: self.inner.lock_guard(),
        }
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

// The wrappers inherit `Send`/`Sync` from their inner locks: std locks are
// `Send + Sync` when `T: Send`, spin locks likewise.  No manual impls needed.
