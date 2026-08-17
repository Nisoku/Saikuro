use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use async_trait::async_trait;
use bytes::Bytes;

use crate::shared::config::StorageConfig;
use crate::shared::traits::{FileBackend, KeyValueBackend, StorageBackend};
use saikuro_event::{Result, SaikuroError};

/// First preopened directory, by WASI preview1 convention.
const PREOPEN_FD: i32 = 3;

const OFLAG_CREAT: i32 = 1 << 0;
const OFLAG_DIR: i32 = 1 << 1;
const OFLAG_TRUNC: i32 = 1 << 3;
const WHENCE_SET: i32 = 0;

const KV_ROOT: &str = "saikuro_kv";

#[link(wasm_import_module = "wasi_snapshot_preview1")]
extern "C" {
    fn path_open(
        dirfd: i32,
        dirflags: i32,
        path: *const u8,
        path_len: i32,
        oflags: i32,
        fs_rights_base: i64,
        fs_rights_inheriting: i64,
        fdflags: i32,
        opened_fd: *mut i32,
    ) -> i32;
    fn fd_close(fd: i32) -> i32;
    fn fd_read(fd: i32, iovs: *const Iovec, iovs_len: i32, nread: *mut i32) -> i32;
    fn fd_write(fd: i32, iovs: *const Iovec, iovs_len: i32, nwritten: *mut i32) -> i32;
    fn fd_seek(fd: i32, offset: i64, whence: i32, newoffset: *mut i64) -> i32;
    fn path_unlink_file(dirfd: i32, path: *const u8, path_len: i32) -> i32;
    fn path_create_directory(dirfd: i32, path: *const u8, path_len: i32) -> i32;
    fn path_remove_directory(dirfd: i32, path: *const u8, path_len: i32) -> i32;
    fn fd_readdir(fd: i32, buf: *mut u8, buf_len: usize, cookie: i64, bufused: *mut usize) -> i32;
}

#[repr(C)]
struct Iovec {
    buf: *mut u8,
    buf_len: usize,
}

fn wasi_err(code: i32) -> SaikuroError {
    SaikuroError::io(format!("wasi_snapshot_preview1 error code {code}"))
}

fn open_file(path: &str, create: bool, directory: bool) -> Result<i32> {
    let bytes = path.as_bytes();
    let mut fd = 0i32;
    let mut oflags = 0i32;
    if create {
        oflags |= OFLAG_CREAT;
    }
    if directory {
        oflags |= OFLAG_DIR;
    } else if create {
        oflags |= OFLAG_TRUNC;
    }
    // SAFETY: `bytes` and `fd` outlive the call; the host writes `fd` on success.
    let rc = unsafe {
        path_open(
            PREOPEN_FD,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            oflags,
            u64::MAX,
            u64::MAX,
            0,
            &mut fd,
        )
    };
    if rc != 0 {
        return Err(wasi_err(rc));
    }
    Ok(fd)
}

fn read_all(fd: i32) -> Result<Vec<u8>> {
    let mut newoff = 0i64;
    // SAFETY: `newoff` outlives the call.
    unsafe {
        fd_seek(fd, 0, WHENCE_SET, &mut newoff);
    }
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let iov = Iovec {
            buf: buf.as_mut_ptr(),
            buf_len: buf.len(),
        };
        let mut nread = 0i32;
        // SAFETY: `buf` and `nread` outlive the call.
        let rc = unsafe { fd_read(fd, &iov as *const Iovec, 1, &mut nread) };
        if rc != 0 {
            return Err(wasi_err(rc));
        }
        if nread == 0 {
            break;
        }
        out.extend_from_slice(&buf[..nread as usize]);
    }
    Ok(out)
}

fn write_all(fd: i32, data: &[u8]) -> Result<()> {
    let mut written = 0usize;
    while written < data.len() {
        let iov = Iovec {
            buf: data[written..].as_ptr() as *mut u8,
            buf_len: data.len() - written,
        };
        let mut nwritten = 0i32;
        // SAFETY: `data` outlives the call.
        let rc = unsafe { fd_write(fd, &iov as *const Iovec, 1, &mut nwritten) };
        if rc != 0 {
            return Err(wasi_err(rc));
        }
        if nwritten == 0 {
            return Err(SaikuroError::io("wasi write made no progress"));
        }
        written += nwritten as usize;
    }
    Ok(())
}

fn close(fd: i32) {
    // SAFETY: `fd` is a valid descriptor previously returned by `path_open`.
    unsafe {
        fd_close(fd);
    }
}

fn read_dir_names(fd: i32) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut cookie: i64 = 0;
    let mut buf = vec![0u8; 8192];
    loop {
        let mut bufused: usize = 0;
        // SAFETY: `buf` and `bufused` outlive the call.
        let rc = unsafe { fd_readdir(fd, buf.as_mut_ptr(), buf.len(), cookie, &mut bufused) };
        if rc != 0 {
            return Err(wasi_err(rc));
        }
        if bufused == 0 {
            break;
        }
        let mut off = 0usize;
        let mut last_next: u64 = 0;
        while off + 24 <= bufused {
            let d_next = u64::from_le_bytes(buf[off..off + 8].try_into().unwrap());
            let namlen = u32::from_le_bytes(buf[off + 16..off + 20].try_into().unwrap()) as usize;
            let name_start = off + 24;
            let name_end = name_start + namlen;
            if name_end > bufused {
                break;
            }
            let name = core::str::from_utf8(&buf[name_start..name_end]).unwrap_or("");
            if name != "." && name != ".." {
                out.push(String::from(name));
            }
            last_next = d_next;
            off = (name_end + 7) & !7;
        }
        if off == 0 || last_next == 0 {
            break;
        }
        cookie = last_next as i64;
        if bufused < buf.len() {
            break;
        }
    }
    Ok(out)
}

