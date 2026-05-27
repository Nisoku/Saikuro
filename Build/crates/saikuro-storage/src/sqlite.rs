use async_trait::async_trait;
use bytes::Bytes;
use rusqlite::OptionalExtension;
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

const CREATE_KV: &str = "
    CREATE TABLE IF NOT EXISTS saikuro_kv (
        namespace TEXT NOT NULL,
        key TEXT NOT NULL,
        value BLOB NOT NULL,
        PRIMARY KEY (namespace, key)
    )
";

/// A SQLite-backed persistent key-value storage backend.
///
/// Stores namespaced key-value pairs in a single table.  All I/O is
/// dispatched to the blocking thread pool.
pub struct SqliteStorage {
    config: StorageConfig,
    conn: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
}

impl SqliteStorage {
    /// Open or create a SQLite database at the given path.
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::with_config(path, StorageConfig::default())
    }

    /// Open or create a SQLite database with a custom configuration.
    pub fn with_config(
        path: impl AsRef<std::path::Path>,
        config: StorageConfig,
    ) -> Result<Self> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| StorageError::internal(format!("sqlite open: {e}")))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| StorageError::internal(format!("sqlite pragma: {e}")))?;
        conn.execute_batch(CREATE_KV)
            .map_err(|e| StorageError::internal(format!("sqlite create table: {e}")))?;
        Ok(Self {
            config,
            conn: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        })
    }

    /// Open an in-memory SQLite database (useful for testing).
    pub fn temporary() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| StorageError::internal(format!("sqlite in-memory: {e}")))?;
        conn.execute_batch(CREATE_KV)
            .map_err(|e| StorageError::internal(format!("sqlite create table: {e}")))?;
        Ok(Self {
            config: StorageConfig::default(),
            conn: std::sync::Arc::new(std::sync::Mutex::new(conn)),
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
impl KeyValueBackend for SqliteStorage {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let conn = self.conn.clone();
        let ns = self.apply_prefix(namespace);
        let key = key.to_owned();
        block(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare_cached(
                    "SELECT 1 FROM saikuro_kv WHERE namespace = ?1 AND key = ?2",
                )
                .map_err(|e| StorageError::internal(format!("sqlite prepare: {e}")))?;
            let exists = stmt
                .exists(rusqlite::params![ns, key])
                .map_err(|e| StorageError::internal(format!("sqlite exists: {e}")))?;
            Ok(exists)
        })
        .await
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let conn = self.conn.clone();
        let ns = self.apply_prefix(namespace);
        let key = key.to_owned();
        block(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare_cached(
                    "SELECT value FROM saikuro_kv WHERE namespace = ?1 AND key = ?2",
                )
                .map_err(|e| StorageError::internal(format!("sqlite prepare: {e}")))?;
            let result: Option<Vec<u8>> = stmt
                .query_row(rusqlite::params![ns, key], |row| row.get(0))
                .optional()
                .map_err(|e| StorageError::internal(format!("sqlite query: {e}")))?;
            Ok(result.map(Bytes::from))
        })
        .await
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let conn = self.conn.clone();
        let ns = self.apply_prefix(namespace);
        let key = key.to_owned();
        let val = value.to_vec();
        block(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO saikuro_kv (namespace, key, value) VALUES (?1, ?2, ?3)
                 ON CONFLICT(namespace, key) DO UPDATE SET value = excluded.value",
                rusqlite::params![ns, key, val],
            )
            .map_err(|e| StorageError::internal(format!("sqlite insert: {e}")))?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let conn = self.conn.clone();
        let ns = self.apply_prefix(namespace);
        let key = key.to_owned();
        block(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "DELETE FROM saikuro_kv WHERE namespace = ?1 AND key = ?2",
                rusqlite::params![ns, key],
            )
            .map_err(|e| StorageError::internal(format!("sqlite delete: {e}")))?;
            Ok(())
        })
        .await
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let conn = self.conn.clone();
        let ns = self.apply_prefix(namespace);
        block(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare_cached(
                    "SELECT key FROM saikuro_kv WHERE namespace = ?1 ORDER BY key",
                )
                .map_err(|e| StorageError::internal(format!("sqlite prepare: {e}")))?;
            let keys: Vec<String> = stmt
                .query_map(rusqlite::params![ns], |row| row.get(0))
                .map_err(|e| StorageError::internal(format!("sqlite query_map: {e}")))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(keys)
        })
        .await
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let conn = self.conn.clone();
        let prefix = self.config.namespace_prefix.clone();
        block(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare_cached("SELECT DISTINCT namespace FROM saikuro_kv ORDER BY namespace")
                .map_err(|e| StorageError::internal(format!("sqlite prepare: {e}")))?;
            let names: Vec<String> = stmt
                .query_map([], |row| row.get(0))
                .map_err(|e| StorageError::internal(format!("sqlite query_map: {e}")))?
                .filter_map(|r| r.ok())
                .map(|n: String| match &prefix {
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

    async fn create_namespace(&self, _namespace: &str) -> Result<()> {
        Ok(())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let conn = self.conn.clone();
        let ns = self.apply_prefix(namespace);
        block(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "DELETE FROM saikuro_kv WHERE namespace = ?1",
                rusqlite::params![ns],
            )
            .map_err(|e| StorageError::internal(format!("sqlite delete namespace: {e}")))?;
            Ok(())
        })
        .await
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        self.delete_namespace(namespace).await
    }
}

#[async_trait]
impl StorageBackend for SqliteStorage {
    fn supports_files(&self) -> bool {
        false
    }
}
