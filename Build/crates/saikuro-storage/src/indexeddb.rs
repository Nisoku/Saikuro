//! IndexedDB-backed storage backend for WASM.
//!
//! Uses the browser's IndexedDB API to provide persistent key-value storage
//! that survives page reloads. Enabled automatically when the `wasm-storage`
//! feature is active on a `wasm32` target.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::Bytes;
use js_sys::Uint8Array;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    IdbDatabase, IdbFactory, IdbKeyRange, IdbObjectStore, IdbRequest, IdbTransactionMode,
    IdbVersionChangeEvent,
};

use super::{
    config::StorageConfig,
    error::{Result, StorageError},
    traits::{KeyValueBackend, StorageBackend},
};

const DB_NAME: &str = "SaikuroStorage";
const STORE_NAME: &str = "kv_store";
const DB_VERSION: u32 = 1;

thread_local! {
    static DB_HANDLE: RefCell<Option<IdbDatabase>> = const { RefCell::new(None) };
}

// Send-safe JsFuture wrapper
/// A `JsFuture` wrapper that implements `Send`.
///
/// SAFETY: On single-threaded `wasm32-unknown-unknown` no `JsValue` ever
/// crosses a thread boundary, so the `Send` requirement of the storage trait
/// is satisfied soundly.
struct SendJsFuture(JsFuture);

unsafe impl Send for SendJsFuture {}

impl Future for SendJsFuture {
    type Output = <JsFuture as Future>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().0).poll(cx)
    }
}

// Helpers
fn make_key(namespace: &str, key: &str) -> String {
    format!("{namespace}:{key}")
}

fn key_prefix(namespace: &str) -> String {
    format!("{namespace}:")
}

/// Convert an `IdbRequest` into a Rust `Future` by wrapping its `onsuccess`
/// and `onerror` in a JavaScript Promise.
fn idb_await(request: &IdbRequest) -> SendJsFuture {
    let req = request.clone();
    let promise =
        js_sys::Promise::new(&mut |resolve: js_sys::Function, reject: js_sys::Function| {
            let r = req.clone();
            let ok = Closure::once(move || {
                let result = r.result().unwrap_or(JsValue::undefined());
                resolve.call1(&JsValue::undefined(), &result).ok();
            });
            req.set_onsuccess(Some(ok.as_ref().unchecked_ref()));
            ok.forget();

            let req_err = req.clone();
            let err = Closure::once(move || {
                let error: JsValue = req_err
                    .error()
                    .ok()
                    .flatten()
                    .map(|e| e.into())
                    .unwrap_or(JsValue::null());
                reject.call1(&JsValue::undefined(), &error).ok();
            });
            req.set_onerror(Some(err.as_ref().unchecked_ref()));
            err.forget();
        });
    SendJsFuture(JsFuture::from(promise))
}

/// Convert a JsValue containing an `ArrayBuffer` to `Bytes`.
fn js_to_bytes(val: JsValue) -> Bytes {
    let buf = Uint8Array::new(&val);
    let mut vec = vec![0u8; buf.length() as usize];
    buf.copy_to(&mut vec);
    Bytes::from(vec)
}

/// Convert `Bytes` to a JS `ArrayBuffer`.
fn bytes_to_js(val: &Bytes) -> JsValue {
    let buf = Uint8Array::new_with_length(val.len() as u32);
    buf.copy_from(val);
    buf.buffer().into()
}