fn unlink(path: &str) -> Result<()> {
    let bytes = path.as_bytes();
    // SAFETY: `bytes` outlive the call.
    let rc = unsafe { path_unlink_file(PREOPEN_FD, bytes.as_ptr(), bytes.len() as i32) };
    if rc != 0 {
        return Err(wasi_err(rc));
    }
    Ok(())
}

fn mkdir(path: &str) -> Result<()> {
    let bytes = path.as_bytes();
    // SAFETY: `bytes` outlive the call.
    let rc = unsafe { path_create_directory(PREOPEN_FD, bytes.as_ptr(), bytes.len() as i32) };
    if rc != 0 {
        return Err(wasi_err(rc));
    }
    Ok(())
}

fn rmdir(path: &str) -> Result<()> {
    let bytes = path.as_bytes();
    // SAFETY: `bytes` outlive the call.
    let rc = unsafe { path_remove_directory(PREOPEN_FD, bytes.as_ptr(), bytes.len() as i32) };
    if rc != 0 {
        return Err(wasi_err(rc));
    }
    Ok(())
}

fn ns_dir(config: &StorageConfig, namespace: &str) -> String {
    match &config.namespace_prefix {
        Some(prefix) => format!("{KV_ROOT}/{prefix}:{namespace}"),
        None => format!("{KV_ROOT}/{namespace}"),
    }
}

fn kv_path(config: &StorageConfig, namespace: &str, key: &str) -> String {
    format!("{}/{}", ns_dir(config, namespace), key)
}

/// Key-value storage backed by files under the preopened directory.
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
        match open_file(&kv_path(self.config(), namespace, key), false, false) {
            Ok(fd) => {
                close(fd);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let fd = match open_file(&kv_path(self.config(), namespace, key), false, false) {
            Ok(fd) => fd,
            Err(_) => return Ok(None),
        };
        let data = read_all(fd)?;
        close(fd);
        Ok(Some(Bytes::from(data)))
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let dir = ns_dir(self.config(), namespace);
        // Best-effort: ensure the namespace directory exists.
        let _ = mkdir(&dir);
        let fd = open_file(&kv_path(self.config(), namespace, key), true, false)?;
        let res = write_all(fd, &value);
        close(fd);
        res
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        unlink(&kv_path(self.config(), namespace, key))
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let dir = ns_dir(self.config(), namespace);
        let fd = match open_file(&dir, false, true) {
            Ok(fd) => fd,
            Err(_) => return Ok(Vec::new()),
        };
        let names = read_dir_names(fd);
        close(fd);
        names
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let fd = match open_file(KV_ROOT, false, true) {
            Ok(fd) => fd,
            Err(_) => return Ok(Vec::new()),
        };
        let names = read_dir_names(fd);
        close(fd);
        let prefix = self.config().namespace_prefix.clone();
        Ok(names
            .map(|names| {
                names
                    .into_iter()
                    .filter(|n| match &prefix {
                        Some(p) => n.starts_with(&format!("{p}:")),
                        None => true,
                    })
                    .map(|n| match &prefix {
                        Some(p) => n[(format!("{p}:").len())..].to_string(),
                        None => n,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn create_namespace(&self, namespace: &str) -> Result<()> {
        mkdir(&ns_dir(self.config(), namespace))
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        rmdir(&ns_dir(self.config(), namespace))
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        self.delete_namespace(namespace).await
    }
}

impl StorageBackend for WasiKvStore {
    fn supports_files(&self) -> bool {
        false
    }
}

/// File storage backed by files under the preopened directory.
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
}

impl Default for WasiFileStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl FileBackend for WasiFileStore {
    async fn read_file(&self, path: &str) -> Result<Bytes> {
        let fd = open_file(path, false, false)?;
        let data = read_all(fd);
        close(fd);
        data.map(Bytes::from)
    }

    async fn write_file(&self, path: &str, content: Bytes) -> Result<()> {
        let fd = open_file(path, true, false)?;
        let res = write_all(fd, &content);
        close(fd);
        res
    }

    async fn append_file(&self, path: &str, content: Bytes) -> Result<()> {
        let fd = open_file(path, true, false)?;
        let existing = read_all(fd);
        close(fd);
        let existing = existing?;
        let mut merged = existing;
        merged.extend_from_slice(&content);
        let fd = open_file(path, true, false)?;
        let res = write_all(fd, &merged);
        close(fd);
        res
    }

    async fn delete_file(&self, path: &str) -> Result<()> {
        unlink(path)
    }

    async fn file_exists(&self, path: &str) -> Result<bool> {
        match open_file(path, false, false) {
            Ok(fd) => {
                close(fd);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        let fd = match open_file(path, false, true) {
            Ok(fd) => fd,
            Err(_) => return Ok(Vec::new()),
        };
        let names = read_dir_names(fd);
        close(fd);
        names
    }

    async fn create_dir(&self, path: &str) -> Result<()> {
        mkdir(path)
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        rmdir(path)
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
