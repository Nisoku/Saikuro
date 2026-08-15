use alloc::string::String;
use alloc::vec::Vec;
use bytes::Bytes;
use saikuro_event::Result;

/// A file-like storage interface for hierarchical storage.
#[allow(async_fn_in_trait)]
pub trait FileBackend: 'static {
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