async fn open_database(name: &str, version: u32) -> Result<IdbDatabase> {
    let window = web_sys::window().ok_or_else(|| StorageError::internal("no window object"))?;
    let factory: IdbFactory = window
        .indexed_db()
        .map_err(|_| StorageError::internal("IndexedDB API call failed"))?
        .ok_or_else(|| StorageError::internal("IndexedDB not available"))?;

    let open_request = factory
        .open_with_u32(name, version)
        .map_err(|e| StorageError::internal(format!("IndexedDB open call failed: {e:?}")))?;

    // Install onupgradeneeded handler (fires when DB is created or version
    // changes). The handler receives an `IdbVersionChangeEvent` whose
    // `target.result` is the `IdbDatabase` at the point where schema changes
    // are allowed.
    {
        let upgrade = Closure::<dyn FnMut(IdbVersionChangeEvent)>::new(
            move |event: IdbVersionChangeEvent| {
                if let Some(target) = event.target() {
                    if let Some(req) = target.dyn_ref::<IdbRequest>() {
                        if let Ok(result) = req.result() {
                            let db: &IdbDatabase = result.unchecked_ref();
                            if !db.object_store_names().contains(STORE_NAME) {
                                let _ = db.create_object_store(STORE_NAME);
                            }
                        }
                    }
                }
            },
        );
        open_request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
        upgrade.forget();
    }

    let result = idb_await(open_request.unchecked_ref::<IdbRequest>())
        .await
        .map_err(|e| StorageError::internal(format!("IndexedDB open failed: {e:?}")))?;

    Ok(result.into())
}

async fn get_db() -> Result<IdbDatabase> {
    if let Some(db) = DB_HANDLE.with(|cell| cell.borrow().clone()) {
        return Ok(db);
    }
    let db = open_database(DB_NAME, DB_VERSION).await?;
    DB_HANDLE.with(|cell| *cell.borrow_mut() = Some(db.clone()));
    Ok(db)
}

async fn tx(
    db: &IdbDatabase,
    mode: IdbTransactionMode,
) -> Result<(web_sys::IdbTransaction, IdbObjectStore)> {
    let transaction = db
        .transaction_with_str_and_mode(STORE_NAME, mode)
        .map_err(|_| StorageError::internal("failed to create IndexedDB transaction"))?;
    let store = transaction
        .object_store(STORE_NAME)
        .map_err(|_| StorageError::internal("failed to get object store"))?;
    Ok((transaction, store))
}

fn store_get(store: &IdbObjectStore, key: &JsValue) -> Result<SendJsFuture> {
    let request = store
        .get(key)
        .map_err(|_| StorageError::internal("IndexedDB get request failed"))?;
    Ok(idb_await(&request))
}

fn store_put(store: &IdbObjectStore, key: &JsValue, value: &JsValue) -> Result<SendJsFuture> {
    let request = store
        .put_with_key(value, key)
        .map_err(|_| StorageError::internal("IndexedDB put request failed"))?;
    Ok(idb_await(&request))
}

fn store_delete(store: &IdbObjectStore, key: &JsValue) -> Result<SendJsFuture> {
    let request = store
        .delete(key)
        .map_err(|_| StorageError::internal("IndexedDB delete request failed"))?;
    Ok(idb_await(&request))
}

fn store_get_all_keys(store: &IdbObjectStore, query: Option<&JsValue>) -> Result<SendJsFuture> {
    let request = match query {
        Some(q) => store.get_all_keys_with_key(q),
        None => store.get_all_keys(),
    }
    .map_err(|_| StorageError::internal("IndexedDB getAllKeys request failed"))?;
    Ok(idb_await(&request))
}

/// Build an `IDBKeyRange` covering all keys beginning with `prefix`.
fn prefix_range(prefix: &str) -> Result<IdbKeyRange> {
    let upper = {
        let mut s = prefix.to_owned();
        s.push('\u{10FFFF}');
        s
    };
    IdbKeyRange::bound(&JsValue::from(prefix), &JsValue::from(&upper))
        .map_err(|_| StorageError::internal("failed to create IDBKeyRange"))
}

// IndexedDbStorage
/// Persistent key-value storage backed by the browser's IndexedDB.
///
/// All data survives page reloads and is scoped to the browser origin.
/// Works on both desktop and mobile browsers that support IndexedDB.
///
/// ## Namespace model
///
/// Keys are stored as compound strings `"{namespace}:{key}"` within a single
/// IndexedDB object store. This keeps the database schema simple while
/// supporting all `KeyValueBackend` operations.
pub struct IndexedDbStorage {
    config: StorageConfig,
}

impl IndexedDbStorage {
    /// Create an `IndexedDbStorage` with default settings.
    ///
    /// The database is opened lazily on the first operation.
    pub fn new() -> Self {
        Self {
            config: StorageConfig::default(),
        }
    }

