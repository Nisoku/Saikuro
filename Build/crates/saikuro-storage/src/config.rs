//! Configuration for storage backends.

use std::time::Duration;

/// Persistence mode controls how data is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PersistenceMode {
    /// Data is only kept in memory; lost on process/context restart.
    /// Use for caching, ephemeral state, testing.
    #[default]
    Transient,

    /// Data is persisted to durable storage (disk, IndexedDB, OPFS).
    /// Survives process/context restarts.
    Durable,

    /// Data is persisted but may be cleaned up by the system under pressure.
    /// Example: browser localStorage may be cleared by user.
    BestEffort,
}

/// Cleanup policy for automatic garbage collection of old entries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CleanupPolicy {
    /// Never automatically clean up entries.
    #[default]
    Never,

    /// Clean up entries older than the specified duration after last access.
    Ttl(Duration),

    /// Clean up entries older than the specified duration after creation.
    Age(Duration),

    /// Keep only the most recent N entries per namespace.
    Lru(usize),
}

/// Configuration for a storage backend instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    /// How data should be persisted.
    pub persistence: PersistenceMode,

    /// Automatic cleanup policy.
    pub cleanup: CleanupPolicy,

    /// Optional namespace prefix for isolation.
    pub namespace_prefix: Option<String>,

    /// Whether to create namespaces automatically if they don't exist.
    pub auto_create_namespaces: bool,

    /// Whether to sync to durable storage after each write (if supported).
    pub sync_on_write: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            persistence: PersistenceMode::Transient,
            cleanup: CleanupPolicy::Never,
            namespace_prefix: None,
            auto_create_namespaces: true,
            sync_on_write: false,
        }
    }
}

impl StorageConfig {
    /// Create a configuration for transient, in-memory storage.
    pub fn transient() -> Self {
        Self {
            persistence: PersistenceMode::Transient,
            ..Default::default()
        }
    }

    /// Create a configuration for durable storage.
    pub fn durable() -> Self {
        Self {
            persistence: PersistenceMode::Durable,
            sync_on_write: true,
            ..Default::default()
        }
    }

    /// Set a namespace prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.namespace_prefix = Some(prefix.into());
        self
    }

    /// Set TTL-based cleanup.
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.cleanup = CleanupPolicy::Ttl(ttl);
        self
    }
}
