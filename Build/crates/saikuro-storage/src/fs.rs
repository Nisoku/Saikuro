use async_trait::async_trait;
use bytes::Bytes;
use std::path::{Path, PathBuf};
use tokio::task::spawn_blocking;

use super::{
    config::StorageConfig,
    error::{Result, StorageError},
    traits::{FileBackend, KeyValueBackend, StorageBackend},
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

/// A filesystem-backed storage backend for native targets.
///
/// Stores key-value data under `{base_dir}/kv/namespaces/{ns}/{key}` and
/// file data under `{base_dir}/files/{path}`.
pub struct FilesystemStorage {
    config: StorageConfig,
    kv_root: PathBuf,
    files_root: PathBuf,
}

impl FilesystemStorage {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self::with_config(base_dir, StorageConfig::default())
    }

    pub fn with_config(base_dir: impl Into<PathBuf>, config: StorageConfig) -> Self {
        let base_dir: PathBuf = base_dir.into();
        let kv_root = base_dir.join("kv").join("namespaces");
        let files_root = base_dir.join("files");
        Self {
            config,
            kv_root,
            files_root,
        }
    }

    fn apply_prefix(&self, namespace: &str) -> String {
        match &self.config.namespace_prefix {
            Some(prefix) => format!("{prefix}:{namespace}"),
            None => namespace.to_owned(),
        }
    }
}

// -- helpers run on the blocking pool --

fn exists(path: &Path) -> Result<bool> {
    Ok(path.try_exists().map_err(StorageError::from)?)
}

fn read_bytes(path: &Path) -> Result<Option<Bytes>> {
    match std::fs::read(path) {
        Ok(data) => Ok(Some(Bytes::from(data))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(StorageError::from(e)),
    }
}

fn write_bytes(path: &Path, value: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, value)?;
    Ok(())
}

fn append_bytes(path: &Path, value: &[u8]) -> Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(value)?;
    Ok(())
}

fn delete(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(StorageError::from(e)),
    }
}

fn list_files(dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut entries: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    entries.sort();
    Ok(entries)
}

fn list_subdirs(dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut entries: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    entries.sort();
    Ok(entries)
}

fn remove_dir(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(StorageError::from(e)),
    }
}

fn clear_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(&p)?;
        } else {
            std::fs::remove_file(&p)?;
        }
    }
    Ok(())
}

fn strip_ns_prefix<'a>(prefix: &Option<String>, name: &'a str) -> String {
    match prefix {
        Some(p) => {
            let pstr = format!("{p}:");
            if name.starts_with(&pstr) {
                name[pstr.len()..].to_owned()
            } else {
                name.to_owned()
            }
        }
        None => name.to_owned(),
    }
}

#[async_trait]
impl KeyValueBackend for FilesystemStorage {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let path = self.kv_root.join(self.apply_prefix(namespace)).join(key);
        block(move || exists(&path)).await
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let path = self.kv_root.join(self.apply_prefix(namespace)).join(key);
        block(move || read_bytes(&path)).await
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let path = self.kv_root.join(self.apply_prefix(namespace)).join(key);
        let val = value.to_vec();
        block(move || write_bytes(&path, &val)).await
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let path = self.kv_root.join(self.apply_prefix(namespace)).join(key);
        block(move || delete(&path)).await
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let path = self.kv_root.join(self.apply_prefix(namespace));
        block(move || list_files(&path)).await
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let path = self.kv_root.clone();
        let prefix = self.config.namespace_prefix.clone();
        block(move || {
            let raw = list_subdirs(&path)?;
            Ok(raw
                .into_iter()
                .map(|n| strip_ns_prefix(&prefix, &n))
                .collect())
        })
        .await
    }

    async fn create_namespace(&self, namespace: &str) -> Result<()> {
        let ns = namespace.to_owned();
        let path = self.kv_root.join(self.apply_prefix(namespace));
        block(move || {
            if path.exists() {
                return Err(StorageError::namespace_already_exists(&ns));
            }
            std::fs::create_dir_all(&path)?;
            Ok(())
        })
        .await
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let path = self.kv_root.join(self.apply_prefix(namespace));
        block(move || remove_dir(&path)).await
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        let path = self.kv_root.join(self.apply_prefix(namespace));
        block(move || clear_dir(&path)).await
    }
}

#[async_trait]
impl FileBackend for FilesystemStorage {
    async fn read_file(&self, path: &str) -> Result<Bytes> {
        let full = self.files_root.join(path);
        let path_owned = path.to_owned();
        block(move || read_bytes(&full))
            .await?
            .ok_or_else(|| StorageError::key_not_found(path_owned))
    }

    async fn write_file(&self, path: &str, content: Bytes) -> Result<()> {
        let full = self.files_root.join(path);
        let bytes = content.to_vec();
        block(move || write_bytes(&full, &bytes)).await
    }

    async fn append_file(&self, path: &str, content: Bytes) -> Result<()> {
        let full = self.files_root.join(path);
        let bytes = content.to_vec();
        block(move || append_bytes(&full, &bytes)).await
    }

    async fn delete_file(&self, path: &str) -> Result<()> {
        let full = self.files_root.join(path);
        block(move || delete(&full)).await
    }

    async fn file_exists(&self, path: &str) -> Result<bool> {
        let full = self.files_root.join(path);
        block(move || exists(&full)).await
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        let full = self.files_root.join(path);
        block(move || list_files(&full)).await
    }

    async fn create_dir(&self, path: &str) -> Result<()> {
        let full = self.files_root.join(path);
        block(move || {
            std::fs::create_dir_all(&full)?;
            Ok(())
        })
        .await
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let full = self.files_root.join(path);
        block(move || remove_dir(&full)).await
    }
}

#[async_trait]
impl StorageBackend for FilesystemStorage {
    fn supports_files(&self) -> bool {
        true
    }

    fn as_file_backend(&self) -> Option<&dyn FileBackend> {
        Some(self)
    }
}
