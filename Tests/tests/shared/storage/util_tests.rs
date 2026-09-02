
use crate::TestSuite;
use saikuro_storage::util;
use saikuro_storage::StorageConfig;

pub fn register(suite: &mut TestSuite) {
    suite.register("storage::encode_decode_empty", encode_decode_empty);
    suite.register("storage::encode_decode_ascii", encode_decode_ascii);
    suite.register("storage::encode_decode_binary", encode_decode_binary);
    suite.register("storage::make_key_joins", make_key_joins);
    suite.register(
        "storage::make_key_empty_namespace",
        make_key_empty_namespace,
    );
    suite.register("storage::make_key_empty_key", make_key_empty_key);
    suite.register(
        "storage::apply_prefix_none_is_identity",
        apply_prefix_none_is_identity,
    );
    suite.register(
        "storage::strip_prefix_none_is_identity",
        strip_prefix_none_is_identity,
    );
    suite.register(
        "storage::apply_then_strip_roundtrip",
        apply_then_strip_roundtrip,
    );
    suite.register(
        "storage::apply_with_prefix_prepends",
        apply_with_prefix_prepends,
    );
    suite.register(
        "storage::strip_with_prefix_removes",
        strip_with_prefix_removes,
    );
    suite.register(
        "storage::key_prefix_ends_with_separator",
        key_prefix_ends_with_separator,
    );
    suite.register(
        "storage::key_prefix_empty_namespace",
        key_prefix_empty_namespace,
    );
    suite.register(
        "storage::strip_prefix_does_not_strip_unprefixed",
        strip_prefix_does_not_strip_unprefixed,
    );
    suite.register(
        "storage::apply_prefix_then_strip_prefix_no_prefix",
        apply_prefix_then_strip_prefix_no_prefix,
    );
}

fn encode_decode_empty() -> Result<(), &'static str> {
    let encoded = util::encode_bytes(&bytes::Bytes::new());
    let decoded = util::decode_bytes(&encoded);
    assert_eq!(decoded, bytes::Bytes::new());
    Ok(())
}

fn encode_decode_ascii() -> Result<(), &'static str> {
    let original = bytes::Bytes::from_static(b"hello");
    let encoded = util::encode_bytes(&original);
    let decoded = util::decode_bytes(&encoded);
    assert_eq!(decoded, original);
    Ok(())
}

fn encode_decode_binary() -> Result<(), &'static str> {
    let data: alloc::vec::Vec<u8> = (0..=255).collect();
    let original = bytes::Bytes::from(data);
    let encoded = util::encode_bytes(&original);
    let decoded = util::decode_bytes(&encoded);
    assert_eq!(decoded, original);
    Ok(())
}

fn make_key_joins() -> Result<(), &'static str> {
    let key = util::make_key("ns", "key");
    assert!(key.contains("ns"));
    assert!(key.contains("key"));
    Ok(())
}

fn make_key_empty_namespace() -> Result<(), &'static str> {
    let key = util::make_key("", "key");
    assert!(!key.is_empty());
    Ok(())
}

fn make_key_empty_key() -> Result<(), &'static str> {
    let key = util::make_key("ns", "");
    assert!(!key.is_empty());
    Ok(())
}

fn apply_prefix_none_is_identity() -> Result<(), &'static str> {
    let config = StorageConfig::default();
    let result = util::apply_prefix(&config, "ns");
    assert_eq!(result, "ns");
    Ok(())
}

fn strip_prefix_none_is_identity() -> Result<(), &'static str> {
    let config = StorageConfig::default();
    let result = util::strip_prefix(&config, "ns:key");
    assert_eq!(result, "ns:key");
    Ok(())
}

fn apply_then_strip_roundtrip() -> Result<(), &'static str> {
    let mut config = StorageConfig::default();
    config.namespace_prefix = Some("global".into());
    let applied = util::apply_prefix(&config, "ns");
    let stripped = util::strip_prefix(&config, &applied);
    assert_eq!(stripped, "ns");
    Ok(())
}

fn apply_with_prefix_prepends() -> Result<(), &'static str> {
    let mut config = StorageConfig::default();
    config.namespace_prefix = Some("pfx".into());
    let applied = util::apply_prefix(&config, "ns");
    assert!(applied.starts_with("pfx"));
    Ok(())
}

fn strip_with_prefix_removes() -> Result<(), &'static str> {
    let mut config = StorageConfig::default();
    config.namespace_prefix = Some("pfx".into());
    let applied = util::apply_prefix(&config, "ns");
    let stripped = util::strip_prefix(&config, &applied);
    assert_eq!(stripped, "ns");
    Ok(())
}

fn key_prefix_ends_with_separator() -> Result<(), &'static str> {
    assert_eq!(util::key_prefix("ns"), "ns:");
    Ok(())
}

fn key_prefix_empty_namespace() -> Result<(), &'static str> {
    assert_eq!(util::key_prefix(""), ":");
    Ok(())
}

fn strip_prefix_does_not_strip_unprefixed() -> Result<(), &'static str> {
    let cfg = StorageConfig::default().with_prefix("app");
    assert_eq!(util::strip_prefix(&cfg, "other:myns"), "other:myns");
    Ok(())
}

fn apply_prefix_then_strip_prefix_no_prefix() -> Result<(), &'static str> {
    let cfg = StorageConfig::default();
    let original = "myns";
    let applied = util::apply_prefix(&cfg, original);
    let stripped = util::strip_prefix(&cfg, &applied);
    assert_eq!(stripped, original);
    Ok(())
}
