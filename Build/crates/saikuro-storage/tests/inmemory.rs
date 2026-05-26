//! Integration tests for the in-memory storage backend.
//!
//! This is the native fallback for `LocalStorage` and `SessionStorage`,
//! so exercising it here verifies the behaviour that adapter users get
//! on non-wasm32 platforms.

use bytes::Bytes;
use saikuro_storage::{InMemoryStorage, KeyValueBackend, StorageBackend, StorageConfig};

// Construction

#[test]
fn new_creates_empty_store() {
    let s = InMemoryStorage::new();
    assert_eq!(s.config(), &StorageConfig::default());
}

#[test]
fn with_config_applies_config() {
    let cfg = StorageConfig::durable().with_prefix("test");
    let s = InMemoryStorage::with_config(cfg.clone());
    assert_eq!(s.config(), &cfg);
}

// put / get / exists

#[test]
fn put_and_get_roundtrip() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("hello")).await.unwrap();
        let v = s.get("ns", "k").await.unwrap();
        assert_eq!(v, Some(Bytes::from("hello")));
    })
}

#[test]
fn get_missing_key_returns_none() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        let v = s.get("ns", "missing").await.unwrap();
        assert_eq!(v, None);
    })
}

#[test]
fn exists_returns_true_for_existing_key() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.unwrap();
        assert!(s.exists("ns", "k").await.unwrap());
    })
}

#[test]
fn exists_returns_false_for_missing_key() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        assert!(!s.exists("ns", "missing").await.unwrap());
    })
}

#[test]
fn exists_errors_on_missing_namespace() {
    saikuro_exec::block_on(async {
        let cfg = StorageConfig {
            auto_create_namespaces: false,
            namespace_prefix: Some("x".into()),
            ..Default::default()
        };
        let s = InMemoryStorage::with_config(cfg);
        let r = s.exists("nonexistent", "k").await;
        assert!(r.is_err());
    })
}

#[test]
fn put_overwrites_existing() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v1")).await.unwrap();
        s.put("ns", "k", Bytes::from("v2")).await.unwrap();
        let v = s.get("ns", "k").await.unwrap();
        assert_eq!(v, Some(Bytes::from("v2")));
    })
}

#[test]
fn put_and_get_binary_data() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        let data: Bytes = (0..=255).collect();
        s.put("ns", "bin", data.clone()).await.unwrap();
        let v = s.get("ns", "bin").await.unwrap();
        assert_eq!(v, Some(data));
    })
}

// delete

#[test]
fn delete_removes_key() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.unwrap();
        s.delete("ns", "k").await.unwrap();
        assert!(!s.exists("ns", "k").await.unwrap());
    })
}

#[test]
fn delete_missing_key_does_not_error() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.delete("ns", "missing").await.unwrap();
    })
}

// list_keys

#[test]
fn list_keys_returns_all_keys() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "a", Bytes::from("1")).await.unwrap();
        s.put("ns", "b", Bytes::from("2")).await.unwrap();
        let mut keys = s.list_keys("ns").await.unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    })
}

#[test]
fn list_keys_empty_namespace() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        let keys = s.list_keys("ns").await.unwrap();
        assert!(keys.is_empty());
    })
}

#[test]
fn list_keys_isolates_namespaces() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns1", "k", Bytes::from("v")).await.unwrap();
        s.put("ns2", "k", Bytes::from("v")).await.unwrap();
        let keys1 = s.list_keys("ns1").await.unwrap();
        assert_eq!(keys1, vec!["k"]);
    })
}

// list_namespaces

#[test]
fn list_namespaces_returns_all() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns1", "a", Bytes::from("1")).await.unwrap();
        s.put("ns2", "b", Bytes::from("2")).await.unwrap();
        let mut nss = s.list_namespaces().await.unwrap();
        nss.sort();
        assert_eq!(nss, vec!["ns1", "ns2"]);
    })
}

#[test]
fn list_namespaces_empty_when_no_data() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        let nss = s.list_namespaces().await.unwrap();
        assert!(nss.is_empty());
    })
}

// create_namespace / delete_namespace / clear_namespace

#[test]
fn create_namespace_then_put_and_get() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.create_namespace("explicit").await.unwrap();
        s.put("explicit", "k", Bytes::from("v")).await.unwrap();
        let v = s.get("explicit", "k").await.unwrap();
        assert_eq!(v, Some(Bytes::from("v")));
    })
}

#[test]
fn create_existing_namespace_errors() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.create_namespace("ns").await.unwrap();
        let r = s.create_namespace("ns").await;
        assert!(r.is_err());
    })
}

#[test]
fn delete_namespace_removes_all_keys() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.unwrap();
        s.delete_namespace("ns").await.unwrap();
        assert_eq!(s.get("ns", "k").await.unwrap(), None);
    })
}

#[test]
fn delete_nonexistent_namespace_does_not_error() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.delete_namespace("nowhere").await.unwrap();
    })
}

#[test]
fn clear_namespace_empties_keys() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.unwrap();
        s.clear_namespace("ns").await.unwrap();
        assert!(!s.exists("ns", "k").await.unwrap());
    })
}

#[test]
fn clear_namespace_preserves_namespace() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.unwrap();
        s.clear_namespace("ns").await.unwrap();
        s.put("ns", "k2", Bytes::from("v")).await.unwrap();
        assert!(s.exists("ns", "k2").await.unwrap());
    })
}

// auto_create_namespaces = false

#[test]
fn put_auto_creates_namespace_by_default() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::new();
        s.put("auto", "k", Bytes::from("v")).await.unwrap();
        assert!(s.exists("auto", "k").await.unwrap());
    })
}

#[test]
fn put_fails_when_auto_create_disabled() {
    saikuro_exec::block_on(async {
        let cfg = StorageConfig {
            auto_create_namespaces: false,
            ..Default::default()
        };
        let s = InMemoryStorage::with_config(cfg);
        let r = s.put("manual", "k", Bytes::from("v")).await;
        assert!(r.is_err());
    })
}

#[test]
fn get_fails_on_missing_namespace_without_auto_create() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::with_config(StorageConfig {
            auto_create_namespaces: false,
            ..Default::default()
        });
        let r = s.get("nowhere", "k").await;
        assert!(r.is_err());
    })
}

// namespace prefix isolation

#[test]
fn namespace_prefix_isolates_storage() {
    saikuro_exec::block_on(async {
        let a = InMemoryStorage::with_config(StorageConfig::default().with_prefix("tenant_a"));
        let b = InMemoryStorage::with_config(StorageConfig::default().with_prefix("tenant_b"));

        a.put("ns", "k", Bytes::from("from_a")).await.unwrap();
        b.put("ns", "k", Bytes::from("from_b")).await.unwrap();

        assert_eq!(a.get("ns", "k").await.unwrap(), Some(Bytes::from("from_a")));
        assert_eq!(b.get("ns", "k").await.unwrap(), Some(Bytes::from("from_b")));
    })
}

#[test]
fn namespace_prefix_list_namespaces_is_stripped() {
    saikuro_exec::block_on(async {
        let s = InMemoryStorage::with_config(StorageConfig::default().with_prefix("app"));
        s.put("myns", "k", Bytes::from("v")).await.unwrap();
        let nss = s.list_namespaces().await.unwrap();
        assert_eq!(nss, vec!["myns"]);
    })
}

// StorageBackend trait

#[test]
fn supports_files_is_false() {
    assert!(!InMemoryStorage::new().supports_files());
}

#[test]
fn as_file_backend_is_none() {
    assert!(InMemoryStorage::new().as_file_backend().is_none());
}
