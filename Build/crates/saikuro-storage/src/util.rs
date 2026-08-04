// Pure helper functions shared by webstorage, opfs, and indexeddb backends.
// These are re-exported from the wasm32-gated webstorage module so the
// impl_web_storage! macro can reach them via $crate::webstorage::*.
//
// The functions are `pub` (doc-hidden) rather than `pub(crate)` so the
// crate's integration tests can exercise them; on native they are otherwise
// only referenced from the wasm32-gated backends.

use bytes::Bytes;

use super::config::StorageConfig;

pub(crate) const NAMESPACE_SEPARATOR: char = ':';

#[doc(hidden)]
pub fn encode_bytes(val: &Bytes) -> String {
    val.iter().map(|&b| b as char).collect()
}

#[doc(hidden)]
pub fn decode_bytes(s: &str) -> Bytes {
    let vec: Vec<u8> = s.chars().map(|c| c as u8).collect();
    Bytes::from(vec)
}

#[doc(hidden)]
pub fn make_key(namespace: &str, key: &str) -> String {
    format!(
        "{namespace}{SEPARATOR}{key}",
        SEPARATOR = NAMESPACE_SEPARATOR
    )
}

#[doc(hidden)]
pub fn key_prefix(namespace: &str) -> String {
    format!("{namespace}{SEPARATOR}", SEPARATOR = NAMESPACE_SEPARATOR)
}

#[doc(hidden)]
pub fn apply_prefix(config: &StorageConfig, namespace: &str) -> String {
    match &config.namespace_prefix {
        Some(prefix) => format!(
            "{prefix}{SEPARATOR}{namespace}",
            SEPARATOR = NAMESPACE_SEPARATOR
        ),
        None => namespace.to_owned(),
    }
}

#[doc(hidden)]
pub fn strip_prefix(config: &StorageConfig, stored: &str) -> String {
    match &config.namespace_prefix {
        Some(prefix) => {
            let prefix_str = format!("{prefix}{SEPARATOR}", SEPARATOR = NAMESPACE_SEPARATOR);
            if stored.starts_with(&prefix_str) {
                stored[prefix_str.len()..].to_owned()
            } else {
                stored.to_owned()
            }
        }
        None => stored.to_owned(),
    }
}
