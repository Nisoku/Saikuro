use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::Bytes;
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    FileSystemCreateWritableOptions, FileSystemDirectoryHandle, FileSystemGetDirectoryOptions,
    FileSystemGetFileOptions, FileSystemHandle, FileSystemHandleKind, FileSystemRemoveOptions,
};

use super::{
    config::StorageConfig,
    error::{Result, StorageError},
    traits::{FileBackend, KeyValueBackend, StorageBackend},
};

const ROOT_DIR_NAME: &str = "SaikuroStorage";

struct SendJsFuture(JsFuture);

unsafe impl Send for SendJsFuture {}

impl Future for SendJsFuture {
    type Output = <JsFuture as Future>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().0).poll(cx)
    }
}

fn promise_await(promise: ::js_sys::Promise) -> SendJsFuture {
    SendJsFuture(JsFuture::from(promise))
}

thread_local! {
    static ROOT_HANDLE: RefCell<Option<FileSystemDirectoryHandle>> = const { RefCell::new(None) };
}

async fn open_root() -> Result<FileSystemDirectoryHandle> {
    let window = web_sys::window().ok_or_else(|| StorageError::internal("no window object"))?;
    let storage = window.navigator().storage();
    let promise = storage.get_directory();
    let result = promise_await(promise)
        .await
        .map_err(|e| StorageError::internal(format!("OPFS getDirectory failed: {e:?}")))?;
    let root: FileSystemDirectoryHandle = result.into();

    let opts = FileSystemGetDirectoryOptions::new();
    let promise = root.get_directory_handle_with_options(ROOT_DIR_NAME, &opts);
    let app_dir = promise_await(promise).await.map_err(|e| {
        StorageError::internal(format!(
            "OPFS getDirectoryHandle({ROOT_DIR_NAME}) failed: {e:?}"
        ))
    })?;
    Ok(app_dir.into())
}

async fn get_root() -> Result<FileSystemDirectoryHandle> {
    if let Some(handle) = ROOT_HANDLE.with(|cell| cell.borrow().clone()) {
        return Ok(handle);
    }
    let handle = open_root().await?;
    ROOT_HANDLE.with(|cell| *cell.borrow_mut() = Some(handle.clone()));
    Ok(handle)
}

async fn ensure_dir(
    parent: &FileSystemDirectoryHandle,
    name: &str,
) -> Result<FileSystemDirectoryHandle> {
    let opts = FileSystemGetDirectoryOptions::new();
    opts.set_create(true);
    let promise = parent.get_directory_handle_with_options(name, &opts);
    let result = promise_await(promise)
        .await
        .map_err(|e| StorageError::internal(format!("OPFS ensureDir({name}) failed: {e:?}")))?;
    Ok(result.into())
}

async fn get_or_create_file(
    parent: &FileSystemDirectoryHandle,
    name: &str,
) -> Result<web_sys::FileSystemFileHandle> {
    let opts = FileSystemGetFileOptions::new();
    opts.set_create(true);
    let promise = parent.get_file_handle_with_options(name, &opts);
    let result = promise_await(promise).await.map_err(|e| {
        StorageError::internal(format!("OPFS getOrCreateFile({name}) failed: {e:?}"))
    })?;
    Ok(result.into())
}

async fn get_file(
    parent: &FileSystemDirectoryHandle,
    name: &str,
) -> Result<Option<web_sys::FileSystemFileHandle>> {
    let opts = FileSystemGetFileOptions::new();
    let promise = parent.get_file_handle_with_options(name, &opts);
    match promise_await(promise).await {
        Ok(val) => {
            let handle: web_sys::FileSystemFileHandle = val.into();
            Ok(Some(handle))
        }
        Err(_) => Ok(None),
    }
}

async fn read_file_from_handle(file: &web_sys::FileSystemFileHandle) -> Result<Bytes> {
    let file_promise = file.get_file();
    let file_val = promise_await(file_promise)
        .await
        .map_err(|e| StorageError::internal(format!("OPFS getFile failed: {e:?}")))?;
    let js_file: web_sys::File = file_val.into();

    let buf_promise = js_file.array_buffer();
    let buf_val = promise_await(buf_promise)
        .await
        .map_err(|e| StorageError::internal(format!("OPFS arrayBuffer failed: {e:?}")))?;
    let buf: ArrayBuffer = buf_val.into();
    let uint8 = Uint8Array::new(&buf);
    let mut vec = vec![0u8; uint8.length() as usize];
    uint8.copy_to(&mut vec);
    Ok(Bytes::from(vec))
}

