use alloc::string::ToString;
use bytes::Bytes;
use saikuro_event::Result;
use serde::{de::DeserializeOwned, Serialize};

use super::kv::KeyValueBackend;

/// Extension methods for [`KeyValueBackend`] providing JSON/MessagePack helpers.
#[allow(async_fn_in_trait)]
pub trait KeyValueBackendExt: KeyValueBackend {
    /// Get a JSON-serialized value.
    async fn get_json<T: DeserializeOwned>(&self, namespace: &str, key: &str) -> Result<Option<T>> {
        match self.get(namespace, key).await? {
            Some(bytes) => {
                let value = serde_json::from_slice(&bytes)
                    .map_err(|e| saikuro_event::SaikuroError::deserialization(e.to_string()))?;
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
            .map_err(|e| saikuro_event::SaikuroError::serialization(e.to_string()))?;
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
                let value = saikuro_core::msgpack::from_slice(&bytes)
                    .map_err(|e| saikuro_event::SaikuroError::deserialization(e.to_string()))?;
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
        let bytes = saikuro_core::msgpack::to_vec(value)
            .map_err(|e| saikuro_event::SaikuroError::serialization(e.to_string()))?;
        self.put(namespace, key, Bytes::from(bytes)).await
    }
}

impl<B: KeyValueBackend> KeyValueBackendExt for B {}
