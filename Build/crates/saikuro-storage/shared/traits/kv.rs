use alloc::string::String;
use alloc::vec::Vec;
use bytes::Bytes;
use saikuro_event::Result;

use crate::config::StorageConfig;

/// A key-value storage interface with namespace support.
#[allow(async_fn_in_trait)]
pub trait KeyValueBackend: 'static {
    /// Get the configuration for this backend.
    fn config(&self) -> &StorageConfig;

    /// Check if a key exists in a namespace.
    async fn exists(&self, namespace: &str, key: &str) -> Result<bool>;

    /// Get raw bytes for a key.
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>>;

    /// Put raw bytes for a key.
    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()>;

    /// Delete a key.
    async fn delete(&self, namespace: &str, key: &str) -> Result<()>;

    /// List all keys in a namespace.
    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>>;

    /// List all namespaces.
    async fn list_namespaces(&self) -> Result<Vec<String>>;

    /// Create a namespace explicitly.
    async fn create_namespace(&self, namespace: &str) -> Result<()>;

    /// Delete a namespace and all its keys.
    async fn delete_namespace(&self, namespace: &str) -> Result<()>;

    /// Clear all keys in a namespace without deleting the namespace.
    async fn clear_namespace(&self, namespace: &str) -> Result<()>;
}
