// Pure helper functions shared by webstorage, opfs, and indexeddb backends.
// These are re-exported from the wasm32-gated webstorage module so the
// impl_web_storage! macro can reach them via $crate::webstorage::*.
//
// The dead_code allow is needed because on native these are only referenced
// from #[cfg(test)] and from the wasm32-gated webstorage module.

#![allow(dead_code)]

use bytes::Bytes;

use super::config::StorageConfig;

pub(crate) const NAMESPACE_SEPARATOR: char = ':';

pub(crate) fn encode_bytes(val: &Bytes) -> String {
    val.iter().map(|&b| b as char).collect()
}

pub(crate) fn decode_bytes(s: &str) -> Bytes {
    let vec: Vec<u8> = s.chars().map(|c| c as u8).collect();
    Bytes::from(vec)
}

pub(crate) fn make_key(namespace: &str, key: &str) -> String {
    format!(
        "{namespace}{SEPARATOR}{key}",
        SEPARATOR = NAMESPACE_SEPARATOR
    )
}

pub(crate) fn key_prefix(namespace: &str) -> String {
    format!("{namespace}{SEPARATOR}", SEPARATOR = NAMESPACE_SEPARATOR)
}

pub(crate) fn apply_prefix(config: &StorageConfig, namespace: &str) -> String {
    match &config.namespace_prefix {
        Some(prefix) => format!(
            "{prefix}{SEPARATOR}{namespace}",
            SEPARATOR = NAMESPACE_SEPARATOR
        ),
        None => namespace.to_owned(),
    }
}

pub(crate) fn strip_prefix(config: &StorageConfig, stored: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    // encode_bytes / decode_bytes

    #[test]
    fn encode_decode_roundtrip_empty() {
        let b = Bytes::new();
        assert_eq!(decode_bytes(&encode_bytes(&b)), b);
    }

    #[test]
    fn encode_decode_roundtrip_ascii() {
        let b = Bytes::from("hello");
        assert_eq!(decode_bytes(&encode_bytes(&b)), b);
    }

    #[test]
    fn encode_decode_roundtrip_all_bytes() {
        let b: Bytes = (0..=255).collect();
        assert_eq!(decode_bytes(&encode_bytes(&b)), b);
    }

    #[test]
    fn encode_decode_roundtrip_binary() {
        let b = Bytes::from(&[0x00, 0x01, 0x7f, 0x80, 0xff, 0xab][..]);
        assert_eq!(decode_bytes(&encode_bytes(&b)), b);
    }

    // make_key / key_prefix

    #[test]
    fn make_key_joins_with_separator() {
        assert_eq!(make_key("ns", "k"), "ns:k");
    }

    #[test]
    fn make_key_with_empty_namespace() {
        assert_eq!(make_key("", "k"), ":k");
    }

    #[test]
    fn make_key_with_empty_key() {
        assert_eq!(make_key("ns", ""), "ns:");
    }

    #[test]
    fn key_prefix_ends_with_separator() {
        assert_eq!(key_prefix("ns"), "ns:");
    }

    #[test]
    fn key_prefix_empty_namespace() {
        assert_eq!(key_prefix(""), ":");
    }

    // apply_prefix / strip_prefix

    fn config_with_prefix(prefix: &str) -> StorageConfig {
        StorageConfig::default().with_prefix(prefix)
    }

    #[test]
    fn apply_prefix_without_config_prefix_is_identity() {
        let cfg = StorageConfig::default();
        assert_eq!(apply_prefix(&cfg, "myns"), "myns");
    }

    #[test]
    fn apply_prefix_prepends_global_prefix() {
        let cfg = config_with_prefix("app");
        assert_eq!(apply_prefix(&cfg, "myns"), "app:myns");
    }

    #[test]
    fn strip_prefix_without_config_prefix_is_identity() {
        let cfg = StorageConfig::default();
        assert_eq!(strip_prefix(&cfg, "myns"), "myns");
    }

    #[test]
    fn strip_prefix_removes_global_prefix() {
        let cfg = config_with_prefix("app");
        assert_eq!(strip_prefix(&cfg, "app:myns"), "myns");
    }

    #[test]
    fn strip_prefix_does_not_strip_unprefixed() {
        let cfg = config_with_prefix("app");
        assert_eq!(strip_prefix(&cfg, "other:myns"), "other:myns");
    }

    #[test]
    fn apply_prefix_then_strip_prefix_roundtrip() {
        let cfg = config_with_prefix("app");
        let original = "myns";
        let applied = apply_prefix(&cfg, original);
        let stripped = strip_prefix(&cfg, &applied);
        assert_eq!(stripped, original);
    }

    #[test]
    fn apply_prefix_then_strip_prefix_no_prefix() {
        let cfg = StorageConfig::default();
        let original = "myns";
        let applied = apply_prefix(&cfg, original);
        let stripped = strip_prefix(&cfg, &applied);
        assert_eq!(stripped, original);
    }
}
