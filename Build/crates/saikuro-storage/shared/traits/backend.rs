use saikuro_event::Result;

use super::file::FileBackend;
use super::kv::KeyValueBackend;

/// Unified storage backend trait combining key-value and file operations.
#[allow(async_fn_in_trait)]
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
