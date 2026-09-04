#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

// Exactly one storage engine may be active per build.
#[cfg(all(feature = "native", feature = "wasm"))]
compile_error!("only one storage engine may be enabled (native vs wasm)");
#[cfg(all(feature = "native", feature = "no_std"))]
compile_error!("only one storage engine may be enabled (native vs no_std)");
#[cfg(all(feature = "native", feature = "embedded"))]
compile_error!("only one storage engine may be enabled (native vs embedded)");
#[cfg(all(feature = "wasm", feature = "no_std"))]
compile_error!("only one storage engine may be enabled (wasm vs no_std)");
#[cfg(all(feature = "wasm", feature = "embedded"))]
compile_error!("only one storage engine may be enabled (wasm vs embedded)");
#[cfg(all(feature = "no_std", feature = "embedded"))]
compile_error!("only one storage engine may be enabled (no_std vs embedded)");

// The `native` engine requires the standard library.
#[cfg(all(feature = "native", not(feature = "std")))]
compile_error!("the native engine requires the std toolchain");

// On WASI `std` is the libc base and `no_std` selects the engine.
#[cfg(all(
    feature = "no_std",
    feature = "std",
    not(target_os = "wasi")
))]
compile_error!("the no_std engine must not be combined with the std toolchain");

pub mod common;
#[cfg(feature = "embedded")]
pub mod embedded;
#[cfg(feature = "native")]
pub mod native;
pub mod shared;
#[cfg(any(feature = "wasi-preview1", feature = "wasi-component"))]
pub mod wasi;
#[cfg(all(feature = "wasm", feature = "std"))]
pub mod wasm;

/// Generates a web-storage-backed key-value backend.
#[macro_export]
macro_rules! impl_web_storage {
    ($name:ident, $storage_fn:ident) => {
        use bytes::Bytes;
        use $crate::shared::traits::{KeyValueBackend, StorageBackend};

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

            fn storage(&self) -> $crate::Result<web_sys::Storage> {
                let w = $crate::webstorage::window()?;
                w.$storage_fn()
                    .map_err(|e| {
                        $crate::SaikuroError::internal(format!(
                            "failed to get {}: {e:?}",
                            stringify!($storage_fn)
                        ))
                    })?
                    .ok_or_else(|| {
                        $crate::SaikuroError::backend_not_available(stringify!($storage_fn))
                    })
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl KeyValueBackend for $name {
            fn config(&self) -> &$crate::StorageConfig {
                &self.config
            }

            async fn exists(&self, namespace: &str, key: &str) -> $crate::Result<bool> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::util::apply_prefix(&self.config, namespace);
                let full_key = $crate::util::make_key(&prefixed_ns, key);
                match $crate::webstorage::storage_get(&storage, &full_key)? {
                    Some(_) => Ok(true),
                    None => Ok(false),
                }
            }

            async fn get(&self, namespace: &str, key: &str) -> $crate::Result<Option<Bytes>> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::util::apply_prefix(&self.config, namespace);
                let full_key = $crate::util::make_key(&prefixed_ns, key);
                $crate::webstorage::storage_get(&storage, &full_key)
            }

            async fn put(&self, namespace: &str, key: &str, value: Bytes) -> $crate::Result<()> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::util::apply_prefix(&self.config, namespace);
                let full_key = $crate::util::make_key(&prefixed_ns, key);
                $crate::webstorage::storage_set(&storage, &full_key, &value)
            }

            async fn delete(&self, namespace: &str, key: &str) -> $crate::Result<()> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::util::apply_prefix(&self.config, namespace);
                let full_key = $crate::util::make_key(&prefixed_ns, key);
                $crate::webstorage::storage_remove(&storage, &full_key);
                Ok(())
            }

            async fn list_keys(&self, namespace: &str) -> $crate::Result<Vec<String>> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::util::apply_prefix(&self.config, namespace);
                Ok($crate::webstorage::get_keys_in_namespace(
                    &storage,
                    &prefixed_ns,
                ))
            }

            async fn list_namespaces(&self) -> $crate::Result<Vec<String>> {
                let storage = self.storage()?;
                let raw = $crate::webstorage::get_namespaces(&storage);
                let result: Vec<String> = raw
                    .into_iter()
                    .map(|ns| $crate::util::strip_prefix(&self.config, &ns))
                    .collect();
                Ok(result)
            }

            async fn create_namespace(&self, _namespace: &str) -> $crate::Result<()> {
                Ok(())
            }

            async fn delete_namespace(&self, namespace: &str) -> $crate::Result<()> {
                let storage = self.storage()?;
                let prefixed_ns = $crate::util::apply_prefix(&self.config, namespace);
                let prefix = $crate::util::key_prefix(&prefixed_ns);
                $crate::webstorage::delete_keys_with_prefix(&storage, &prefix);
                Ok(())
            }

            async fn clear_namespace(&self, namespace: &str) -> $crate::Result<()> {
                self.delete_namespace(namespace).await
            }
        }

        impl StorageBackend for $name {
            fn supports_files(&self) -> bool {
                false
            }
        }
    };
}

#[cfg(feature = "flash")]
pub use shared::config::FlashConfig;
pub use shared::config::{BackendKind, CleanupPolicy, PersistenceMode, StorageConfig};

pub use saikuro_event::{Result, SaikuroError};

/// Raw byte buffer used by every key-value and file backend.
pub use bytes::Bytes;

pub use shared::traits::{FileBackend, KeyValueBackend, KeyValueBackendExt, StorageBackend};

pub use shared::config;
pub use shared::traits;
pub use shared::util;

#[cfg(feature = "inmemory")]
pub use common::inmemory::InMemoryStorage;

#[cfg(all(feature = "wasm", feature = "std", target_arch = "wasm32"))]
pub use wasm::indexeddb::IndexedDbStorage;

#[cfg(all(feature = "wasm", feature = "std"))]
pub use wasm::local_storage::LocalStorage;

#[cfg(all(feature = "wasm", feature = "std"))]
pub use wasm::session_storage::SessionStorage;

#[cfg(all(feature = "wasm", feature = "std", target_arch = "wasm32"))]
pub use wasm::fs_access::FsAccessStorage;

#[cfg(all(feature = "wasm", feature = "std", target_arch = "wasm32"))]
pub use wasm::opfs::OpfsStorage;

// Root aliases for the wasm submodules referenced by `impl_web_storage!`.
#[cfg(all(feature = "wasm", feature = "std"))]
pub use wasm::local_storage;
#[cfg(all(feature = "wasm", feature = "std"))]
pub use wasm::session_storage;
#[cfg(all(feature = "wasm", feature = "std", target_arch = "wasm32"))]
pub use wasm::{fs_access, indexeddb, opfs, webstorage};

#[cfg(feature = "fs")]
pub use native::fs::FilesystemStorage;

#[cfg(feature = "sled")]
pub use native::sled::SledStorage;

#[cfg(feature = "sqlite")]
pub use common::sqlite::SqliteStorage;

#[cfg(feature = "flash")]
pub use embedded::flash::FlashKvStore;

#[cfg(any(feature = "wasi-preview1", feature = "wasi-component"))]
pub use wasi::{WasiFileStore, WasiKvStore};
