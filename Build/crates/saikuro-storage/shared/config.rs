use alloc::string::String;
use core::time::Duration;

/// Selects which storage backend implementation to use at runtime.
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
    /// File System Access API (user-picked directory). WASM only.
    FsAccess,
    /// Native filesystem via `std::fs`. Native only.
    Filesystem,
    /// Sled embedded database. Native only.
    Sled,
    /// SQLite via `graphitesql`. Available on all engines.
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
    #[cfg(feature = "std")]
    pub storage_path: Option<std::path::PathBuf>,

    /// Automatic cleanup policy.
    pub cleanup: CleanupPolicy,

    /// Optional namespace prefix for isolation.
    pub namespace_prefix: Option<String>,

    /// Whether to create namespaces automatically if they don't exist.
    pub auto_create_namespaces: bool,

    /// Whether to sync to durable storage after each write (if supported).
    pub sync_on_write: bool,

    /// SQLite page size in bytes for in-memory databases.
    pub sqlite_page_size: Option<u32>,
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
            sqlite_page_size: Some(1024),
            #[cfg(feature = "std")]
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
    #[cfg(feature = "std")]
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

    /// Set the SQLite page size for in-memory databases.
    ///
    /// Pass `None` to fall back to `graphitesql`'s 4096-byte pages.
    pub fn sqlite_page_size(mut self, page_size: Option<u32>) -> Self {
        self.sqlite_page_size = page_size;
        self
    }
}

/// Bounded-size limits
#[cfg(feature = "flash")]
pub mod limits {
    /// Maximum length of a namespace, in bytes. Encoded as `u8` in the
    /// on-flash record header.
    pub const MAX_NAMESPACE_LEN: usize = 255;

    /// Default maximum key length, in bytes.
    pub const DEFAULT_MAX_KEY_LEN: usize = 64;

    /// Default maximum value length, in bytes.
    pub const DEFAULT_MAX_VALUE_LEN: usize = 4096;

    /// Default flash sector (erase unit) size in bytes. The store's sectors
    /// must be multiples of the device erase size.
    pub const DEFAULT_SECTOR_SIZE: usize = 4096;

    /// Default number of sectors in the flash region. With the default sector
    /// size this is a 256 KiB region. One sector is reserved as the
    /// compaction spare, so usable capacity is
    /// `(sector_count - 1) * usable_bytes_per_sector`.
    pub const DEFAULT_SECTOR_COUNT: usize = 64;
}

/// Geometry and size limits for a flash-backed key-value store.
#[cfg(feature = "flash")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashConfig {
    /// Offset of the store's region inside the flash device. Must be aligned
    /// to the device erase size.
    pub base_offset: u32,
    /// Size of one sector in bytes. Must be a multiple of the device erase
    /// size.
    pub sector_size: usize,
    /// Number of sectors in the region. At least two: one for data, one as
    /// the compaction spare.
    pub sector_count: usize,
    /// Maximum key length in bytes, 1..=65535.
    pub max_key_len: usize,
    /// Maximum value length in bytes. A record must fit a single sector.
    pub max_value_len: usize,
}

#[cfg(feature = "flash")]
impl FlashConfig {
    /// A 256 KiB region using the defaults for every field.
    pub const DEFAULT: Self = Self {
        base_offset: 0,
        sector_size: limits::DEFAULT_SECTOR_SIZE,
        sector_count: limits::DEFAULT_SECTOR_COUNT,
        max_key_len: limits::DEFAULT_MAX_KEY_LEN,
        max_value_len: limits::DEFAULT_MAX_VALUE_LEN,
    };

    /// Validate geometry and size limits.
    ///
    /// Fails if the region is too small for two sectors, if `sector_size` is
    /// not a positive multiple of `erase_size`, or if a size limit is out of
    /// its documented range. Device-specific constraints (capacity, record
    /// fit against `WRITE_SIZE`) are checked by
    /// [`FlashKvStore::new`](crate::flash::FlashKvStore::new).
    pub fn new(
        base_offset: u32,
        sector_size: usize,
        sector_count: usize,
        max_key_len: usize,
        max_value_len: usize,
        erase_size: usize,
    ) -> Result<Self, &'static str> {
        if sector_count < 2 {
            return Err("flash region needs at least two sectors");
        }
        if sector_size == 0 || erase_size == 0 || !sector_size.is_multiple_of(erase_size) {
            return Err("sector size must be a positive multiple of the erase size");
        }
        if max_key_len == 0 || max_key_len > u16::MAX as usize {
            return Err("max key length must be in 1..=65535");
        }
        if max_value_len == 0 {
            return Err("max value length must be positive");
        }
        Ok(Self {
            base_offset,
            sector_size,
            sector_count,
            max_key_len,
            max_value_len,
        })
    }

    /// Total size of the region in bytes.
    pub fn region_size(&self) -> usize {
        self.sector_size * self.sector_count
    }
}
