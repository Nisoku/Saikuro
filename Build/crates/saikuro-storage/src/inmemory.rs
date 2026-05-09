//! In-memory storage backend using DashMap.
//!
//! This is the reference implementation and the default backend for
//! Saikuro's ephemeral storage needs.

use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::debug;

use super::{
    config::StorageConfig,
    error::{Result, StorageError},
    traits::{KeyValueBackend, StorageBackend},
};

/// An in-memory namespace containing key-value pairs.
type NamespaceStore = DashMap<String, Bytes>;

/// A thread-safe, in-memory storage backend.
///
/// Uses `DashMap` for lock-free concurrent access. All data is ephemeral
/// and will be lost when the backend is dropped.
pub struct InMemoryStorage {
    config: StorageConfig,
    namespaces: DashMap<String, Arc<NamespaceStore>>,
}

impl InMemoryStorage {
    /// Create a new in-memory storage backend with default configuration.
    pub fn new() -> Self {
        Self::with_config(StorageConfig::default())
    }

    /// Create a new in-memory storage backend with custom configuration.
    pub fn with_config(config: StorageConfig) -> Self {
        let namespaces = DashMap::new();

        debug!(
            persistence = ?config.persistence,
            "in-memory storage backend initialized"
        );

        Self { config, namespaces }
    }

    /// Get or create a namespace.
    fn get_or_create_namespace(&self, namespace: &str) -> Result<Arc<NamespaceStore>> {
        if let Some(ns) = self.namespaces.get(namespace) {
            return Ok(ns.clone());
        }

        if !self.config.auto_create_namespaces {
            return Err(StorageError::namespace_not_found(namespace));
        }

        let ns = Arc::new(NamespaceStore::new());
        self.namespaces.insert(namespace.to_owned(), ns.clone());
        Ok(ns)
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueBackend for InMemoryStorage {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let ns = self.get_or_create_namespace(namespace)?;
        Ok(ns.contains_key(key))
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let ns = self.get_or_create_namespace(namespace)?;
        Ok(ns.get(key).map(|v| v.clone()))
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let ns = self.get_or_create_namespace(namespace)?;
        ns.insert(key.to_owned(), value);
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let ns = self.get_or_create_namespace(namespace)?;
        ns.remove(key);
        Ok(())
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let ns = self.get_or_create_namespace(namespace)?;
        Ok(ns.iter().map(|entry| entry.key().clone()).collect())
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        Ok(self
            .namespaces
            .iter()
            .map(|entry| entry.key().clone())
            .collect())
    }

    async fn create_namespace(&self, namespace: &str) -> Result<()> {
        if self.namespaces.contains_key(namespace) {
            return Err(StorageError::NamespaceAlreadyExists(namespace.to_owned()));
        }
        self.namespaces
            .insert(namespace.to_owned(), Arc::new(NamespaceStore::new()));
        Ok(())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        self.namespaces.remove(namespace);
        Ok(())
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        let ns = self.get_or_create_namespace(namespace)?;
        ns.clear();
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for InMemoryStorage {
    fn supports_files(&self) -> bool {
        false
    }
}
