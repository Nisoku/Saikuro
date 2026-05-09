//! Saikuro Storage Backend Abstraction
//!
//! Provides a platform-agnostic storage interface for key-value and file-like
//! operations. Works across native (std::fs, databases) and WASM environments
//! (OPFS, IndexedDB, localStorage).

pub mod config;
pub mod error;
pub mod traits;

#[cfg(feature = "inmemory")]
pub mod inmemory;

pub use config::{CleanupPolicy, PersistenceMode, StorageConfig};
pub use error::{Result, StorageError};
pub use traits::{FileBackend, KeyValueBackend, KeyValueBackendExt, StorageBackend};

#[cfg(feature = "inmemory")]
pub use inmemory::InMemoryStorage;