async fn write_file_to_handle(file: &web_sys::FileSystemFileHandle, data: &Bytes) -> Result<()> {
    let writable_promise = file.create_writable();
    let writable_val = promise_await(writable_promise)
        .await
        .map_err(|e| StorageError::internal(format!("OPFS createWritable failed: {e:?}")))?;
    let writable: web_sys::FileSystemWritableFileStream = writable_val.into();

    let write_promise = writable
        .write_with_u8_array(data)
        .map_err(|e| StorageError::internal(format!("OPFS write call failed: {e:?}")))?;
    promise_await(write_promise)
        .await
        .map_err(|e| StorageError::internal(format!("OPFS write failed: {e:?}")))?;

    promise_await(writable.close())
        .await
        .map_err(|e| StorageError::internal(format!("OPFS close failed: {e:?}")))?;

    Ok(())
}

async fn append_file_to_handle(file: &web_sys::FileSystemFileHandle, data: &Bytes) -> Result<()> {
    let create_opts = FileSystemCreateWritableOptions::new();
    create_opts.set_keep_existing_data(true);
    let writable_promise = file.create_writable_with_options(&create_opts);
    let writable_val = promise_await(writable_promise).await.map_err(|e| {
        StorageError::internal(format!("OPFS createWritable(append) failed: {e:?}"))
    })?;
    let writable: web_sys::FileSystemWritableFileStream = writable_val.into();

    let write_promise = writable
        .write_with_u8_array(data)
        .map_err(|e| StorageError::internal(format!("OPFS write call(append) failed: {e:?}")))?;
    promise_await(write_promise)
        .await
        .map_err(|e| StorageError::internal(format!("OPFS write(append) failed: {e:?}")))?;

    promise_await(writable.close())
        .await
        .map_err(|e| StorageError::internal(format!("OPFS close(append) failed: {e:?}")))?;

    Ok(())
}

async fn remove_entry(parent: &FileSystemDirectoryHandle, name: &str) -> Result<()> {
    let promise = parent.remove_entry(name);
    promise_await(promise)
        .await
        .map_err(|e| StorageError::internal(format!("OPFS removeEntry({name}) failed: {e:?}")))?;
    Ok(())
}

async fn remove_entry_recursive(parent: &FileSystemDirectoryHandle, name: &str) -> Result<()> {
    let opts = FileSystemRemoveOptions::new();
    opts.set_recursive(true);
    let promise = parent.remove_entry_with_options(name, &opts);
    promise_await(promise).await.map_err(|e| {
        StorageError::internal(format!("OPFS removeEntry({name},recursive) failed: {e:?}"))
    })?;
    Ok(())
}

async fn list_entry_names(
    dir: &FileSystemDirectoryHandle,
) -> Result<Vec<(String, FileSystemHandleKind)>> {
    let iter = dir.entries();
    let mut entries = Vec::new();
    loop {
        let promise = iter
            .next()
            .map_err(|e| StorageError::internal(format!("OPFS iterator next() failed: {e:?}")))?;
        let result = promise_await(promise)
            .await
            .map_err(|e| StorageError::internal(format!("OPFS iterator promise failed: {e:?}")))?;

        let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if done {
            break;
        }

        let value = js_sys::Reflect::get(&result, &JsValue::from_str("value"))
            .map_err(|_| StorageError::internal("missing value in iterator result"))?;

        let arr = js_sys::Array::from(&value);
        let name = arr.get(0).as_string().unwrap_or_default();
        let handle: FileSystemHandle = arr.get(1).into();
        entries.push((name, handle.kind()));
    }
    Ok(entries)
}

fn navigate_path(path: &str) -> (Vec<&str>, &str) {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return (vec![], "");
    }
    let (dirs, file) = parts.split_at(parts.len() - 1);
    (dirs.to_vec(), file[0])
}

