use bytes::Bytes;
use saikuro_storage::util::{
    apply_prefix, decode_bytes, encode_bytes, key_prefix, make_key, strip_prefix,
};
use saikuro_storage::StorageConfig;

// encode_bytes / decode_bytes


#[test]
fn encode_decode_roundtrip_all_bytes() {
    let b: Bytes = (0..=255).collect();
    assert_eq!(decode_bytes(&encode_bytes(&b)), b);
}


// make_key / key_prefix


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
fn strip_prefix_does_not_strip_unprefixed() {
    let cfg = config_with_prefix("app");
    assert_eq!(strip_prefix(&cfg, "other:myns"), "other:myns");
}


#[test]
fn apply_prefix_then_strip_prefix_no_prefix() {
    let cfg = StorageConfig::default();
    let original = "myns";
    let applied = apply_prefix(&cfg, original);
    let stripped = strip_prefix(&cfg, &applied);
    assert_eq!(stripped, original);
}
