use bytes::Bytes;
use saikuro_storage::traits::{FileBackend, KeyValueBackend, Result as KvResult, StorageBackend};
use saikuro_storage::{BackendKind, InMemoryStorage, PersistenceMode, StorageConfig};

#[cfg(feature = "storage-fs")]
use saikuro_storage::FilesystemStorage;
#[cfg(feature = "storage-sled")]
use saikuro_storage::SledStorage;
#[cfg(feature = "storage-sqlite")]
use saikuro_storage::SqliteStorage;

use crate::error::{Error, Result};

/// A concrete storage backend chosen at runtime from [`StorageConfig`].
pub enum Storage {
    InMemory(InMemoryStorage),
    #[cfg(feature = "storage-fs")]
    Filesystem(FilesystemStorage),
    #[cfg(feature = "storage-sled")]
    Sled(SledStorage),
    #[cfg(feature = "storage-sqlite")]
    Sqlite(SqliteStorage),
}

impl KeyValueBackend for Storage {
    fn config(&self) -> &StorageConfig {
        match self {
            Storage::InMemory(b) => b.config(),
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.config(),
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.config(),
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.config(),
        }
    }

    async fn exists(&self, namespace: &str, key: &str) -> KvResult<bool> {
        match self {
            Storage::InMemory(b) => b.exists(namespace, key).await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.exists(namespace, key).await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.exists(namespace, key).await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.exists(namespace, key).await,
        }
    }

    async fn get(&self, namespace: &str, key: &str) -> KvResult<Option<Bytes>> {
        match self {
            Storage::InMemory(b) => b.get(namespace, key).await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.get(namespace, key).await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.get(namespace, key).await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.get(namespace, key).await,
        }
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> KvResult<()> {
        match self {
            Storage::InMemory(b) => b.put(namespace, key, value).await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.put(namespace, key, value).await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.put(namespace, key, value).await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.put(namespace, key, value).await,
        }
    }

    async fn delete(&self, namespace: &str, key: &str) -> KvResult<()> {
        match self {
            Storage::InMemory(b) => b.delete(namespace, key).await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.delete(namespace, key).await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.delete(namespace, key).await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.delete(namespace, key).await,
        }
    }

    async fn list_keys(&self, namespace: &str) -> KvResult<Vec<String>> {
        match self {
            Storage::InMemory(b) => b.list_keys(namespace).await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.list_keys(namespace).await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.list_keys(namespace).await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.list_keys(namespace).await,
        }
    }

    async fn list_namespaces(&self) -> KvResult<Vec<String>> {
        match self {
            Storage::InMemory(b) => b.list_namespaces().await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.list_namespaces().await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.list_namespaces().await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.list_namespaces().await,
        }
    }

    async fn create_namespace(&self, namespace: &str) -> KvResult<()> {
        match self {
            Storage::InMemory(b) => b.create_namespace(namespace).await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.create_namespace(namespace).await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.create_namespace(namespace).await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.create_namespace(namespace).await,
        }
    }

    async fn delete_namespace(&self, namespace: &str) -> KvResult<()> {
        match self {
            Storage::InMemory(b) => b.delete_namespace(namespace).await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.delete_namespace(namespace).await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.delete_namespace(namespace).await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.delete_namespace(namespace).await,
        }
    }

    async fn clear_namespace(&self, namespace: &str) -> KvResult<()> {
        match self {
            Storage::InMemory(b) => b.clear_namespace(namespace).await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.clear_namespace(namespace).await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.clear_namespace(namespace).await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.clear_namespace(namespace).await,
        }
    }
}

impl StorageBackend for Storage {
    fn supports_files(&self) -> bool {
        match self {
            Storage::InMemory(_) => false,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(_) => true,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(_) => false,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(_) => false,
        }
    }

    fn as_file_backend(&self) -> Option<&dyn FileBackend> {
        match self {
            Storage::InMemory(_) => None,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => Some(b),
            #[cfg(feature = "storage-sled")]
            Storage::Sled(_) => None,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(_) => None,
        }
    }

    async fn flush(&self) -> KvResult<()> {
        match self {
            Storage::InMemory(b) => b.flush().await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.flush().await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.flush().await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.flush().await,
        }
    }

    async fn close(&self) -> KvResult<()> {
        match self {
            Storage::InMemory(b) => b.close().await,
            #[cfg(feature = "storage-fs")]
            Storage::Filesystem(b) => b.close().await,
            #[cfg(feature = "storage-sled")]
            Storage::Sled(b) => b.close().await,
            #[cfg(feature = "storage-sqlite")]
            Storage::Sqlite(b) => b.close().await,
        }
    }
}

/// Create a storage backend based on the given configuration.
pub async fn create_storage(config: &StorageConfig) -> Result<Storage> {
    match config.backend {
        BackendKind::Filesystem => return create_filesystem(config).await,
        BackendKind::Sled => return create_sled(config).await,
        BackendKind::Sqlite => return create_sqlite(config).await,
        BackendKind::WebStorage
        | BackendKind::IndexedDb
        | BackendKind::Opfs
        | BackendKind::FsAccess => {
            return Err(Error::Storage(
                "browser/wasm storage backends are not available from the native factory".into(),
            ));
        }
        BackendKind::InMemory => { /* fall through to persistence-based dispatch */ }
    }

    match config.persistence {
        PersistenceMode::Transient | PersistenceMode::BestEffort => Ok(Storage::InMemory(
            InMemoryStorage::with_config(config.clone()),
        )),
        PersistenceMode::Durable => Err(Error::Storage(
            "no durable storage backend selected; set `config.backend` to \
             `BackendKind::Filesystem`, `Sled`, or `Sqlite` on native"
                .into(),
        )),
    }
}

async fn create_filesystem(_config: &StorageConfig) -> Result<Storage> {
    #[cfg(feature = "storage-fs")]
    {
        let path = _config
            .storage_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("./saikuro_data"));
        let storage = FilesystemStorage::with_config(path, _config.clone());
        Ok(Storage::Filesystem(storage))
    }
    #[cfg(not(feature = "storage-fs"))]
    {
        Err(Error::Storage(
            "Filesystem backend not available: enable the 'storage-fs' feature".into(),
        ))
    }
}

async fn create_sled(_config: &StorageConfig) -> Result<Storage> {
    #[cfg(feature = "storage-sled")]
    {
        let path = _config
            .storage_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("./saikuro_sled"));
        let storage = SledStorage::with_config(path, _config.clone())?;
        Ok(Storage::Sled(storage))
    }
    #[cfg(not(feature = "storage-sled"))]
    {
        Err(Error::Storage(
            "Sled backend not available: enable the 'storage-sled' feature".into(),
        ))
    }
}

async fn create_sqlite(_config: &StorageConfig) -> Result<Storage> {
    #[cfg(feature = "storage-sqlite")]
    {
        let path = _config
            .storage_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("./saikuro_sqlite"));
        let storage = SqliteStorage::with_config(path, _config.clone())?;
        Ok(Storage::Sqlite(storage))
    }
    #[cfg(not(feature = "storage-sqlite"))]
    {
        Err(Error::Storage(
            "SQLite backend not available: enable the 'storage-sqlite' feature".into(),
        ))
    }
}

/// Create a transient (in-memory) storage backend.
pub fn create_transient_storage() -> Storage {
    Storage::InMemory(InMemoryStorage::new())
}
