// Re-export pure helpers from the unconditionally-compiled util module
// so that the impl_web_storage! macro (which uses $crate::webstorage::*)
// continues to work.
#[allow(unused_imports)]
pub(crate) use crate::util::{
    apply_prefix, decode_bytes, encode_bytes, key_prefix, make_key, strip_prefix,
    NAMESPACE_SEPARATOR,
};

use bytes::Bytes;

use crate::util;

use super::error::{Result, StorageError};

pub(crate) fn window() -> Result<web_sys::Window> {
    web_sys::window().ok_or_else(|| StorageError::internal("no window object"))
}

pub(crate) fn get_all_keys(storage: &web_sys::Storage) -> Vec<String> {
    let len = storage.length().unwrap_or(0);
    let mut keys = Vec::with_capacity(len as usize);
    for i in 0..len {
        if let Some(Ok(key)) = storage.key(i).transpose() {
            keys.push(key);
        }
    }
    keys
}

pub(crate) fn get_keys_in_namespace(storage: &web_sys::Storage, namespace: &str) -> Vec<String> {
    let prefix = util::key_prefix(namespace);
    let all = get_all_keys(storage);
    all.into_iter()
        .filter(|k| k.starts_with(&prefix))
        .map(|k| k[prefix.len()..].to_owned())
        .collect()
}

pub(crate) fn get_namespaces(storage: &web_sys::Storage) -> Vec<String> {
    let all = get_all_keys(storage);
    let mut namespaces: Vec<String> = all
        .iter()
        .filter_map(|k| {
            k.split_once(util::NAMESPACE_SEPARATOR)
                .map(|(ns, _)| ns.to_owned())
        })
        .collect();
    namespaces.sort();
    namespaces.dedup();
    namespaces
}

pub(crate) fn delete_keys_with_prefix(storage: &web_sys::Storage, prefix: &str) {
    let keys: Vec<String> = get_all_keys(storage)
        .into_iter()
        .filter(|k| k.starts_with(prefix))
        .collect();
    for key in keys {
        let _ = storage.delete(&key);
    }
}

pub(crate) fn storage_get(storage: &web_sys::Storage, key: &str) -> Result<Option<Bytes>> {
    match storage.get_item(key) {
        Ok(Some(val)) => Ok(Some(util::decode_bytes(&val))),
        Ok(None) => Ok(None),
        Err(e) => Err(StorageError::internal(format!(
            "web storage get_item failed: {e:?}"
        ))),
    }
}

pub(crate) fn storage_set(storage: &web_sys::Storage, key: &str, value: &Bytes) -> Result<()> {
    let encoded = util::encode_bytes(value);
    storage
        .set_item(key, &encoded)
        .map_err(|e| StorageError::internal(format!("web storage set_item failed: {e:?}")))
}

pub(crate) fn storage_remove(storage: &web_sys::Storage, key: &str) {
    let _ = storage.remove_item(key);
}
