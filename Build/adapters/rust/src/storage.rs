use saikuro_storage::traits::StorageBackend;
use saikuro_storage::{BackendKind, PersistenceMode, StorageConfig};

use crate::error::Error;
use crate::error::Result;

/// Create a storage backend based on the given configuration.
///
/// When [`StorageConfig::backend`] is [`BackendKind::InMemory`] (the default),
/// the platform- and persistence-aware dispatch table below is used:
///
/// | `persistence`      | native                          | wasm32 (with `wasm-storage`)  |
/// |--------------------|---------------------------------|-------------------------------|
/// | `Transient`        | `InMemoryStorage`               | `InMemoryStorage`             |
/// | `BestEffort`       | `InMemoryStorage`               | `LocalStorage`                |
/// | `Durable`          | error (no durable backend selected) | `IndexedDbStorage`        |
///
/// Set [`StorageConfig::backend`] to a specific [`BackendKind`] variant
/// to bypass the table and force a particular implementation.  A backend
/// that is not compiled into the binary (e.g. `Filesystem` without the
/// `storage-fs` feature) returns `Err` at runtime.
pub async fn create_storage(config: &StorageConfig) -> Result<Box<dyn StorageBackend>> {
    // Explicit backend kind overrides
    match config.backend {
        BackendKind::Filesystem => {
            return create_filesystem(config).await;
        }
        BackendKind::Sled => {
            return create_sled(config).await;
        }
        BackendKind::Sqlite => {
            return create_sqlite(config).await;
        }
        BackendKind::WebStorage => {
            return create_web_storage(config).await;
        }
        BackendKind::IndexedDb => {
            return create_indexeddb(config).await;
        }
        BackendKind::Opfs => {
            return create_opfs(config).await;
        }
        BackendKind::InMemory => { /* fall through to persistence-based dispatch */ }
    }

    // InMemory (default): persistence-mode-based dispatch (backward compat)
    match config.persistence {
        PersistenceMode::Transient => {
            let storage = saikuro_storage::InMemoryStorage::with_config(config.clone());
            Ok(Box::new(storage))
        }

        PersistenceMode::BestEffort => {
            // wasm32 with wasm-storage → LocalStorage (persists across page reload)
            #[cfg(all(target_arch = "wasm32", feature = "wasm-storage"))]
            {
                let storage = saikuro_storage::LocalStorage::with_config(config.clone());
                return Ok(Box::new(storage));
            }

            // Fallback: in-memory (native, or wasm32 without wasm-storage)
            #[cfg(not(all(target_arch = "wasm32", feature = "wasm-storage")))]
            {
                let storage = saikuro_storage::InMemoryStorage::with_config(config.clone());
                Ok(Box::new(storage))
            }
        }

        PersistenceMode::Durable => {
            // wasm32 with wasm-storage → IndexedDB (survives page reload + clear)
            #[cfg(all(target_arch = "wasm32", feature = "wasm-storage"))]
            {
                let storage = saikuro_storage::IndexedDbStorage::with_config(config.clone());
                return Ok(Box::new(storage));
            }

            // Not available with the default backend selection — the caller
            // should set `BackendKind::Filesystem` (or `Sled`, `Sqlite`) explicitly.
            #[cfg(not(all(target_arch = "wasm32", feature = "wasm-storage")))]
            {
                Err(Error::Storage(
                    "no durable storage backend selected; set `config.backend` to \
                     `BackendKind::Filesystem`, `Sled`, or `Sqlite` on native, \
                     or enable the 'wasm-storage' feature on wasm32"
                        .into(),
                ))
            }
        }
    }
}

// Platform helper factories

async fn create_filesystem(_config: &StorageConfig) -> Result<Box<dyn StorageBackend>> {
    #[cfg(feature = "storage-fs")]
    {
        let path = _config
            .storage_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("./saikuro_data"));
        let storage = saikuro_storage::FilesystemStorage::with_config(path, _config.clone());
        Ok(Box::new(storage))
    }
    #[cfg(not(feature = "storage-fs"))]
    {
        Err(Error::Storage(
            "Filesystem backend not available: enable the 'storage-fs' feature".into(),
        ))
    }
}

async fn create_sled(_config: &StorageConfig) -> Result<Box<dyn StorageBackend>> {
    #[cfg(feature = "storage-sled")]
    {
        let path = _config
            .storage_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("./saikuro_sled"));
        let storage = saikuro_storage::SledStorage::with_config(path, _config.clone())?;
        Ok(Box::new(storage))
    }
    #[cfg(not(feature = "storage-sled"))]
    {
        Err(Error::Storage(
            "Sled backend not available: enable the 'storage-sled' feature".into(),
        ))
    }
}

async fn create_sqlite(_config: &StorageConfig) -> Result<Box<dyn StorageBackend>> {
    #[cfg(feature = "storage-sqlite")]
    {
        let path = _config
            .storage_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("./saikuro.sqlite"));
        let storage = saikuro_storage::SqliteStorage::with_config(path, _config.clone())?;
        Ok(Box::new(storage))
    }
    #[cfg(not(feature = "storage-sqlite"))]
    {
        Err(Error::Storage(
            "SQLite backend not available: enable the 'storage-sqlite' feature".into(),
        ))
    }
}

async fn create_web_storage(config: &StorageConfig) -> Result<Box<dyn StorageBackend>> {
    let storage = saikuro_storage::LocalStorage::with_config(config.clone());
    Ok(Box::new(storage))
}

async fn create_indexeddb(_config: &StorageConfig) -> Result<Box<dyn StorageBackend>> {
    #[cfg(all(target_arch = "wasm32", feature = "wasm-storage"))]
    {
        let storage = saikuro_storage::IndexedDbStorage::with_config(_config.clone());
        return Ok(Box::new(storage));
    }
    Err(Error::Storage(
        "IndexedDB backend is only available on wasm32 with the 'wasm-storage' feature".into(),
    ))
}

async fn create_opfs(_config: &StorageConfig) -> Result<Box<dyn StorageBackend>> {
    #[cfg(all(target_arch = "wasm32", feature = "wasm-storage"))]
    {
        let storage = saikuro_storage::OpfsStorage::with_config(_config.clone());
        return Ok(Box::new(storage));
    }
    Err(Error::Storage(
        "OPFS backend is only available on wasm32 with the 'wasm-storage' feature".into(),
    ))
}

/// Create a transient (in-memory) storage backend.
pub fn create_transient_storage() -> Box<dyn StorageBackend> {
    Box::new(saikuro_storage::InMemoryStorage::new())
}
