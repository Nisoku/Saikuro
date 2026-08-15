use alloc::string::{String, ToString};
use alloc::vec::Vec;

use bytes::Bytes;

use wasi::filesystem::{
    self, Descriptor, DescriptorFlags, DirectoryEntry, Error as FsError, OpenFlags, PathFlags,
};
use wasi::io::streams::{InputStream, OutputStream, StreamError};

use crate::shared::config::StorageConfig;
use crate::shared::traits::{FileBackend, KeyValueBackend, StorageBackend};
use saikuro_event::{Result, SaikuroError};

mod bindings {
    wit_bindgen::generate!({
        world: "saikuro-kv",
        path: "wasi/wit",
    });
}

use bindings::wasi::keyvalue::store as kv;

/// Bucket identifier prefix, keeping Saikuro namespaces isolated from other
/// key-value tenants on the same host.
const BUCKET_PREFIX: &str = "saikuro";

fn bucket_for(namespace: &str) -> Result<kv::Bucket> {
    let id = format!("{BUCKET_PREFIX}-{namespace}");
    kv::open(&id).map_err(map_kv_err)
}

fn map_kv_err(e: kv::Error) -> SaikuroError {
    SaikuroError::backend_unavailable(format!("wasi:keyvalue: {e:?}"))
}

fn map_fs_err(e: FsError) -> SaikuroError {
    SaikuroError::backend_unavailable(format!("wasi:filesystem: {e:?}"))
}

fn map_stream_err(e: StreamError) -> SaikuroError {
    SaikuroError::io(format!("wasi:streams: {e:?}"))
}

/// Key-value storage backed by the host `wasi:keyvalue` store.
pub struct WasiKvStore {
    config: StorageConfig,
}

impl WasiKvStore {
    /// Create a key-value store using the default configuration.
    pub fn new() -> Self {
        Self {
            config: StorageConfig::default(),
        }
    }

    /// Create a key-value store with an explicit configuration.
    pub fn with_config(config: StorageConfig) -> Self {
        Self { config }
    }

    fn ns(&self, namespace: &str) -> String {
        match &self.config.namespace_prefix {
            Some(prefix) => format!("{prefix}:{namespace}"),
            None => namespace.to_string(),
        }
    }
}

impl Default for WasiKvStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyValueBackend for WasiKvStore {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let bucket = bucket_for(&self.ns(namespace))?;
        bucket.exists(key).map_err(map_kv_err)
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let bucket = bucket_for(&self.ns(namespace))?;
        match bucket.get(key).map_err(map_kv_err)? {
            Some(bytes) => Ok(Some(Bytes::from(bytes))),
            None => Ok(None),
        }
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let bucket = bucket_for(&self.ns(namespace))?;
        bucket.set(key, &value.to_vec()).map_err(map_kv_err)
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let bucket = bucket_for(&self.ns(namespace))?;
        bucket.delete(key).map_err(map_kv_err)
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let bucket = bucket_for(&self.ns(namespace))?;
        let mut out = Vec::new();
        let mut cursor = None;
        loop {
            let resp = bucket.list_keys(cursor.clone()).map_err(map_kv_err)?;
            out.extend(resp.keys);
            match resp.cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(out)
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        // `wasi:keyvalue` exposes no bucket enumeration, so namespaces are not
        // enumerable through this backend. Callers that need the set of
        // namespaces must track them client-side.
        Ok(Vec::new())
    }

    async fn create_namespace(&self, _namespace: &str) -> Result<()> {
        Ok(())
    }

    async fn delete_namespace(&self, _namespace: &str) -> Result<()> {
        Ok(())
    }

    async fn clear_namespace(&self, _namespace: &str) -> Result<()> {
        Ok(())
    }
}

impl StorageBackend for WasiKvStore {
    fn supports_files(&self) -> bool {
        false
    }
}

/// File storage backed by the host `wasi:filesystem` interface.
pub struct WasiFileStore {
    config: StorageConfig,
}

