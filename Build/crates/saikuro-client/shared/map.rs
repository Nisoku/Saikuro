//! Cross-platform concurrent map for pending invocations.

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;
#[cfg(feature = "std")]
use dashmap::DashMap;

use saikuro_core::invocation::InvocationId;

use crate::shared::types::{ChannelSendTx, PendingSlot};

// PendingMap

/// A concurrent map of pending invocations, keyed by [`InvocationId`].
pub struct PendingMap {
    #[cfg(feature = "std")]
    inner: DashMap<InvocationId, PendingSlot>,
    #[cfg(not(feature = "std"))]
    inner: spin::Mutex<BTreeMap<InvocationId, PendingSlot>>,
}

impl PendingMap {
    /// Create an empty map.
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            inner: DashMap::new(),
            #[cfg(not(feature = "std"))]
            inner: spin::Mutex::new(BTreeMap::new()),
        }
    }

    /// Insert a slot for the given invocation ID.
    pub fn insert(&self, key: InvocationId, value: PendingSlot) {
        #[cfg(feature = "std")]
        {
            self.inner.insert(key, value);
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.lock().insert(key, value);
        }
    }

    /// Remove and return the slot for the given invocation ID, if present.
    pub fn remove(&self, key: &InvocationId) -> Option<(InvocationId, PendingSlot)> {
        #[cfg(feature = "std")]
        {
            self.inner.remove(key)
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.lock().remove(key).map(|v| (*key, v))
        }
    }

    /// Remove all entries.
    pub fn clear(&self) {
        #[cfg(feature = "std")]
        {
            self.inner.clear();
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.lock().clear();
        }
    }

    /// Collect all keys.
    pub fn keys(&self) -> alloc::vec::Vec<InvocationId> {
        #[cfg(feature = "std")]
        {
            self.inner.iter().map(|e| *e.key()).collect()
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.lock().keys().copied().collect()
        }
    }
}

// ChannelSenderMap

/// A concurrent map of channel outbound senders, keyed by [`InvocationId`].
pub struct ChannelSenderMap {
    #[cfg(feature = "std")]
    inner: DashMap<InvocationId, ChannelSendTx>,
    #[cfg(not(feature = "std"))]
    inner: spin::Mutex<BTreeMap<InvocationId, ChannelSendTx>>,
}

impl ChannelSenderMap {
    /// Create an empty map.
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            inner: DashMap::new(),
            #[cfg(not(feature = "std"))]
            inner: spin::Mutex::new(BTreeMap::new()),
        }
    }

    /// Insert a channel sender for the given invocation ID.
    pub fn insert(&self, key: InvocationId, value: ChannelSendTx) {
        #[cfg(feature = "std")]
        {
            self.inner.insert(key, value);
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.lock().insert(key, value);
        }
    }

    /// Remove and return the channel sender for the given invocation ID.
    pub fn remove(&self, key: &InvocationId) -> Option<(InvocationId, ChannelSendTx)> {
        #[cfg(feature = "std")]
        {
            self.inner.remove(key)
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.lock().remove(key).map(|v| (*key, v))
        }
    }

    /// Clone all values (used during shutdown).
    pub fn clone_values(&self) -> alloc::vec::Vec<ChannelSendTx> {
        #[cfg(feature = "std")]
        {
            self.inner.iter().map(|e| e.value().clone()).collect()
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.lock().values().cloned().collect()
        }
    }

    /// Remove all entries.
    pub fn clear(&self) {
        #[cfg(feature = "std")]
        {
            self.inner.clear();
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.lock().clear();
        }
    }
}