fn apply_prefix(config: &StorageConfig, namespace: &str) -> String {
    match &config.namespace_prefix {
        Some(prefix) => format!("{prefix}_{namespace}"),
        None => namespace.to_owned(),
    }
}

fn strip_prefix(config: &StorageConfig, stored: &str) -> String {
    match &config.namespace_prefix {
        Some(prefix) => {
            let prefix_str = format!("{prefix}_");
            if stored.starts_with(&prefix_str) {
                stored[prefix_str.len()..].to_owned()
            } else {
                stored.to_owned()
            }
        }
        None => stored.to_owned(),
    }
}

pub struct OpfsStorage {
    config: StorageConfig,
}

impl OpfsStorage {
    pub fn new() -> Self {
        Self {
            config: StorageConfig::default(),
        }
    }

    pub fn with_config(config: StorageConfig) -> Self {
        Self { config }
    }

    async fn namespace_dir(&self, namespace: &str) -> Result<FileSystemDirectoryHandle> {
        let root = get_root().await?;
        let prefixed = apply_prefix(&self.config, namespace);
        ensure_dir(&root, &prefixed).await
    }

    async fn namespace_dir_if_exists(
        &self,
        namespace: &str,
    ) -> Result<Option<FileSystemDirectoryHandle>> {
        let root = get_root().await?;
        let prefixed = apply_prefix(&self.config, namespace);
        let opts = FileSystemGetDirectoryOptions::new();
        let promise = root.get_directory_handle_with_options(&prefixed, &opts);
        match promise_await(promise).await {
            Ok(val) => Ok(Some(val.into())),
            Err(_) => Ok(None),
        }
    }

    async fn navigate_to_dir(
        &self,
        dirs: &[&str],
        create: bool,
    ) -> Result<FileSystemDirectoryHandle> {
        let mut current = get_root().await?;
        for &dir_name in dirs {
            if create {
                let opts = FileSystemGetDirectoryOptions::new();
                opts.set_create(true);
                let promise = current.get_directory_handle_with_options(dir_name, &opts);
                let result = promise_await(promise)
                    .await
                    .map_err(|e| StorageError::internal(format!("OPFS navigate failed: {e:?}")))?;
                current = result.into();
            } else {
                let opts = FileSystemGetDirectoryOptions::new();
                let promise = current.get_directory_handle_with_options(dir_name, &opts);
                match promise_await(promise).await {
                    Ok(val) => {
                        current = val.into();
                    }
                    Err(_) => {
                        return Err(StorageError::key_not_found(dirs.join("/")));
                    }
                }
            }
        }
        Ok(current)
    }
}

impl Default for OpfsStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueBackend for OpfsStorage {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let ns_dir = match self.namespace_dir_if_exists(namespace).await? {
            Some(dir) => dir,
            None => return Ok(false),
        };
        let file_handle = get_file(&ns_dir, key).await?;
        Ok(file_handle.is_some())
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let ns_dir = match self.namespace_dir_if_exists(namespace).await? {
            Some(dir) => dir,
            None => return Ok(None),
        };
        let file_handle = match get_file(&ns_dir, key).await? {
            Some(f) => f,
            None => return Ok(None),
        };
        let bytes = read_file_from_handle(&file_handle).await?;
        Ok(Some(bytes))
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let ns_dir = self.namespace_dir(namespace).await?;
        let file_handle = get_or_create_file(&ns_dir, key).await?;
        write_file_to_handle(&file_handle, &value).await
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let ns_dir = match self.namespace_dir_if_exists(namespace).await? {
            Some(dir) => dir,
            None => return Ok(()),
        };
        let _ = remove_entry(&ns_dir, key).await;
        Ok(())
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let ns_dir = match self.namespace_dir_if_exists(namespace).await? {
            Some(dir) => dir,
            None => return Ok(vec![]),
        };
        let entries = list_entry_names(&ns_dir).await?;
        let keys: Vec<String> = entries
            .into_iter()
            .filter(|(_, kind)| *kind == FileSystemHandleKind::File)
            .map(|(name, _)| name)
            .collect();
        Ok(keys)
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let root = get_root().await?;
        let entries = list_entry_names(&root).await?;
        let namespaces: Vec<String> = entries
            .into_iter()
            .filter(|(_, kind)| *kind == FileSystemHandleKind::Directory)
            .map(|(name, _)| strip_prefix(&self.config, &name))
            .collect();
        Ok(namespaces)
    }

    async fn create_namespace(&self, namespace: &str) -> Result<()> {
        self.namespace_dir(namespace).await?;
        Ok(())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let root = get_root().await?;
        let prefixed = apply_prefix(&self.config, namespace);
        let _ = remove_entry_recursive(&root, &prefixed).await;
        Ok(())
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        self.delete_namespace(namespace).await
    }
}