impl WasiFileStore {
    /// Create a file store using the default configuration.
    pub fn new() -> Self {
        Self {
            config: StorageConfig::default(),
        }
    }

    /// Create a file store with an explicit configuration.
    pub fn with_config(config: StorageConfig) -> Self {
        Self { config }
    }

    /// The first preopened directory is the store root.
    fn root(&self) -> Result<Descriptor> {
        let (descriptors, _) = filesystem::preopens().map_err(map_fs_err)?;
        descriptors.into_iter().next().ok_or_else(|| {
            SaikuroError::backend_unavailable("wasi:filesystem has no preopened directory")
        })
    }
}

impl Default for WasiFileStore {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBackend for WasiFileStore {
    async fn read_file(&self, path: &str) -> Result<Bytes> {
        let root = self.root()?;
        let desc = filesystem::open_at(
            &root,
            PathFlags::default(),
            path,
            OpenFlags::empty(),
            DescriptorFlags::READ,
        )
        .map_err(map_fs_err)?;
        let stream: InputStream = desc.read_via_stream(0).map_err(map_fs_err)?;
        let mut out = Vec::new();
        loop {
            match stream.read(4096).map_err(map_stream_err)? {
                chunk if chunk.is_empty() => break,
                chunk => out.extend_from_slice(&chunk),
            }
        }
        Ok(Bytes::from(out))
    }

    async fn write_file(&self, path: &str, content: Bytes) -> Result<()> {
        let root = self.root()?;
        let desc = filesystem::open_at(
            &root,
            PathFlags::default(),
            path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE,
            DescriptorFlags::READ | DescriptorFlags::WRITE,
        )
        .map_err(map_fs_err)?;
        let stream: OutputStream = desc.write_via_stream(0).map_err(map_fs_err)?;
        stream.write(&content).map_err(map_stream_err)?;
        stream.flush().map_err(map_stream_err)
    }

    async fn append_file(&self, path: &str, content: Bytes) -> Result<()> {
        let root = self.root()?;
        let desc = filesystem::open_at(
            &root,
            PathFlags::default(),
            path,
            OpenFlags::CREATE,
            DescriptorFlags::READ | DescriptorFlags::WRITE,
        )
        .map_err(map_fs_err)?;
        let stream: OutputStream = desc.append_via_stream().map_err(map_fs_err)?;
        stream.write(&content).map_err(map_stream_err)?;
        stream.flush().map_err(map_stream_err)
    }

    async fn delete_file(&self, path: &str) -> Result<()> {
        let root = self.root()?;
        root.unlink_file_at(path).map_err(map_fs_err)
    }

    async fn file_exists(&self, path: &str) -> Result<bool> {
        let root = self.root()?;
        match filesystem::open_at(
            &root,
            PathFlags::default(),
            path,
            OpenFlags::empty(),
            DescriptorFlags::READ,
        ) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        let root = self.root()?;
        let dir = filesystem::open_at(
            &root,
            PathFlags::default(),
            path,
            OpenFlags::DIRECTORY,
            DescriptorFlags::READ,
        )
        .map_err(map_fs_err)?;
        let mut out = Vec::new();
        let mut cookie = 0u64;
        loop {
            let entries: Vec<DirectoryEntry> = dir.readdir(0, cookie, 4096).map_err(map_fs_err)?;
            if entries.is_empty() {
                break;
            }
            for entry in entries {
                if entry.name == "." || entry.name == ".." {
                    continue;
                }
                out.push(entry.name);
            }
            cookie += entries.len() as u64;
        }
        Ok(out)
    }

    async fn create_dir(&self, path: &str) -> Result<()> {
        let root = self.root()?;
        root.create_directory_at(path).map_err(map_fs_err)
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let root = self.root()?;
        root.remove_directory_at(path).map_err(map_fs_err)
    }
}

impl StorageBackend for WasiFileStore {
    fn supports_files(&self) -> bool {
        true
    }

    fn as_file_backend(&self) -> Option<&dyn FileBackend> {
        Some(self)
    }
}