    /// Create an `IndexedDbStorage` with the given [`StorageConfig`].
    pub fn with_config(config: StorageConfig) -> Self {
        Self { config }
    }
}

impl Default for IndexedDbStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueBackend for IndexedDbStorage {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let db = get_db().await?;
        let (_tx, store) = tx(&db, IdbTransactionMode::Readonly).await?;
        match store_get(&store, &JsValue::from(make_key(namespace, key)))?.await {
            Ok(val) => Ok(!val.is_undefined() && !val.is_null()),
            Err(e) => Err(StorageError::internal(format!(
                "IndexedDB exists failed: {e:?}"
            ))),
        }
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let db = get_db().await?;
        let (_tx, store) = tx(&db, IdbTransactionMode::Readonly).await?;
        match store_get(&store, &JsValue::from(make_key(namespace, key)))?.await {
            Ok(val) if !val.is_undefined() && !val.is_null() => Ok(Some(js_to_bytes(val))),
            _ => Ok(None),
        }
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let db = get_db().await?;
        let (_tx, store) = tx(&db, IdbTransactionMode::Readwrite).await?;
        store_put(
            &store,
            &JsValue::from(make_key(namespace, key)),
            &bytes_to_js(&value),
        )?
        .await
        .map_err(|e| StorageError::internal(format!("IndexedDB put failed: {e:?}")))?;
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let db = get_db().await?;
        let (_tx, store) = tx(&db, IdbTransactionMode::Readwrite).await?;
        store_delete(&store, &JsValue::from(make_key(namespace, key)))?
            .await
            .map_err(|e| StorageError::internal(format!("IndexedDB delete failed: {e:?}")))?;
        Ok(())
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let db = get_db().await?;
        let (_tx, store) = tx(&db, IdbTransactionMode::Readonly).await?;
        let prefix = key_prefix(namespace);
        let range = prefix_range(&prefix)?;
        let result = store_get_all_keys(&store, Some(&JsValue::from(range)))?
            .await
            .map_err(|e| StorageError::internal(format!("IndexedDB list_keys failed: {e:?}")))?;

        let prefix_len = prefix.len();
        let keys: Vec<String> = result
            .dyn_into::<js_sys::Array>()
            .unwrap_or_default()
            .to_vec()
            .into_iter()
            .filter_map(|v| v.as_string())
            .map(|k| k[prefix_len..].to_owned())
            .collect();
        Ok(keys)
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let db = get_db().await?;
        let (_tx, store) = tx(&db, IdbTransactionMode::Readonly).await?;
        let result = store_get_all_keys(&store, None)?.await.map_err(|e| {
            StorageError::internal(format!("IndexedDB list_namespaces failed: {e:?}"))
        })?;

        let mut namespaces: Vec<String> = result
            .dyn_into::<js_sys::Array>()
            .unwrap_or_default()
            .to_vec()
            .into_iter()
            .filter_map(|v| v.as_string())
            .filter_map(|k| k.split_once(':').map(|(ns, _)| ns.to_owned()))
            .collect();
        namespaces.sort();
        namespaces.dedup();
        Ok(namespaces)
    }

    async fn create_namespace(&self, _namespace: &str) -> Result<()> {
        Ok(())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let db = get_db().await?;
        let (_tx, store) = tx(&db, IdbTransactionMode::Readwrite).await?;
        let range = prefix_range(&make_key(namespace, ""))?;
        let request = store
            .delete(&JsValue::from(range))
            .map_err(|_| StorageError::internal("IndexedDB range delete failed"))?;
        idb_await(&request).await.map_err(|e| {
            StorageError::internal(format!("IndexedDB delete_namespace failed: {e:?}"))
        })?;
        Ok(())
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        self.delete_namespace(namespace).await
    }
}

#[async_trait]
impl StorageBackend for IndexedDbStorage {
    fn supports_files(&self) -> bool {
        false
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        DB_HANDLE.with(|cell| *cell.borrow_mut() = None);
        Ok(())
    }
}
