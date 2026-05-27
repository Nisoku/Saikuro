use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use tokio::task::spawn_blocking;

use super::{
    config::StorageConfig,
    error::{Result, StorageError},
    traits::{KeyValueBackend, StorageBackend},
};

/// Spawn blocking I/O, converting [`JoinError`] to [`StorageError`].
async fn block<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    spawn_blocking(f)
        .await
        .map_err(|e| StorageError::internal(format!("blocking task failed: {e}")))?
}

/// A sled-backed persistent key-value storage backend.
///
/// Each namespace maps to a sled [`Tree`] within a single database file.
/// All I/O is dispatched to the blocking thread pool.
pub struct SledStorage {
    config: StorageConfig,
    db: Arc<sled::Db>,
}

impl SledStorage {
    /// Open or create a sled database at the given path.
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::with_config(path, StorageConfig::default())
    }

    /// Open or create a sled database with a custom configuration.
    pub fn with_config(
        path: impl AsRef<std::path::Path>,
        config: StorageConfig,
    ) -> Result<Self> {
        let db = sled::open(path).map_err(|e| StorageError::internal(format!("sled open: {e}")))?;
        Ok(Self {
            config,
            db: Arc::new(db),
        })
    }

    /// Open an in-memory sled database (useful for testing).
    pub fn temporary() -> Result<Self> {
        let db =
            sled::Config::default().temporary(true).open().map_err(|e| {
                StorageError::internal(format!("sled temporary: {e}"))
            })?;
        Ok(Self {
            config: StorageConfig::default(),
            db: Arc::new(db),
        })
    }

    fn apply_prefix(&self, namespace: &str) -> String {
        match &self.config.namespace_prefix {
            Some(prefix) => format!("{prefix}:{namespace}"),
            None => namespace.to_owned(),
        }
    }

    #[allow(dead_code)]
    fn strip_prefix(&self, stored: &str) -> String {
        match &self.config.namespace_prefix {
            Some(prefix) => {
                let prefix_str = format!("{prefix}:");
                if stored.starts_with(&prefix_str) {
                    stored[prefix_str.len()..].to_owned()
                } else {
                    stored.to_owned()
                }
            }
            None => stored.to_owned(),
        }
    }
}

#[async_trait]
impl KeyValueBackend for SledStorage {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let db = self.db.clone();
        let ns = self.apply_prefix(namespace);
        let key = key.to_owned();
        block(move || {
            let tree = db
                .open_tree(&ns)
                .map_err(|e| StorageError::internal(format!("sled tree: {e}")))?;
            Ok(tree.contains_key(key.as_bytes()).map_err(|e| {
                StorageError::internal(format!("sled contains_key: {e}"))
            })?)
        })
        .await
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let db = self.db.clone();
        let ns = self.apply_prefix(namespace);
        let key = key.to_owned();
        block(move || {
            let tree = db
                .open_tree(&ns)
                .map_err(|e| StorageError::internal(format!("sled tree: {e}")))?;
            match tree.get(key.as_bytes()) {
                Ok(Some(iv)) => Ok(Some(Bytes::from(iv.to_vec()))),
                Ok(None) => Ok(None),
                Err(e) => Err(StorageError::internal(format!("sled get: {e}"))),
            }
        })
        .await
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let db = self.db.clone();
        let ns = self.apply_prefix(namespace);
        let key = key.to_owned();
        let val = value.to_vec();
        block(move || {
            let tree = db
                .open_tree(&ns)
                .map_err(|e| StorageError::internal(format!("sled tree: {e}")))?;
            tree.insert(key.as_bytes(), val)
                .map_err(|e| StorageError::internal(format!("sled insert: {e}")))?;
            db.flush().map_err(|e| StorageError::internal(format!("sled flush: {e}")))?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let db = self.db.clone();
        let ns = self.apply_prefix(namespace);
        let key = key.to_owned();
        block(move || {
            let tree = db
                .open_tree(&ns)
                .map_err(|e| StorageError::internal(format!("sled tree: {e}")))?;
            tree.remove(key.as_bytes())
                .map_err(|e| StorageError::internal(format!("sled remove: {e}")))?;
            Ok(())
        })
        .await
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let db = self.db.clone();
        let ns = self.apply_prefix(namespace);
        block(move || {
            let tree = db
                .open_tree(&ns)
                .map_err(|e| StorageError::internal(format!("sled tree: {e}")))?;
            let keys: Vec<String> = tree
                .iter()
                .keys()
                .filter_map(|r| r.ok())
                .filter_map(|k| String::from_utf8(k.to_vec()).ok())
                .collect();
            Ok(keys)
        })
        .await
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let db = self.db.clone();
        let prefix = self.config.namespace_prefix.clone();
        block(move || {
            let names: Vec<String> = db
                .tree_names()
                .into_iter()
                .filter_map(|n| String::from_utf8(n.to_vec()).ok())
                .filter(|n| !n.is_empty() && n != "__sled__default")
                .map(|n| match &prefix {
                    Some(p) => {
                        let pstr = format!("{p}:");
                        if n.starts_with(&pstr) {
                            n[pstr.len()..].to_owned()
                        } else {
                            n
                        }
                    }
                    None => n,
                })
                .collect();
            Ok(names)
        })
        .await
    }

    async fn create_namespace(&self, namespace: &str) -> Result<()> {
        let db = self.db.clone();
        let ns = self.apply_prefix(namespace);
        block(move || {
            db.open_tree(&ns)
                .map_err(|e| StorageError::internal(format!("sled create tree: {e}")))?;
            Ok(())
        })
        .await
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let db = self.db.clone();
        let ns = self.apply_prefix(namespace);
        block(move || {
            db.drop_tree(&ns)
                .map_err(|e| StorageError::internal(format!("sled drop tree: {e}")))?;
            Ok(())
        })
        .await
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        let db = self.db.clone();
        let ns = self.apply_prefix(namespace);
        block(move || {
            let tree = db
                .open_tree(&ns)
                .map_err(|e| StorageError::internal(format!("sled tree: {e}")))?;
            tree.clear()
                .map_err(|e| StorageError::internal(format!("sled clear: {e}")))?;
            Ok(())
        })
        .await
    }
}

#[async_trait]
impl StorageBackend for SledStorage {
    fn supports_files(&self) -> bool {
        false
    }
}
