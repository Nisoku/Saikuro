use bytes::Bytes;
use std::path::{Component, Path, PathBuf};
use tokio::task::spawn_blocking;

use crate::config::StorageConfig;
use crate::traits::{FileBackend, KeyValueBackend, StorageBackend};
use saikuro_event::{Result, SaikuroError};

/// Spawn blocking I/O, converting [`JoinError`] to [`SaikuroError`].
async fn block<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    spawn_blocking(f)
        .await
        .map_err(|e| SaikuroError::internal(format!("blocking task failed: {e}")))?
}

/// A filesystem-backed storage backend for native targets.
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

fn exists(path: &Path) -> Result<bool> {
    path.try_exists().map_err(SaikuroError::from)
}

fn read_bytes(path: &Path) -> Result<Option<Bytes>> {
    match std::fs::read(path) {
        Ok(data) => Ok(Some(Bytes::from(data))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(SaikuroError::from(e)),
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
        Err(e) => Err(SaikuroError::from(e)),
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
        Err(e) => Err(SaikuroError::from(e)),
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

fn safe_join(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel = Path::new(rel);
    if rel.is_absolute() {
        return Err(SaikuroError::internal(format!(
            "path must not be absolute: {rel:?}"
        )));
    }
    for comp in rel.components() {
        if matches!(comp, Component::ParentDir) {
            return Err(SaikuroError::internal(format!(
                "path must not contain '..': {rel:?}"
            )));
        }
    }
    Ok(root.join(rel))
}

fn strip_ns_prefix(prefix: &Option<String>, name: &str) -> String {
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
        block(move || match std::fs::create_dir_all(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(SaikuroError::namespace_already_exists(&ns))
            }
            Err(e) => Err(e.into()),
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


impl FileBackend for FilesystemStorage {
    async fn read_file(&self, path: &str) -> Result<Bytes> {
        let full = safe_join(&self.files_root, path)?;
        let path_owned = path.to_owned();
        block(move || read_bytes(&full))
            .await?
            .ok_or_else(|| SaikuroError::key_not_found(path_owned))
    }

    async fn write_file(&self, path: &str, content: Bytes) -> Result<()> {
        let full = safe_join(&self.files_root, path)?;
        let bytes = content.to_vec();
        block(move || write_bytes(&full, &bytes)).await
    }

    async fn append_file(&self, path: &str, content: Bytes) -> Result<()> {
        let full = safe_join(&self.files_root, path)?;
        let bytes = content.to_vec();
        block(move || append_bytes(&full, &bytes)).await
    }

    async fn delete_file(&self, path: &str) -> Result<()> {
        let full = safe_join(&self.files_root, path)?;
        block(move || delete(&full)).await
    }

    async fn file_exists(&self, path: &str) -> Result<bool> {
        let full = safe_join(&self.files_root, path)?;
        block(move || exists(&full)).await
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        let full = safe_join(&self.files_root, path)?;
        block(move || list_files(&full)).await
    }

    async fn create_dir(&self, path: &str) -> Result<()> {
        let full = safe_join(&self.files_root, path)?;
        block(move || {
            std::fs::create_dir_all(&full)?;
            Ok(())
        })
        .await
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let full = safe_join(&self.files_root, path)?;
        block(move || remove_dir(&full)).await
    }
}


impl StorageBackend for FilesystemStorage {
    fn supports_files(&self) -> bool {
        true
    }

    fn as_file_backend(&self) -> Option<&dyn FileBackend> {
        Some(self)
    }
}
