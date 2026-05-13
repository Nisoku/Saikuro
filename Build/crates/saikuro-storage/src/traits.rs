//! Storage backend traits and utilities.

use async_trait::async_trait;
use bytes::Bytes;
use serde::{de::DeserializeOwned, Serialize};

use super::{config::StorageConfig, error::Result};

/// A key-value storage interface with namespace support.
#[async_trait]
pub trait KeyValueBackend: Send + Sync + 'static {
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

/// A file-like storage interface for hierarchical storage.
#[async_trait]
pub trait FileBackend: Send + Sync + 'static {
    /// Read a file's contents.
    async fn read_file(&self, path: &str) -> Result<Bytes>;

    /// Write a file's contents, creating it if it doesn't exist.
    async fn write_file(&self, path: &str, content: Bytes) -> Result<()>;

    /// Append content to an existing file.
    async fn append_file(&self, path: &str, content: Bytes) -> Result<()>;

    /// Delete a file.
    async fn delete_file(&self, path: &str) -> Result<()>;

    /// Check if a file exists.
    async fn file_exists(&self, path: &str) -> Result<bool>;

    /// List files in a directory.
    async fn list_dir(&self, path: &str) -> Result<Vec<String>>;

    /// Create a directory.
    async fn create_dir(&self, path: &str) -> Result<()>;

    /// Delete a directory and all its contents.
    async fn delete_dir(&self, path: &str) -> Result<()>;
}

/// Unified storage backend trait combining key-value and file operations.
#[async_trait]
pub trait StorageBackend: KeyValueBackend {
    /// Check if this backend supports file operations.
    fn supports_files(&self) -> bool;

    /// Get the file backend, if supported.
    fn as_file_backend(&self) -> Option<&dyn FileBackend> {
        None
    }

    /// Flush any pending writes to durable storage.
    async fn flush(&self) -> Result<()> {
        Ok(())
    }

    /// Close the backend and release any resources.
    async fn close(&self) -> Result<()> {
        Ok(())
    }
}

/// Extension methods for KeyValueBackend providing JSON serialization.
#[async_trait]
pub trait KeyValueBackendExt: KeyValueBackend {
    /// Get a JSON-serialized value.
    async fn get_json<T: DeserializeOwned>(&self, namespace: &str, key: &str) -> Result<Option<T>> {
        match self.get(namespace, key).await? {
            Some(bytes) => {
                let value = serde_json::from_slice(&bytes)
                    .map_err(|e| super::error::StorageError::deserialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Put a JSON-serialized value.
    async fn put_json<T: Serialize + Sync>(
        &self,
        namespace: &str,
        key: &str,
        value: &T,
    ) -> Result<()> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| super::error::StorageError::serialization(e.to_string()))?;
        self.put(namespace, key, Bytes::from(bytes)).await
    }

    /// Get a MessagePack-serialized value.
    async fn get_msgpack<T: DeserializeOwned>(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<T>> {
        match self.get(namespace, key).await? {
            Some(bytes) => {
                let value = rmp_serde::from_slice(&bytes)
                    .map_err(|e| super::error::StorageError::deserialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Put a MessagePack-serialized value.
    async fn put_msgpack<T: Serialize + Sync>(
        &self,
        namespace: &str,
        key: &str,
        value: &T,
    ) -> Result<()> {
        let bytes = rmp_serde::to_vec_named(value)
            .map_err(|e| super::error::StorageError::serialization(e.to_string()))?;
        self.put(namespace, key, Bytes::from(bytes)).await
    }
}

impl<B: KeyValueBackend> KeyValueBackendExt for B {}
