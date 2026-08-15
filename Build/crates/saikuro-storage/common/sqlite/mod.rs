#[cfg(feature = "native")]
mod actor;
#[cfg(not(feature = "native"))]
mod direct;

#[cfg(feature = "native")]
pub use actor::SqliteStorage;
#[cfg(not(feature = "native"))]
pub use direct::SqliteStorage;

use alloc::string::String;
use alloc::vec::Vec;

use bytes::Bytes;

use graphitesql::exec::eval::Params;
use graphitesql::Value;

use crate::shared::config::StorageConfig;
use crate::shared::traits::{KeyValueBackend, StorageBackend};
use saikuro_event::{Result, SaikuroError};

/// Schema for the single key-value table shared by every engine.
pub(crate) const CREATE_KV: &str = "
    CREATE TABLE IF NOT EXISTS saikuro_kv (
        namespace TEXT NOT NULL,
        key TEXT NOT NULL,
        value BLOB NOT NULL,
        PRIMARY KEY (namespace, key)
    )
";

/// Execution primitive shared by the actor and direct backends.
pub(crate) trait RawSqlite {
    async fn query(&self, sql: &str, params: Params) -> Result<graphitesql::QueryResult>;
    async fn batch(&self, sql: &str) -> Result<()>;
}

/// Map a `graphitesql` error into the crate's unified error type.
pub(crate) fn map_err(e: graphitesql::Error) -> SaikuroError {
    SaikuroError::internal(format!("graphitesql error: {e:?}"))
}

fn apply_prefix(config: &StorageConfig, namespace: &str) -> String {
    match &config.namespace_prefix {
        Some(prefix) => format!("{prefix}:{namespace}"),
        None => namespace.to_owned(),
    }
}

fn strip_prefix(config: &StorageConfig, stored: &str) -> String {
    match &config.namespace_prefix {
        Some(prefix) => {
            let prefixed = format!("{prefix}:");
            if stored.starts_with(&prefixed) {
                stored[prefixed.len()..].to_owned()
            } else {
                stored.to_owned()
            }
        }
        None => stored.to_owned(),
    }
}

fn ns_key_params(namespace: &str, key: &str) -> Params {
    Params {
        positional: vec![Value::Text(namespace.to_owned()), Value::Text(key.to_owned())],
        named: Vec::new(),
    }
}

fn blob_of(row: &[Value]) -> Option<Vec<u8>> {
    row.first().and_then(|v| match v {
        Value::Blob(b) => Some(b.clone()),
        _ => None,
    })
}

impl KeyValueBackend for SqliteStorage {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let ns = apply_prefix(self.config(), namespace);
        let res = self
            .query(
                "SELECT 1 FROM saikuro_kv WHERE namespace = ?1 AND key = ?2",
                ns_key_params(&ns, key),
            )
            .await?;
        Ok(!res.rows.is_empty())
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let ns = apply_prefix(self.config(), namespace);
        let res = self
            .query(
                "SELECT value FROM saikuro_kv WHERE namespace = ?1 AND key = ?2",
                ns_key_params(&ns, key),
            )
            .await?;
        Ok(res.rows.first().and_then(blob_of).map(Bytes::from))
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let ns = apply_prefix(self.config(), namespace);
        self.query(
            "INSERT OR REPLACE INTO saikuro_kv (namespace, key, value) VALUES (?1, ?2, ?3)",
            Params {
                positional: vec![
                    Value::Text(ns),
                    Value::Text(key.to_owned()),
                    Value::Blob(value.to_vec()),
                ],
                named: Vec::new(),
            },
        )
        .await?;
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let ns = apply_prefix(self.config(), namespace);
        self.query(
            "DELETE FROM saikuro_kv WHERE namespace = ?1 AND key = ?2",
            ns_key_params(&ns, key),
        )
        .await?;
        Ok(())
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let ns = apply_prefix(self.config(), namespace);
        let res = self
            .query(
                "SELECT key FROM saikuro_kv WHERE namespace = ?1 ORDER BY key",
                Params {
                    positional: vec![Value::Text(ns)],
                    named: Vec::new(),
                },
            )
            .await?;
        Ok(res
            .rows
            .iter()
            .filter_map(|row| match row.first() {
                Some(Value::Text(t)) => Some(t.clone()),
                _ => None,
            })
            .collect())
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let res = self
            .query(
                "SELECT DISTINCT namespace FROM saikuro_kv ORDER BY namespace",
                Params {
                    positional: Vec::new(),
                    named: Vec::new(),
                },
            )
            .await?;
        let prefix = self.config().namespace_prefix.clone();
        Ok(res
            .rows
            .iter()
            .filter_map(|row| match row.first() {
                Some(Value::Text(t)) => Some(t.clone()),
                _ => None,
            })
            .filter(|n| match &prefix {
                Some(p) => n.starts_with(&format!("{p}:")),
                None => true,
            })
            .map(|n| strip_prefix(self.config(), &n))
            .collect())
    }

    async fn create_namespace(&self, _namespace: &str) -> Result<()> {
        Ok(())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let ns = apply_prefix(self.config(), namespace);
        self.query(
            "DELETE FROM saikuro_kv WHERE namespace = ?1",
            Params {
                positional: vec![Value::Text(ns)],
                named: Vec::new(),
            },
        )
        .await?;
        Ok(())
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        self.delete_namespace(namespace).await
    }
}

impl StorageBackend for SqliteStorage {
    fn supports_files(&self) -> bool {
        false
    }
}
