use saikuro_storage::traits::StorageBackend;
use saikuro_storage::{PersistenceMode, StorageConfig};

#[cfg(not(all(target_arch = "wasm32", feature = "wasm-storage")))]
use crate::error::Error;
use crate::error::Result;

/// Create a storage backend based on the given configuration.
///
/// Dispatch rules (mirrors the transport [`connect`] pattern):
///
/// | `persistence`      | native                          | wasm32 (with `wasm-storage`)  |
/// |--------------------|---------------------------------|-------------------------------|
/// | `Transient`        | `InMemoryStorage`               | `InMemoryStorage`             |
/// | `BestEffort`       | `InMemoryStorage`               | `LocalStorage`                |
/// | `Durable`          | error (no durable native backend yet) | `IndexedDbStorage`      |
pub async fn create_storage(config: &StorageConfig) -> Result<Box<dyn StorageBackend>> {
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

            // Not available on this platform
            #[cfg(not(all(target_arch = "wasm32", feature = "wasm-storage")))]
            {
                Err(Error::Storage(
                    "no durable storage backend available on this platform; \
                     enable the 'wasm-storage' feature on wasm32, or use \
                     PersistenceMode::Transient / BestEffort"
                        .into(),
                ))
            }
        }
    }
}

/// Create a transient (in-memory) storage backend.
pub fn create_transient_storage() -> Box<dyn StorageBackend> {
    Box::new(saikuro_storage::InMemoryStorage::new())
}
