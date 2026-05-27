//! Configuration for storage backends.

use std::time::Duration;

/// Selects which storage backend implementation to use at runtime.
///
/// When [`BackendKind::InMemory`] (the default), [`StorageConfig::persistence`]
/// determines the backend via the platform-aware dispatch in
/// [`create_storage`](crate::traits::StorageBackend).
///
/// Set this explicitly to bypass the automatic dispatch and force a specific
/// backend (returns an error if the backend is not available on the current
/// platform/feature set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendKind {
    /// In-memory DashMap backend. Works everywhere.
    #[default]
    InMemory,
    /// Web Storage API (localStorage / sessionStorage). WASM only.
    WebStorage,
    /// IndexedDB (browser). WASM only.
    IndexedDb,
    /// OPFS (File System Access API). WASM only.
    Opfs,
    /// Native filesystem via `std::fs`. Native only.
    Filesystem,
    /// Sled embedded database. Native only.
    Sled,
    /// SQLite via `rusqlite`. Native only.
    Sqlite,
}

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
    /// Which backend implementation to use.
    ///
    /// Defaults to [`BackendKind::InMemory`], which falls through to the
    /// platform-aware dispatch based on [`persistence`](Self::persistence).
    pub backend: BackendKind,

    /// How data should be persisted.
    pub persistence: PersistenceMode,

    /// Filesystem / database path for native persistent backends.
    ///
    /// - [`BackendKind::Filesystem`]: base directory for kv + file storage.
    /// - [`BackendKind::Sled`]: sled database directory.
    /// - [`BackendKind::Sqlite`]: SQLite database file path.
    ///
    /// When `None`, the factory uses a built-in default
    /// (`./saikuro_data`, `./saikuro_sled`, `./saikuro.sqlite`).
    pub storage_path: Option<std::path::PathBuf>,

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
            backend: BackendKind::InMemory,
            persistence: PersistenceMode::Transient,
            cleanup: CleanupPolicy::Never,
            namespace_prefix: None,
            auto_create_namespaces: true,
            sync_on_write: false,
            storage_path: None,
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

    /// Select a specific backend kind.
    pub fn with_backend(mut self, backend: BackendKind) -> Self {
        self.backend = backend;
        self
    }

    /// Set the filesystem / database path for native persistent backends.
    pub fn with_storage_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.storage_path = Some(path.into());
        self
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
