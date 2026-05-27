//! Saikuro Storage Backend Abstraction
//!
//! Provides a platform-agnostic storage interface for key-value and file-like
//! operations. Works across native (std::fs, databases) and WASM environments
//! (OPFS, IndexedDB, localStorage, sessionStorage).

pub mod config;
pub mod error;
pub mod traits;
pub mod util;

#[cfg(feature = "inmemory")]
pub mod inmemory;

#[cfg(all(feature = "wasm-storage", target_arch = "wasm32"))]
pub mod indexeddb;

#[cfg(all(feature = "wasm-storage", target_arch = "wasm32"))]
pub mod webstorage;

#[cfg(all(feature = "wasm-storage", target_arch = "wasm32"))]
pub mod opfs;

#[cfg(feature = "local-storage")]
pub mod local_storage;

#[cfg(feature = "session-storage")]
pub mod session_storage;

/// Generates a web-storage-backed key-value backend.
///
/// `$name` is the struct name (e.g., `LocalStorage`).
/// `$storage_fn` is the `Window` method to get the storage object
/// (e.g., `local_storage` or `session_storage`).
#[macro_export]
macro_rules! impl_web_storage {
    ($name:ident, $storage_fn:ident) => {
        use async_trait::async_trait;
        use bytes::Bytes;
        use $crate::traits::{KeyValueBackend, StorageBackend};

        pub struct $name {
            config: $crate::StorageConfig,
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    config: $crate::StorageConfig::default(),
                }
            }

            pub fn with_config(config: $crate::StorageConfig) -> Self {
                Self { config }
            }

            fn storage(&self) -> $crate::error::Result<web_sys::Storage> {
                let w = $crate::webstorage::window()?;
                w.$storage_fn()
                    .map_err(|e| {
                        $crate::StorageError::internal(format!(
                            "failed to get {}: {e:?}",
                            stringify!($storage_fn)
                        ))
                    })?
                    .ok_or_else(|| {
                        $crate::StorageError::backend_not_available(stringify!($storage_fn))
                    })
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        #[async_trait]
        impl KeyValueBackend for $name {
            fn config(&self) -> &$crate::StorageConfig {
                &self.config
            }

            async fn exists(&self, namespace: &str, key: &str) -> $crate::error::Result<bool> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::webstorage::apply_prefix(&self.config, namespace);
                let full_key = $crate::webstorage::make_key(&prefixed_ns, key);
                match $crate::webstorage::storage_get(&storage, &full_key)? {
                    Some(_) => Ok(true),
                    None => Ok(false),
                }
            }

            async fn get(
                &self,
                namespace: &str,
                key: &str,
            ) -> $crate::error::Result<Option<Bytes>> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::webstorage::apply_prefix(&self.config, namespace);
                let full_key = $crate::webstorage::make_key(&prefixed_ns, key);
                $crate::webstorage::storage_get(&storage, &full_key)
            }

            async fn put(
                &self,
                namespace: &str,
                key: &str,
                value: Bytes,
            ) -> $crate::error::Result<()> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::webstorage::apply_prefix(&self.config, namespace);
                let full_key = $crate::webstorage::make_key(&prefixed_ns, key);
                $crate::webstorage::storage_set(&storage, &full_key, &value)
            }

            async fn delete(&self, namespace: &str, key: &str) -> $crate::error::Result<()> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::webstorage::apply_prefix(&self.config, namespace);
                let full_key = $crate::webstorage::make_key(&prefixed_ns, key);
                $crate::webstorage::storage_remove(&storage, &full_key);
                Ok(())
            }

            async fn list_keys(&self, namespace: &str) -> $crate::error::Result<Vec<String>> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::webstorage::apply_prefix(&self.config, namespace);
                Ok($crate::webstorage::get_keys_in_namespace(
                    &storage,
                    &prefixed_ns,
                ))
            }

            async fn list_namespaces(&self) -> $crate::error::Result<Vec<String>> {
                let storage = self.storage()?;
                let raw = $crate::webstorage::get_namespaces(&storage);
                let result: Vec<String> = raw
                    .into_iter()
                    .map(|ns| $crate::webstorage::strip_prefix(&self.config, &ns))
                    .collect();
                Ok(result)
            }

            async fn create_namespace(&self, _namespace: &str) -> $crate::error::Result<()> {
                Ok(())
            }

            async fn delete_namespace(&self, namespace: &str) -> $crate::error::Result<()> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::webstorage::apply_prefix(&self.config, namespace);
                let prefix = $crate::webstorage::key_prefix(&prefixed_ns);
                $crate::webstorage::delete_keys_with_prefix(&storage, &prefix);
                Ok(())
            }

            async fn clear_namespace(&self, namespace: &str) -> $crate::error::Result<()> {
                self.delete_namespace(namespace).await
            }
        }

        #[async_trait]
        impl StorageBackend for $name {
            fn supports_files(&self) -> bool {
                false
            }
        }
    };
}

pub use config::{BackendKind, CleanupPolicy, PersistenceMode, StorageConfig};
pub use error::{Result, StorageError};
pub use traits::{FileBackend, KeyValueBackend, KeyValueBackendExt, StorageBackend};

#[cfg(feature = "inmemory")]
pub use inmemory::InMemoryStorage;

#[cfg(all(feature = "wasm-storage", target_arch = "wasm32"))]
pub use indexeddb::IndexedDbStorage;

#[cfg(feature = "local-storage")]
pub use local_storage::LocalStorage;

#[cfg(feature = "session-storage")]
pub use session_storage::SessionStorage;

#[cfg(all(feature = "wasm-storage", target_arch = "wasm32"))]
pub use opfs::OpfsStorage;

#[cfg(feature = "fs-storage")]
pub mod fs;

#[cfg(feature = "sled-storage")]
pub mod sled;

#[cfg(feature = "sqlite-storage")]
pub mod sqlite;

#[cfg(feature = "fs-storage")]
pub use fs::FilesystemStorage;

#[cfg(feature = "sled-storage")]
pub use sled::SledStorage;

#[cfg(feature = "sqlite-storage")]
pub use sqlite::SqliteStorage;
