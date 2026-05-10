//! Error types for the storage backend abstraction.

use std::io;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, StorageError>;

/// Error type for all storage backend operations.
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("key not found: {0}")]
    KeyNotFound(String),

    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("key already exists: {0}")]
    KeyAlreadyExists(String),

    #[error("namespace already exists: {0}")]
    NamespaceAlreadyExists(String),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("backend not available: {0}")]
    BackendNotAvailable(String),

    #[error("operation not supported: {0}")]
    OperationNotSupported(String),

    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl StorageError {
    pub fn key_not_found(key: impl Into<String>) -> Self {
        StorageError::KeyNotFound(key.into())
    }

    pub fn namespace_not_found(ns: impl Into<String>) -> Self {
        StorageError::NamespaceNotFound(ns.into())
    }

    pub fn serialization(msg: impl Into<String>) -> Self {
        StorageError::Serialization(msg.into())
    }

    pub fn deserialization(msg: impl Into<String>) -> Self {
        StorageError::Deserialization(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        StorageError::Internal(msg.into())
    }

    pub fn not_supported(msg: impl Into<String>) -> Self {
        StorageError::OperationNotSupported(msg.into())
    }

    pub fn key_already_exists(key: impl Into<String>) -> Self {
        StorageError::KeyAlreadyExists(key.into())
    }

    pub fn namespace_already_exists(ns: impl Into<String>) -> Self {
        StorageError::NamespaceAlreadyExists(ns.into())
    }

    pub fn backend_not_available(msg: impl Into<String>) -> Self {
        StorageError::BackendNotAvailable(msg.into())
    }

    pub fn quota_exceeded(msg: impl Into<String>) -> Self {
        StorageError::QuotaExceeded(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        StorageError::Timeout(msg.into())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        if e.is_io() {
            StorageError::Io(io::Error::other(e))
        } else {
            StorageError::Deserialization(e.to_string())
        }
    }
}

impl From<rmp_serde::encode::Error> for StorageError {
    fn from(e: rmp_serde::encode::Error) -> Self {
        StorageError::Serialization(e.to_string())
    }
}

impl From<rmp_serde::decode::Error> for StorageError {
    fn from(e: rmp_serde::decode::Error) -> Self {
        StorageError::Deserialization(e.to_string())
    }
}
