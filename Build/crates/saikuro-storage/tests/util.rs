use bytes::Bytes;
use saikuro_storage::util::{
    apply_prefix, decode_bytes, encode_bytes, key_prefix, make_key, strip_prefix,
};
use saikuro_storage::StorageConfig;

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