#[async_trait]
impl FileBackend for OpfsStorage {
    async fn read_file(&self, path: &str) -> Result<Bytes> {
        let (dirs, file_name) = navigate_path(path);
        let parent = self.navigate_to_dir(&dirs, false).await?;
        let file_handle = get_file(&parent, file_name)
            .await?
            .ok_or_else(|| StorageError::key_not_found(path))?;
        read_file_from_handle(&file_handle).await
    }

    async fn write_file(&self, path: &str, content: Bytes) -> Result<()> {
        let (dirs, file_name) = navigate_path(path);
        let parent = self.navigate_to_dir(&dirs, true).await?;
        let file_handle = get_or_create_file(&parent, file_name).await?;
        write_file_to_handle(&file_handle, &content).await
    }

    async fn append_file(&self, path: &str, content: Bytes) -> Result<()> {
        let (dirs, file_name) = navigate_path(path);
        let parent = self.navigate_to_dir(&dirs, true).await?;
        let file_handle = get_or_create_file(&parent, file_name).await?;
        append_file_to_handle(&file_handle, &content).await
    }

    async fn delete_file(&self, path: &str) -> Result<()> {
        let (dirs, file_name) = navigate_path(path);
        let parent = match self.navigate_to_dir(&dirs, false).await {
            Ok(dir) => dir,
            Err(_) => return Ok(()),
        };
        let _ = remove_entry(&parent, file_name).await;
        Ok(())
    }

    async fn file_exists(&self, path: &str) -> Result<bool> {
        let (dirs, file_name) = navigate_path(path);
        let parent = match self.navigate_to_dir(&dirs, false).await {
            Ok(dir) => dir,
            Err(_) => return Ok(false),
        };
        let handle = get_file(&parent, file_name).await?;
        Ok(handle.is_some())
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<String>> {
        let (dirs, _) = navigate_path(path);
        let dir = if dirs.is_empty() {
            get_root().await?
        } else {
            self.navigate_to_dir(&dirs, false).await?
        };
        let entries = list_entry_names(&dir).await?;
        let names: Vec<String> = entries.into_iter().map(|(name, _)| name).collect();
        Ok(names)
    }

    async fn create_dir(&self, path: &str) -> Result<()> {
        let (dirs, dir_name) = navigate_path(path);
        if dir_name.is_empty() {
            return Err(StorageError::internal("cannot create root directory"));
        }
        let parent = self.navigate_to_dir(&dirs, true).await?;
        let opts = FileSystemGetDirectoryOptions::new();
        opts.set_create(true);
        let promise = parent.get_directory_handle_with_options(dir_name, &opts);
        promise_await(promise)
            .await
            .map_err(|e| StorageError::internal(format!("OPFS createDir({path}) failed: {e:?}")))?;
        Ok(())
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let (dirs, dir_name) = navigate_path(path);
        if dir_name.is_empty() {
            return Err(StorageError::internal("cannot delete root directory"));
        }
        let parent = match self.navigate_to_dir(&dirs, false).await {
            Ok(dir) => dir,
            Err(_) => return Ok(()),
        };
        remove_entry_recursive(&parent, dir_name).await
    }
}

#[async_trait]
impl StorageBackend for OpfsStorage {
    fn supports_files(&self) -> bool {
        true
    }

    fn as_file_backend(&self) -> Option<&dyn FileBackend> {
        Some(self)
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        ROOT_HANDLE.with(|cell| *cell.borrow_mut() = None);
        Ok(())
    }
}
